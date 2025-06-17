use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    // Discord
    pub discord_token: String,
    pub application_id: u64,
    pub guild_id: Option<u64>, // Para comandos de desarrollo

    // Audio
    pub default_volume: f32,
    pub max_queue_size: usize,
    pub audio_cache_size: usize,
    pub opus_bitrate: u32,
    pub frame_size: usize,

    // Rendimiento
    pub cache_size: usize,
    pub worker_threads: usize,
    pub max_playlist_size: usize,

    // Paths
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,

    // APIs (Opcionales)
    pub spotify_client_id: Option<String>,
    pub spotify_client_secret: Option<String>,
    pub tidal_api_key: Option<String>,
    pub soundcloud_client_id: Option<String>,

    // Límites
    pub max_song_duration: u64,   // En segundos
    pub rate_limit_per_user: u32, // Comandos por minuto

    // Features
    pub enable_equalizer: bool,
    pub enable_effects: bool,
    pub enable_lyrics: bool,
    pub enable_autoplay: bool,
}

impl Config {
    pub fn load() -> Result<Self> {
        dotenv::dotenv().ok();

        let config = Self {
            // Discord
            discord_token: std::env::var("DISCORD_TOKEN")?,
            application_id: std::env::var("APPLICATION_ID")?.parse()?,
            guild_id: std::env::var("GUILD_ID").ok().and_then(|s| s.parse().ok()),

            // Audio (valores optimizados)
            default_volume: std::env::var("DEFAULT_VOLUME")
                .unwrap_or_else(|_| "0.5".to_string())
                .parse()?,
            max_queue_size: std::env::var("MAX_QUEUE_SIZE")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()?,
            audio_cache_size: std::env::var("AUDIO_CACHE_SIZE")
                .unwrap_or_else(|_| "50".to_string())
                .parse()?,
            opus_bitrate: std::env::var("OPUS_BITRATE")
                .unwrap_or_else(|_| "128000".to_string()) // 128kbps
                .parse()?,
            frame_size: std::env::var("FRAME_SIZE")
                .unwrap_or_else(|_| "960".to_string()) // 20ms @ 48kHz
                .parse()?,

            // Rendimiento
            cache_size: std::env::var("CACHE_SIZE")
                .unwrap_or_else(|_| "100".to_string())
                .parse()?,
            worker_threads: match std::env::var("WORKER_THREADS") {
                Ok(val) if !val.trim().is_empty() => val.parse()?,
                _ => num_cpus::get(),
            },
            max_playlist_size: std::env::var("MAX_PLAYLIST_SIZE")
                .unwrap_or_else(|_| "100".to_string())
                .parse()?,

            // Paths
            data_dir: std::env::var("DATA_DIR")
                .unwrap_or_else(|_| "/app/data".to_string())
                .into(),
            cache_dir: std::env::var("CACHE_DIR")
                .unwrap_or_else(|_| "/app/cache".to_string())
                .into(),

            // APIs
            spotify_client_id: std::env::var("SPOTIFY_CLIENT_ID").ok(),
            spotify_client_secret: std::env::var("SPOTIFY_CLIENT_SECRET").ok(),
            tidal_api_key: std::env::var("TIDAL_API_KEY").ok(),
            soundcloud_client_id: std::env::var("SOUNDCLOUD_CLIENT_ID").ok(),

            // Límites
            max_song_duration: std::env::var("MAX_SONG_DURATION")
                .unwrap_or_else(|_| "3600".to_string()) // 1 hora
                .parse()?,
            rate_limit_per_user: std::env::var("RATE_LIMIT_PER_USER")
                .unwrap_or_else(|_| "20".to_string())
                .parse()?,

            // Features
            enable_equalizer: std::env::var("ENABLE_EQUALIZER")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
            enable_effects: std::env::var("ENABLE_EFFECTS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
            enable_lyrics: std::env::var("ENABLE_LYRICS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()?,
            enable_autoplay: std::env::var("ENABLE_AUTOPLAY")
                .unwrap_or_else(|_| "false".to_string())
                .parse()?,
        };

        // Crear directorios si no existen
        std::fs::create_dir_all(&config.data_dir)?;
        std::fs::create_dir_all(&config.cache_dir)?;

        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.default_volume < 0.0 || self.default_volume > 2.0 {
            anyhow::bail!("Volumen debe estar entre 0.0 y 2.0");
        }

        if self.opus_bitrate > 510000 {
            anyhow::bail!("Bitrate Opus no puede exceder 510kbps");
        }

        Ok(())
    }
}
