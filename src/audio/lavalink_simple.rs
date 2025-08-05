use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serenity::model::id::{GuildId, UserId};
use serenity::prelude::*;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::config::Config;

/// Cliente real para Lavalink API
pub struct LavalinkManager {
    client: Client,
    base_url: String,
    password: String,
    user_id: UserId,
}

#[derive(Debug, Serialize, Deserialize)]
struct SearchResponse {
    #[serde(rename = "loadType")]
    load_type: String,
    data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub encoded: String,
    pub info: TrackInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    pub identifier: String,
    pub title: String,
    pub author: String,
    pub length: u64,
    pub uri: String,
}

#[derive(Debug, Serialize)]
struct PlayRequest {
    #[serde(rename = "encodedTrack")]
    encoded_track: String,
}

impl LavalinkManager {
    /// Crea una nueva instancia de LavalinkManager
    pub async fn new(config: &Config, user_id: UserId) -> Result<Self> {
        let host = std::env::var("LAVALINK_HOST")
            .unwrap_or_else(|_| "lavalink".to_string());
        let port = std::env::var("LAVALINK_PORT")
            .unwrap_or_else(|_| "2333".to_string())
            .parse::<u16>()
            .unwrap_or(2333);
        let password = std::env::var("LAVALINK_PASSWORD")
            .unwrap_or_else(|_| "youshallnotpass".to_string());
        
        let base_url = format!("http://{}:{}", host, port);
        
        info!("🎼 Configurando Lavalink en {}", base_url);
        
        let client = Client::new();
        
        // Verificar que Lavalink esté disponible
        let version_url = format!("{}/version", base_url);
        match client.get(&version_url).send().await {
            Ok(response) if response.status().is_success() => {
                let version = response.text().await.unwrap_or_default();
                info!("✅ Lavalink conectado exitosamente - Version: {}", version);
            }
            Ok(response) => {
                warn!("⚠️ Lavalink responde pero con estado: {}", response.status());
            }
            Err(e) => {
                warn!("⚠️ No se pudo conectar a Lavalink: {:?}", e);
                return Err(anyhow!("Lavalink no disponible: {}", e));
            }
        }
        
        Ok(Self {
            client,
            base_url,
            password,
            user_id,
        })
    }

    /// Busca tracks usando Lavalink
    pub async fn search(&self, query: &str) -> Result<Vec<Track>> {
        info!("🔍 Buscando con Lavalink: {}", query);
        
        // Crear la búsqueda con prefijo de YouTube
        let search_query = if query.starts_with("http") {
            query.to_string()
        } else {
            format!("ytsearch:{}", query)
        };
        
        let url = format!("{}/v4/loadtracks?identifier={}", 
            self.base_url, 
            urlencoding::encode(&search_query)
        );
        
        let response = self.client
            .get(&url)
            .header("Authorization", &self.password)
            .send()
            .await
            .map_err(|e| anyhow!("Error al hacer request a Lavalink: {}", e))?;
            
        if !response.status().is_success() {
            return Err(anyhow!("Lavalink respondió con estado: {}", response.status()));
        }
        
        let search_response: SearchResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Error al parsear respuesta JSON: {}", e))?;
            
        match search_response.load_type.as_str() {
            "track" => {
                let track: Track = serde_json::from_value(search_response.data)
                    .map_err(|e| anyhow!("Error al parsear track: {}", e))?;
                Ok(vec![track])
            }
            "search" => {
                let tracks: Vec<Track> = serde_json::from_value(search_response.data)
                    .map_err(|e| anyhow!("Error al parsear lista de tracks: {}", e))?;
                Ok(tracks)
            }
            "playlist" => {
                let playlist = search_response.data.as_object()
                    .ok_or_else(|| anyhow!("Respuesta de playlist inválida"))?;
                let tracks: Vec<Track> = serde_json::from_value(
                    playlist.get("tracks").unwrap_or(&serde_json::Value::Array(vec![])).clone()
                ).map_err(|e| anyhow!("Error al parsear tracks de playlist: {}", e))?;
                Ok(tracks)
            }
            "empty" => {
                warn!("📭 No se encontraron resultados para: {}", query);
                Ok(vec![])
            }
            "error" => {
                let error_msg = search_response.data.get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Error desconocido");
                Err(anyhow!("Lavalink error: {}", error_msg))
            }
            _ => {
                Err(anyhow!("Tipo de respuesta desconocido: {}", search_response.load_type))
            }
        }
    }

    /// Reproduce un track en un guild específico
    pub async fn play(&self, guild_id: GuildId, track: &Track) -> Result<()> {
        info!("🎵 Reproduciendo en guild {}: {}", guild_id, track.info.title);
        
        let url = format!("{}/v4/sessions/default/players/{}", 
            self.base_url, 
            guild_id.get()
        );
        
        let play_request = PlayRequest {
            encoded_track: track.encoded.clone(),
        };
        
        let response = self.client
            .patch(&url)
            .header("Authorization", &self.password)
            .header("Content-Type", "application/json")
            .json(&play_request)
            .send()
            .await
            .map_err(|e| anyhow!("Error al enviar comando play: {}", e))?;
            
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Error al reproducir: {} - {}", status, text));
        }
        
        info!("✅ Track enviado a Lavalink exitosamente");
        Ok(())
    }

    pub async fn pause(&self, guild_id: GuildId) -> Result<()> {
        info!("⏸️ Pausando reproductor en guild {}", guild_id);
        self.update_player(guild_id, r#"{"paused": true}"#).await
    }

    pub async fn resume(&self, guild_id: GuildId) -> Result<()> {
        info!("▶️ Reanudando reproductor en guild {}", guild_id);
        self.update_player(guild_id, r#"{"paused": false}"#).await
    }

    pub async fn skip(&self, guild_id: GuildId) -> Result<Option<String>> {
        info!("⏭️ Saltando track en guild {}", guild_id);
        
        let url = format!("{}/v4/sessions/default/players/{}", 
            self.base_url, 
            guild_id.get()
        );
        
        let response = self.client
            .patch(&url)
            .header("Authorization", &self.password)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({"encodedTrack": null}))
            .send()
            .await
            .map_err(|e| anyhow!("Error al saltar: {}", e))?;
            
        if response.status().is_success() {
            Ok(Some("Track saltado".to_string()))
        } else {
            Err(anyhow!("Error al saltar: {}", response.status()))
        }
    }

    pub async fn stop(&self, guild_id: GuildId) -> Result<()> {
        info!("⏹️ Deteniendo reproductor en guild {}", guild_id);
        
        let url = format!("{}/v4/sessions/default/players/{}", 
            self.base_url, 
            guild_id.get()
        );
        
        let response = self.client
            .delete(&url)
            .header("Authorization", &self.password)
            .send()
            .await
            .map_err(|e| anyhow!("Error al detener: {}", e))?;
            
        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow!("Error al detener: {}", response.status()))
        }
    }

    pub async fn set_volume(&self, guild_id: GuildId, volume: i32) -> Result<()> {
        info!("🔊 Ajustando volumen en guild {} a {}", guild_id, volume);
        let volume_json = format!(r#"{{"volume": {}}}"#, volume);
        self.update_player(guild_id, &volume_json).await
    }

    async fn update_player(&self, guild_id: GuildId, json_body: &str) -> Result<()> {
        let url = format!("{}/v4/sessions/default/players/{}", 
            self.base_url, 
            guild_id.get()
        );
        
        let response = self.client
            .patch(&url)
            .header("Authorization", &self.password)
            .header("Content-Type", "application/json")
            .body(json_body.to_string())
            .send()
            .await
            .map_err(|e| anyhow!("Error al actualizar player: {}", e))?;
            
        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow!("Error al actualizar player: {}", response.status()))
        }
    }

    // Métodos placeholder para mantener compatibilidad
    pub async fn get_queue(&self, guild_id: GuildId) -> Vec<String> {
        info!("📄 Obteniendo cola para guild {}", guild_id);
        vec![] // TODO: Implementar gestión de cola real
    }

    pub async fn clear_queue(&self, guild_id: GuildId) -> Result<usize> {
        info!("🗑️ Limpiando cola para guild {}", guild_id);
        Ok(0) // TODO: Implementar gestión de cola real
    }

    pub async fn join_channel(&self, guild_id: GuildId, channel_id: serenity::model::id::ChannelId) -> Result<()> {
        info!("🔗 Uniendo a canal {} en guild {}", channel_id, guild_id);
        
        let url = format!("{}/v4/sessions/default/players/{}", 
            self.base_url, 
            guild_id.get()
        );
        
        let voice_update = serde_json::json!({
            "voice": {
                "token": "placeholder",
                "endpoint": "placeholder", 
                "sessionId": "placeholder"
            }
        });
        
        let response = self.client
            .patch(&url)
            .header("Authorization", &self.password)
            .header("Content-Type", "application/json")
            .json(&voice_update)
            .send()
            .await
            .map_err(|e| anyhow!("Error al unirse al canal: {}", e))?;
            
        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow!("Error al unirse al canal: {}", response.status()))
        }
    }

    pub async fn leave_channel(&self, guild_id: GuildId) -> Result<()> {
        info!("👋 Dejando canal en guild {}", guild_id);
        self.stop(guild_id).await
    }
}

impl TypeMapKey for LavalinkManager {
    type Value = Arc<LavalinkManager>;
}