use anyhow::{Context, Result};
use lavalink_rs::{
    prelude::*,
    model::{track::Track, LoadType},
};
use serenity::model::id::{GuildId, UserId};
use serenity::prelude::*;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::config::Config;

pub struct LavalinkManager {
    client: Arc<LavalinkClient>,
    guild_players: Arc<RwLock<std::collections::HashMap<GuildId, PlayerState>>>,
}

#[derive(Debug, Clone)]
pub struct PlayerState {
    pub current_track: Option<Track>,
    pub queue: Vec<Track>,
    pub position: u64,
    pub volume: i32,
    pub is_paused: bool,
    pub loop_mode: LoopMode,
}

#[derive(Debug, Clone)]
pub enum LoopMode {
    Off,
    Track,
    Queue,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            current_track: None,
            queue: Vec::new(),
            position: 0,
            volume: 50,
            is_paused: false,
            loop_mode: LoopMode::Off,
        }
    }
}

impl LavalinkManager {
    pub async fn new(config: &Config, user_id: UserId) -> Result<Self> {
        let lavalink_host = std::env::var("LAVALINK_HOST")
            .unwrap_or_else(|_| "localhost".to_string());
        let lavalink_port = std::env::var("LAVALINK_PORT")
            .unwrap_or_else(|_| "2333".to_string())
            .parse::<u16>()
            .unwrap_or(2333);
        let lavalink_password = std::env::var("LAVALINK_PASSWORD")
            .unwrap_or_else(|_| "youshallnotpass".to_string());

        info!("Conectando a Lavalink en {}:{}", lavalink_host, lavalink_port);

        let client = LavalinkClient::builder(user_id)
            .node(NodeBuilder {
                hostname: lavalink_host,
                port: lavalink_port,
                password: lavalink_password,
                is_ssl: false,
            })
            .build()
            .await
            .context("Error al construir cliente Lavalink")?;

        Ok(Self {
            client: Arc::new(client),
            guild_players: Arc::new(RwLock::new(std::collections::HashMap::new())),
        })
    }

    pub async fn search(&self, query: &str) -> Result<Vec<Track>> {
        let query = if query.starts_with("http") {
            query.to_string()
        } else {
            format!("ytsearch:{}", query)
        };

        let tracks = self.client
            .load_tracks(&query)
            .await
            .context("Error al buscar tracks")?;

        match tracks.load_type {
            LoadType::Track => Ok(vec![tracks.data.track.unwrap()]),
            LoadType::Playlist => Ok(tracks.data.playlist.unwrap().tracks),
            LoadType::Search => Ok(tracks.data.tracks),
            LoadType::Empty => {
                warn!("No se encontraron resultados para: {}", query);
                Ok(Vec::new())
            }
            LoadType::Error => {
                error!("Error al cargar track: {:?}", tracks.data.exception);
                Err(anyhow::anyhow!("Error al cargar track"))
            }
        }
    }

    pub async fn play(&self, guild_id: GuildId, track: Track) -> Result<()> {
        // Conectar al canal de voz si no está conectado
        self.client.create_session(guild_id).await
            .context("Error al crear sesión")?;

        // Reproducir track
        self.client.play(guild_id, track.clone())
            .requester(0) // Default requester
            .await
            .context("Error al reproducir track")?;

        // Actualizar estado del player
        let mut players = self.guild_players.write().await;
        let player_state = players.entry(guild_id).or_default();
        player_state.current_track = Some(track);
        player_state.is_paused = false;

        Ok(())
    }

    pub async fn add_to_queue(&self, guild_id: GuildId, tracks: Vec<Track>) -> Result<usize> {
        let mut players = self.guild_players.write().await;
        let player_state = players.entry(guild_id).or_default();
        
        let added_count = tracks.len();
        player_state.queue.extend(tracks);

        info!("Agregadas {} canciones a la cola de {}", added_count, guild_id);
        Ok(added_count)
    }

    pub async fn skip(&self, guild_id: GuildId) -> Result<Option<Track>> {
        let next_track = {
            let mut players = self.guild_players.write().await;
            let player_state = players.entry(guild_id).or_default();
            
            if player_state.queue.is_empty() {
                return Ok(None);
            }
            
            Some(player_state.queue.remove(0))
        };

        if let Some(track) = next_track.clone() {
            self.play(guild_id, track).await?;
        } else {
            self.stop(guild_id).await?;
        }

        Ok(next_track)
    }

    pub async fn pause(&self, guild_id: GuildId) -> Result<()> {
        self.client.pause(guild_id)
            .await
            .context("Error al pausar")?;

        let mut players = self.guild_players.write().await;
        if let Some(player_state) = players.get_mut(&guild_id) {
            player_state.is_paused = true;
        }

        Ok(())
    }

    pub async fn resume(&self, guild_id: GuildId) -> Result<()> {
        self.client.resume(guild_id)
            .await
            .context("Error al reanudar")?;

        let mut players = self.guild_players.write().await;
        if let Some(player_state) = players.get_mut(&guild_id) {
            player_state.is_paused = false;
        }

        Ok(())
    }

    pub async fn stop(&self, guild_id: GuildId) -> Result<()> {
        self.client.stop(guild_id)
            .await
            .context("Error al detener")?;

        let mut players = self.guild_players.write().await;
        if let Some(player_state) = players.get_mut(&guild_id) {
            player_state.current_track = None;
            player_state.is_paused = false;
            player_state.position = 0;
        }

        Ok(())
    }

    pub async fn set_volume(&self, guild_id: GuildId, volume: i32) -> Result<()> {
        let volume = volume.clamp(0, 200);
        
        self.client.volume(guild_id, volume)
            .await
            .context("Error al cambiar volumen")?;

        let mut players = self.guild_players.write().await;
        if let Some(player_state) = players.get_mut(&guild_id) {
            player_state.volume = volume;
        }

        Ok(())
    }

    pub async fn seek(&self, guild_id: GuildId, position: u64) -> Result<()> {
        self.client.seek(guild_id, position)
            .await
            .context("Error al buscar posición")?;

        let mut players = self.guild_players.write().await;
        if let Some(player_state) = players.get_mut(&guild_id) {
            player_state.position = position;
        }

        Ok(())
    }

    pub async fn get_player_state(&self, guild_id: GuildId) -> PlayerState {
        let players = self.guild_players.read().await;
        players.get(&guild_id).cloned().unwrap_or_default()
    }

    pub async fn clear_queue(&self, guild_id: GuildId) -> Result<usize> {
        let mut players = self.guild_players.write().await;
        let count = if let Some(player_state) = players.get_mut(&guild_id) {
            let count = player_state.queue.len();
            player_state.queue.clear();
            count
        } else {
            0
        };

        Ok(count)
    }

    pub async fn shuffle_queue(&self, guild_id: GuildId) -> Result<()> {
        use rand::seq::SliceRandom;
        
        let mut players = self.guild_players.write().await;
        if let Some(player_state) = players.get_mut(&guild_id) {
            let mut rng = rand::thread_rng();
            player_state.queue.shuffle(&mut rng);
        }

        Ok(())
    }

    pub async fn remove_from_queue(&self, guild_id: GuildId, index: usize) -> Result<Option<Track>> {
        let mut players = self.guild_players.write().await;
        let removed = if let Some(player_state) = players.get_mut(&guild_id) {
            if index < player_state.queue.len() {
                Some(player_state.queue.remove(index))
            } else {
                None
            }
        } else {
            None
        };

        Ok(removed)
    }

    pub async fn join_channel(&self, guild_id: GuildId, channel_id: serenity::model::id::ChannelId) -> Result<()> {
        self.client.create_session_with_channel(guild_id, channel_id).await
            .context("Error al unirse al canal de voz")?;
        
        info!("Conectado al canal {} en guild {}", channel_id, guild_id);
        Ok(())
    }

    pub async fn leave_channel(&self, guild_id: GuildId) -> Result<()> {
        self.client.destroy(guild_id).await
            .context("Error al salir del canal de voz")?;
        
        // Limpiar estado del player
        let mut players = self.guild_players.write().await;
        players.remove(&guild_id);
        
        info!("Desconectado del guild {}", guild_id);
        Ok(())
    }

    pub fn get_client(&self) -> Arc<LavalinkClient> {
        Arc::clone(&self.client)
    }
}

impl TypeMapKey for LavalinkManager {
    type Value = Arc<LavalinkManager>;
}