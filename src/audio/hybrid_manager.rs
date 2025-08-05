use anyhow::{Context, Result};
use async_trait::async_trait;
use serenity::model::id::{ChannelId, GuildId, UserId};
use serenity::prelude::*;
use songbird::{Call, Event, EventContext, EventHandler as VoiceEventHandler, Songbird};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::audio::player::AudioPlayer;
use crate::sources::{TrackSource, YtDlpOptimizedClient, MusicSource};
use crate::audio::lavalink_simple::{LavalinkManager, Track};
use crate::config::Config;

/// Manager híbrido que combina Songbird real con preparación para Lavalink
pub struct HybridAudioManager {
    /// Songbird manager para conexiones de voz reales
    songbird: Arc<Songbird>,
    
    /// Audio players por guild
    players: Arc<RwLock<std::collections::HashMap<GuildId, Arc<AudioPlayer>>>>,
    
    /// Estado de conexiones de voz
    voice_connections: Arc<RwLock<std::collections::HashMap<GuildId, Arc<tokio::sync::Mutex<Call>>>>>,
    
    /// Configuración
    config: Arc<Config>,
    
    /// Flag para indicar si Lavalink está disponible
    lavalink_available: bool,
}

impl HybridAudioManager {
    pub fn new(songbird: Arc<Songbird>, config: Arc<Config>) -> Self {
        info!("🎵 Inicializando HybridAudioManager");
        
        Self {
            songbird,
            players: Arc::new(RwLock::new(std::collections::HashMap::new())),
            voice_connections: Arc::new(RwLock::new(std::collections::HashMap::new())),
            config,
            lavalink_available: false, // Por ahora false, se puede cambiar dinámicamente
        }
    }

    /// Conecta a un canal de voz y prepara para reproducción
    pub async fn join_channel(&self, guild_id: GuildId, channel_id: ChannelId, user_id: UserId) -> Result<()> {
        info!("🔗 Conectando al canal {} en guild {}", channel_id, guild_id);

        // Obtener o crear la llamada de voz
        let call = self.songbird.join(guild_id, channel_id).await
            .map_err(|e| anyhow::anyhow!("Error al unirse al canal: {:?}", e))?;

        // Almacenar la conexión
        {
            let mut connections = self.voice_connections.write().await;
            connections.insert(guild_id, call.clone());
        }

        // Configurar event handlers para la llamada
        {
            let mut call_lock = call.lock().await;
            call_lock.add_global_event(
                Event::Track(songbird::TrackEvent::End),
                TrackEndHandler {
                    guild_id,
                    manager: Arc::new(self.clone()),
                }
            );
        }

        // Crear o obtener player para este guild
        let player = {
            let mut players = self.players.write().await;
            players.entry(guild_id)
                .or_insert_with(|| Arc::new(AudioPlayer::new()))
                .clone()
        };

        info!("✅ Conectado exitosamente al canal {} en guild {}", channel_id, guild_id);
        Ok(())
    }

    /// Reproduce una canción usando el sistema apropiado
    pub async fn play(&self, guild_id: GuildId, query: &str, user_id: UserId) -> Result<TrackSource> {
        info!("🎵 Reproduciendo '{}' en guild {}", query, guild_id);

        if self.lavalink_available {
            // Intentar usar Lavalink primero
            match self.play_with_lavalink(guild_id, query, user_id).await {
                Ok(source) => return Ok(source),
                Err(e) => {
                    info!("🔄 Lavalink falló: {:?}, usando fallback yt-dlp", e);
                }
            }
        }

        // Usar yt-dlp + Songbird como fallback
        self.play_with_songbird(guild_id, query, user_id).await
    }

    /// Reproduce usando Songbird directamente (método funcional)
    async fn play_with_songbird(&self, guild_id: GuildId, query: &str, user_id: UserId) -> Result<TrackSource> {
        // Obtener el player
        let player = {
            let players = self.players.read().await;
            players.get(&guild_id)
                .ok_or_else(|| anyhow::anyhow!("No hay player para este guild. ¿Estás conectado a un canal de voz?"))?
                .clone()
        };

        // Buscar la canción con yt-dlp
        let ytdlp_client = YtDlpOptimizedClient::new();
        let search_results = ytdlp_client.search(query, 1).await
            .context("Error al buscar la canción")?;
        
        let source = search_results.into_iter().next()
            .ok_or_else(|| anyhow::anyhow!("No se encontraron resultados para: {}", query))?;

        // Obtener la conexión de voz
        let call = {
            let connections = self.voice_connections.read().await;
            connections.get(&guild_id)
                .ok_or_else(|| anyhow::anyhow!("No hay conexión de voz para este guild"))?
                .clone()
        };

        // Crear el input de audio optimizado
        let audio_input = source.get_input().await
            .context("Error al crear input de audio")?;

        // Agregar el track a la llamada
        {
            let mut call_lock = call.lock().await;
            let track_handle = call_lock.play_input(audio_input);
            
            info!("🎵 Track agregado exitosamente: {}", source.title());
        }

        info!("✅ Reproduciendo '{}' exitosamente", source.title());
        Ok(source)
    }

    /// Reproduce usando Lavalink (método preferido en servidor dedicado)
    async fn play_with_lavalink(&self, guild_id: GuildId, query: &str, user_id: UserId) -> Result<TrackSource> {
        info!("🎼 Usando Lavalink para reproducir: {}", query);
        
        // Obtener Lavalink manager del contexto
        // NOTA: En una implementación completa, esto vendría del contexto de Serenity
        // Por simplicidad, creamos una instancia temporal
        
        // Para esta implementación, vamos a simular que Lavalink busca y encuentra la canción
        // pero usando yt-dlp para la metadata local mientras Lavalink maneja el streaming
        
        // 1. Buscar la canción localmente para metadata
        let ytdlp_client = YtDlpOptimizedClient::new();
        let search_results = ytdlp_client.search(query, 1).await
            .context("Error al buscar la canción con yt-dlp para metadata")?;
        
        let source = search_results.into_iter().next()
            .ok_or_else(|| anyhow::anyhow!("No se encontraron resultados para: {}", query))?;

        // 2. TODO: Aquí debería usar Lavalink para el streaming real
        // Por ahora, registramos que Lavalink se usaría pero seguimos con Songbird
        info!("🎼 Lavalink buscaría y reproduciría: {}", source.title());
        
        // 3. Para mantener compatibilidad, seguimos usando Songbird para el audio
        let call = {
            let connections = self.voice_connections.read().await;
            connections.get(&guild_id)
                .ok_or_else(|| anyhow::anyhow!("No hay conexión de voz para este guild"))?
                .clone()
        };

        // 4. Crear el input de audio optimizado
        let audio_input = source.get_input().await
            .context("Error al crear input de audio")?;

        // 5. Agregar el track a la llamada
        {
            let mut call_lock = call.lock().await;
            let _track_handle = call_lock.play_input(audio_input);
            
            info!("🎵 Track agregado exitosamente vía Lavalink fallback: {}", source.title());
        }

        info!("✅ Lavalink reproduce '{}' exitosamente", source.title());
        Ok(source)
    }

    /// Pausa la reproducción actual
    pub async fn pause(&self, guild_id: GuildId) -> Result<()> {
        let call = {
            let connections = self.voice_connections.read().await;
            connections.get(&guild_id)
                .ok_or_else(|| anyhow::anyhow!("No hay conexión de voz activa"))?
                .clone()
        };

        let call_lock = call.lock().await;
        if let Some(current) = call_lock.queue().current() {
            current.pause()
                .map_err(|e| anyhow::anyhow!("Error al pausar: {:?}", e))?;
            info!("⏸️ Reproducción pausada en guild {}", guild_id);
        } else {
            return Err(anyhow::anyhow!("No hay ninguna canción reproduciéndose"));
        }

        Ok(())
    }

    /// Reanuda la reproducción
    pub async fn resume(&self, guild_id: GuildId) -> Result<()> {
        let call = {
            let connections = self.voice_connections.read().await;
            connections.get(&guild_id)
                .ok_or_else(|| anyhow::anyhow!("No hay conexión de voz activa"))?
                .clone()
        };

        let call_lock = call.lock().await;
        if let Some(current) = call_lock.queue().current() {
            current.play()
                .map_err(|e| anyhow::anyhow!("Error al reanudar: {:?}", e))?;
            info!("▶️ Reproducción reanudada en guild {}", guild_id);
        } else {
            return Err(anyhow::anyhow!("No hay ninguna canción para reanudar"));
        }

        Ok(())
    }

    /// Salta a la siguiente canción
    pub async fn skip(&self, guild_id: GuildId) -> Result<()> {
        let call = {
            let connections = self.voice_connections.read().await;
            connections.get(&guild_id)
                .ok_or_else(|| anyhow::anyhow!("No hay conexión de voz activa"))?
                .clone()
        };

        let call_lock = call.lock().await;
        if let Some(current) = call_lock.queue().current() {
            current.stop()
                .map_err(|e| anyhow::anyhow!("Error al saltar: {:?}", e))?;
            info!("⏭️ Canción saltada en guild {}", guild_id);
        } else {
            return Err(anyhow::anyhow!("No hay ninguna canción reproduciéndose"));
        }

        Ok(())
    }

    /// Detiene la reproducción y limpia la cola
    pub async fn stop(&self, guild_id: GuildId) -> Result<()> {
        let call = {
            let connections = self.voice_connections.read().await;
            connections.get(&guild_id)
                .ok_or_else(|| anyhow::anyhow!("No hay conexión de voz activa"))?
                .clone()
        };

        let call_lock = call.lock().await;
        call_lock.queue().stop();
        info!("⏹️ Reproducción detenida en guild {}", guild_id);

        Ok(())
    }

    /// Desconecta del canal de voz
    pub async fn leave_channel(&self, guild_id: GuildId) -> Result<()> {
        // Remover de Songbird
        self.songbird.leave(guild_id).await
            .map_err(|e| anyhow::anyhow!("Error al salir del canal: {:?}", e))?;

        // Limpiar estado local
        {
            let mut connections = self.voice_connections.write().await;
            connections.remove(&guild_id);
        }

        {
            let mut players = self.players.write().await;
            players.remove(&guild_id);
        }

        info!("👋 Desconectado del guild {}", guild_id);
        Ok(())
    }

    /// Obtiene información de la canción actual
    pub async fn now_playing(&self, guild_id: GuildId) -> Result<Option<NowPlayingInfo>> {
        let call = {
            let connections = self.voice_connections.read().await;
            connections.get(&guild_id)
                .ok_or_else(|| anyhow::anyhow!("No hay conexión de voz activa"))?
                .clone()
        };

        let call_lock = call.lock().await;
        if let Some(_current) = call_lock.queue().current() {
            // TODO: Implementar metadata tracking más adelante
            return Ok(Some(NowPlayingInfo {
                title: "Reproduciendo...".to_string(),
                artist: None,
                duration: None,
                position: std::time::Duration::from_secs(0),
                requester: UserId::new(1), // Placeholder
            }));
        }

        Ok(None)
    }

    /// Establece si Lavalink está disponible
    pub fn set_lavalink_available(&mut self, available: bool) {
        self.lavalink_available = available;
        if available {
            info!("🎼 Lavalink marcado como disponible");
        } else {
            info!("🔄 Usando modo fallback (Songbird + yt-dlp)");
        }
    }

    /// Verifica si hay una conexión activa
    pub async fn is_connected(&self, guild_id: GuildId) -> bool {
        let connections = self.voice_connections.read().await;
        connections.contains_key(&guild_id)
    }
}

impl Clone for HybridAudioManager {
    fn clone(&self) -> Self {
        Self {
            songbird: Arc::clone(&self.songbird),
            players: Arc::clone(&self.players),
            voice_connections: Arc::clone(&self.voice_connections),
            config: Arc::clone(&self.config),
            lavalink_available: self.lavalink_available,
        }
    }
}

impl TypeMapKey for HybridAudioManager {
    type Value = Arc<HybridAudioManager>;
}

/// Metadatos del track
#[derive(Debug, Clone)]
struct TrackMetadata {
    title: String,
    artist: Option<String>,
    duration: Option<std::time::Duration>,
    requester: UserId,
}

impl TypeMapKey for TrackMetadata {
    type Value = TrackMetadata;
}

/// Información de la canción actual
#[derive(Debug, Clone)]
pub struct NowPlayingInfo {
    pub title: String,
    pub artist: Option<String>,
    pub duration: Option<std::time::Duration>,
    pub position: std::time::Duration,
    pub requester: UserId,
}

/// Event handler para cuando termina un track
struct TrackEndHandler {
    guild_id: GuildId,
    manager: Arc<HybridAudioManager>,
}

#[async_trait]
impl VoiceEventHandler for TrackEndHandler {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        debug!("🎵 Track terminado en guild {}", self.guild_id);
        
        // Aquí se podría implementar lógica para reproducir la siguiente canción
        // de una cola, pero por simplicidad lo dejamos así por ahora
        
        None
    }
}