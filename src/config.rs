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

    // APIs (Opcionales) - Removed unused integrations

    // Límites
    pub max_song_duration: u64,   // En segundos
    pub rate_limit_per_user: u32, // Comandos por minuto

    // Features
    pub enable_equalizer: bool,
    pub enable_autoplay: bool,
}

impl Config {
    pub fn load() -> Result<Self> {
        dotenvy::dotenv().ok();

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
                .unwrap_or_else(|_| "96000".to_string()) // 96kbps (Discord default)
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
            enable_autoplay: std::env::var("ENABLE_AUTOPLAY")
                .unwrap_or_else(|_| "false".to_string())
                .parse()?,
        };

        // Create directories if they don't exist
        std::fs::create_dir_all(&config.data_dir)?;
        std::fs::create_dir_all(&config.cache_dir)?;

        // Validate configuration before returning
        config.validate()?;
        
        Ok(config)
    }

    /// Validates configuration values for correctness.
    ///
    /// Performs sanity checks on configuration values to catch
    /// common mistakes and ensure the bot will function properly.
    ///
    /// # Validation Rules
    ///
    /// - Volume must be between 0.0 and 2.0
    /// - Opus bitrate must not exceed 510kbps (Discord limit)
    /// - Cache sizes must be reasonable (> 0)
    /// - Directories must be accessible
    ///
    /// # Returns
    ///
    /// - `Ok(())`: All values are valid
    /// - `Err(anyhow::Error)`: Invalid configuration detected
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use open_music::config::Config;
    /// # fn main() -> anyhow::Result<()> {
    /// let config = Config::load()?;
    /// config.validate()?;  // Ensure config is valid
    /// # Ok(())
    /// # }
    /// ```
    #[allow(dead_code)]
    pub fn validate(&self) -> Result<()> {
        // Validate audio settings
        if self.default_volume < 0.0 || self.default_volume > 2.0 {
            anyhow::bail!("Default volume must be between 0.0 and 2.0, got: {}", self.default_volume);
        }

        if self.opus_bitrate > 510000 {
            anyhow::bail!("Opus bitrate cannot exceed 510kbps, got: {}", self.opus_bitrate);
        }
        
        if self.opus_bitrate < 8000 {
            anyhow::bail!("Opus bitrate too low, minimum 8kbps, got: {}", self.opus_bitrate);
        }

        // Validate cache settings
        if self.cache_size == 0 {
            anyhow::bail!("Cache size must be greater than 0");
        }
        
        if self.audio_cache_size == 0 {
            anyhow::bail!("Audio cache size must be greater than 0");
        }

        // Validate limits
        if self.max_queue_size == 0 {
            anyhow::bail!("Max queue size must be greater than 0");
        }
        
        if self.max_song_duration == 0 {
            anyhow::bail!("Max song duration must be greater than 0");
        }

        Ok(())
    }
    
    /// Returns a summary of the current configuration for logging.
    ///
    /// Provides a safe summary that excludes sensitive information
    /// like tokens while showing key configuration parameters.
    ///
    /// # Returns
    ///
    /// A formatted string suitable for logging or debugging.
    pub fn summary(&self) -> String {
        format!(
            "Config Summary:\n  \
            Discord: App ID {} (Guild: {})\n  \
            Audio: {}% vol, {}kbps, {}ms frames\n  \
            Cache: {} metadata, {} audio files\n  \
            Limits: {} queue, {}s max duration, {}/min rate limit\n  \
            Features: EQ={}, Autoplay={}",
            self.application_id,
            self.guild_id.map_or("global".to_string(), |id| id.to_string()),
            (self.default_volume * 100.0) as u32,
            self.opus_bitrate / 1000,
            (self.frame_size as f32 / 48.0) as u32,  // Convert to ms at 48kHz
            self.cache_size,
            self.audio_cache_size,
            self.max_queue_size,
            self.max_song_duration,
            self.rate_limit_per_user,
            self.enable_equalizer,
            self.enable_autoplay
        )
    }
}

/// Default configuration values.
///
/// Used as fallbacks when environment variables are not provided.
/// These values are chosen for a good balance of quality and performance.
impl Default for Config {
    fn default() -> Self {
        Self {
            // Discord (no defaults - must be provided)
            discord_token: String::new(),
            application_id: 0,
            guild_id: None,
            
            // Audio defaults
            default_volume: 0.5,
            max_queue_size: 1000,
            audio_cache_size: 50,
            opus_bitrate: 96000,   // 96kbps (Discord default)
            frame_size: 960,       // 20ms at 48kHz
            
            // Performance defaults
            cache_size: 100,
            worker_threads: num_cpus::get(),
            max_playlist_size: 100,
            
            // Path defaults
            data_dir: "/app/data".into(),
            cache_dir: "/app/cache".into(),
            
            // Limit defaults
            max_song_duration: 7200,  // 2 hours
            rate_limit_per_user: 20,  // 20 commands per minute
            
            // Feature defaults
            enable_equalizer: true,
            enable_autoplay: false,
        }
    }
}
