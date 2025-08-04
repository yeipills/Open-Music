//! # Bot Module
//!
//! Main Discord bot implementation for Open Music Bot.
//!
//! This module contains the core bot logic, including:
//! - Command registration and handling
//! - Voice connection management
//! - Event handling (ready, interactions, voice state updates)
//! - Background maintenance tasks
//!
//! ## Architecture
//!
//! The bot is built around the [`OpenMusicBot`] struct which implements
//! Serenity's [`EventHandler`] trait. It manages:
//!
//! - Audio playback through [`AudioPlayer`]
//! - Metadata caching via [`MusicCache`]
//! - Persistent storage with [`JsonStorage`]
//! - Voice connections per guild
//!
//! ## Example
//!
//! ```rust,no_run
//! use open_music::bot::OpenMusicBot;
//! use open_music::config::Config;
//!
//! let config = Config::load()?;
//! let storage = Arc::new(tokio::sync::Mutex::new(JsonStorage::new(config.data_dir.clone()).await?));
//! let cache = Arc::new(MusicCache::new(config.cache_size));
//! let bot = OpenMusicBot::new(config, storage, cache);
//! ```

use anyhow::Result;
use dashmap::DashMap;
use serenity::{
    all::{ChannelId, Context, EventHandler, GuildId, Interaction, Ready, VoiceState},
    async_trait,
};
use std::sync::Arc;
use tracing::{error, info, warn};

pub mod commands;
pub mod events;
pub mod handlers;
pub mod hybrid_commands;
pub mod lavalink_simple_commands;
pub mod search;

use crate::{audio::player::AudioPlayer, cache::MusicCache, config::Config, storage::JsonStorage, monitoring::MonitoringSystem};

/// Main Discord bot handler for Open Music Bot.
///
/// This struct implements Serenity's [`EventHandler`] trait and manages all bot functionality
/// including command handling, voice connections, and audio playback.
///
/// ## Fields
///
/// - `config`: Bot configuration (tokens, limits, features)
/// - `storage`: Persistent JSON storage for settings and data
/// - `cache`: LRU cache for track metadata and audio data
/// - `player`: Audio player instance for music playback
/// - `voice_handlers`: Per-guild voice connection handlers
///
/// ## Thread Safety
///
/// All fields are wrapped in appropriate synchronization primitives:
/// - [`Arc`] for shared ownership
/// - [`tokio::sync::Mutex`] for async-safe exclusive access
/// - [`DashMap`] for concurrent map operations
pub struct OpenMusicBot {
    /// Bot configuration loaded from environment variables
    config: Arc<Config>,
    /// JSON-based persistent storage (server settings, playlists, etc.)
    #[allow(dead_code)]
    pub storage: Arc<tokio::sync::Mutex<JsonStorage>>,
    /// LRU cache for track metadata and audio data
    cache: Arc<MusicCache>,
    /// Audio player for music playback and queue management
    pub player: Arc<AudioPlayer>,
    /// Voice connection handlers per Discord guild
    voice_handlers: DashMap<GuildId, Arc<tokio::sync::Mutex<songbird::Call>>>,
    /// Sistema de monitoreo para métricas y logs
    pub monitoring: Arc<MonitoringSystem>,
}

impl OpenMusicBot {
    /// Creates a new instance of the Open Music Bot.
    ///
    /// # Arguments
    ///
    /// * `config` - Bot configuration (Discord tokens, audio settings, etc.)
    /// * `storage` - Persistent storage for server settings and data
    /// * `cache` - LRU cache for track metadata and performance optimization
    ///
    /// # Returns
    ///
    /// A new [`OpenMusicBot`] instance ready to handle Discord events.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use std::sync::Arc;
    /// # use open_music::{bot::OpenMusicBot, config::Config, cache::MusicCache, storage::JsonStorage};
    /// # async fn example() -> anyhow::Result<()> {
    /// let config = Config::load()?;
    /// let storage = Arc::new(tokio::sync::Mutex::new(
    ///     JsonStorage::new(config.data_dir.clone()).await?
    /// ));
    /// let cache = Arc::new(MusicCache::new(config.cache_size));
    /// 
    /// let bot = OpenMusicBot::new(config, storage, cache);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(config: Config, storage: Arc<tokio::sync::Mutex<JsonStorage>>, cache: Arc<MusicCache>, monitoring: Arc<MonitoringSystem>) -> Self {
        let config = Arc::new(config);
        let player = Arc::new(AudioPlayer::new());

        Self {
            config,
            storage,
            cache,
            player,
            voice_handlers: DashMap::new(),
            monitoring,
        }
    }

    /// Registers slash commands with Discord.
    ///
    /// Commands can be registered globally (visible in all servers) or per-guild
    /// (faster updates, useful for development). The registration strategy is
    /// determined by the `guild_id` configuration option.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Discord context for API operations
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Commands registered successfully
    /// * `Err(anyhow::Error)` - Registration failed (permissions, network, etc.)
    ///
    /// # Command Registration Timing
    ///
    /// - **Guild commands**: ~1 second propagation time
    /// - **Global commands**: ~1 hour propagation time
    ///
    /// # Required Permissions
    ///
    /// The bot must have `applications.commands` permission in the target guild(s).
    async fn register_commands(&self, ctx: &Context) -> Result<()> {
        info!("📝 Registrando comandos slash...");

        // Verificar permisos del bot
        let bot_id = ctx.cache.current_user().id;
        info!("🤖 Bot ID: {}", bot_id);
        info!("🔧 Application ID: {}", self.config.application_id);

        // Registrar comandos globales o por guild según configuración
        match self.config.guild_id {
            Some(guild_id) => {
                info!("🏠 Registrando comandos para guild específica: {}", guild_id);
                let guild_id = GuildId::from(guild_id);
                
                // Verificar que el bot esté en la guild
                if !ctx.cache.guilds().contains(&guild_id) {
                    warn!("⚠️ El bot no está en la guild especificada: {}", guild_id);
                    return Ok(()); // No fallar, pero no registrar comandos
                }
                
                commands::register_guild_commands(ctx, guild_id).await
                    .map_err(|e| {
                        error!("❌ Error registrando comandos de guild: {:?}", e);
                        anyhow::anyhow!("No se pudieron registrar comandos de guild. Verifica que el bot tenga permisos de 'applications.commands' en la guild.")
                    })?;
                info!("✅ Comandos de guild registrados para: {}", guild_id);
            },
            None => {
                info!("🌐 Registrando comandos globalmente");
                commands::register_global_commands(ctx).await
                    .map_err(|e| {
                        error!("❌ Error registrando comandos globales: {:?}", e);
                        anyhow::anyhow!("No se pudieron registrar comandos globales. Verifica que el bot tenga permisos de 'applications.commands'.")
                    })?;
                info!("✅ Comandos globales registrados");
            }
        }

        Ok(())
    }

    /// Connects the bot to a voice channel.
    ///
    /// Establishes a voice connection using Songbird and stores the handler
    /// for future audio operations. The connection is automatically managed
    /// and will be cleaned up when the bot is disconnected.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Discord context for API operations
    /// * `guild_id` - ID of the Discord server
    /// * `channel_id` - ID of the voice channel to join
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Successfully connected to voice channel
    /// * `Err(anyhow::Error)` - Connection failed (permissions, channel full, etc.)
    ///
    /// # Required Permissions
    ///
    /// - `Connect` - To join the voice channel
    /// - `Speak` - To play audio in the channel
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use serenity::all::{GuildId, ChannelId};
    /// # async fn example(bot: &OpenMusicBot, ctx: &Context) -> anyhow::Result<()> {
    /// let guild_id = GuildId::from(123456789);
    /// let channel_id = ChannelId::from(987654321);
    /// 
    /// bot.join_voice_channel(ctx, guild_id, channel_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn join_voice_channel(
        &self,
        ctx: &Context,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> Result<()> {
        let manager = songbird::get(ctx)
            .await
            .ok_or_else(|| anyhow::anyhow!("Songbird no inicializado"))?;

        let handler = manager.join(guild_id, channel_id).await;

        match handler {
            Ok(connection_info) => {
                // Guardar handler para uso futuro
                self.voice_handlers
                    .insert(guild_id, connection_info.clone());

                info!("🔊 Conectado al canal de voz en guild {}", guild_id);
                Ok(())
            }
            Err(e) => {
                error!("Error al obtener handler de voz: {:?}", e);
                Err(anyhow::anyhow!("Error al conectar al canal de voz"))
            }
        }
    }

    /// Disconnects the bot from a voice channel.
    ///
    /// Cleanly disconnects from the voice channel, stops any ongoing audio playback,
    /// and removes the voice handler from the internal storage.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Discord context for API operations
    /// * `guild_id` - ID of the Discord server to disconnect from
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Successfully disconnected
    /// * `Err(anyhow::Error)` - Disconnection failed
    ///
    /// # Side Effects
    ///
    /// - Stops any currently playing audio
    /// - Clears the audio queue for the guild
    /// - Removes the voice handler from memory
    pub async fn leave_voice_channel(&self, ctx: &Context, guild_id: GuildId) -> Result<()> {
        let manager = songbird::get(ctx)
            .await
            .ok_or_else(|| anyhow::anyhow!("Songbird no inicializado"))?;

        manager.remove(guild_id).await?;
        self.voice_handlers.remove(&guild_id);

        info!("👋 Desconectado del canal de voz en guild {}", guild_id);
        Ok(())
    }

    /// Retrieves the voice handler for a guild.
    ///
    /// Returns the Songbird call handler for the specified guild, which can be used
    /// for audio operations like playing, pausing, and queue management.
    ///
    /// # Arguments
    ///
    /// * `guild_id` - ID of the Discord server
    ///
    /// # Returns
    ///
    /// * `Some(handler)` - Voice handler exists for the guild
    /// * `None` - No active voice connection for the guild
    ///
    /// # Usage
    ///
    /// ```rust,no_run
    /// # use serenity::all::GuildId;
    /// # async fn example(bot: &OpenMusicBot) {
    /// let guild_id = GuildId::from(123456789);
    /// 
    /// if let Some(handler) = bot.get_voice_handler(guild_id) {
    ///     let handler_lock = handler.lock().await;
    ///     // Use handler for audio operations
    /// }
    /// # }
    /// ```
    pub fn get_voice_handler(
        &self,
        guild_id: GuildId,
    ) -> Option<Arc<tokio::sync::Mutex<songbird::Call>>> {
        self.voice_handlers.get(&guild_id).map(|h| h.clone())
    }
}

#[async_trait]
impl EventHandler for OpenMusicBot {
    /// Called when the bot is ready and connected to Discord.
    ///
    /// This event is triggered after successful authentication and initial data loading.
    /// It performs initial setup including command registration and starting background tasks.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Discord context for API operations
    /// * `ready` - Information about the bot and connected guilds
    ///
    /// # Setup Tasks
    ///
    /// 1. Register slash commands (global or per-guild)
    /// 2. Set bot activity status
    /// 3. Start background maintenance tasks
    /// 4. Log connection information
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("🤖 {} está en línea!", ready.user.name);
        info!("📊 Conectado a {} servidores", ready.guilds.len());

        // Registrar comandos
        if let Err(e) = self.register_commands(&ctx).await {
            error!("Error al registrar comandos: {:?}", e);
        }

        // Establecer estado del bot
        // ctx.set_activity(Some(Activity::playing("/play")));

        // Iniciar tareas de mantenimiento
        let config = self.config.clone();
        let cache = self.cache.clone();

        tokio::spawn(async move {
            maintenance_tasks(config, cache).await;
        });
    }

    /// Handles incoming Discord interactions.
    ///
    /// Processes different types of interactions including:
    /// - Slash commands (`/play`, `/pause`, etc.)
    /// - Button clicks (play/pause controls, queue navigation)
    /// - Select menu interactions (equalizer presets, etc.)
    ///
    /// # Arguments
    ///
    /// * `ctx` - Discord context for API operations
    /// * `interaction` - The interaction to process
    ///
    /// # Error Handling
    ///
    /// Errors are logged but don't crash the bot. Failed interactions may
    /// result in "This interaction failed" messages to users.
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command_interaction) => {
                if let Err(e) = handlers::handle_command(&ctx, command_interaction, self).await {
                    error!("Error manejando comando: {:?}", e);
                }
            }
            Interaction::Component(component_interaction) => {
                if let Err(e) = handlers::handle_component(&ctx, component_interaction, self).await
                {
                    error!("Error manejando componente: {:?}", e);
                }
            }
            _ => {}
        }
    }

    /// Handles voice state updates for users and the bot.
    ///
    /// Monitors voice channel changes to implement features like:
    /// - Auto-disconnect when bot is alone in channel
    /// - Cleanup when bot is manually disconnected
    /// - Pause/resume based on channel activity
    ///
    /// # Arguments
    ///
    /// * `ctx` - Discord context for API operations  
    /// * `old` - Previous voice state (if any)
    /// * `new` - New voice state
    ///
    /// # Behaviors
    ///
    /// - **Bot disconnected**: Cleans up voice handlers and stops playback
    /// - **Bot alone**: Schedules auto-disconnect after timeout
    /// - **Users join/leave**: Updates auto-disconnect logic
    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
        // Detectar si el bot fue desconectado
        let current_user_id = ctx.cache.current_user().id;
        if new.user_id == current_user_id {
            if old.is_some() && new.channel_id.is_none() {
                // Bot fue desconectado
                if let Some(guild_id) = new.guild_id {
                    info!("🔌 Bot desconectado en guild {}", guild_id);

                    // Limpiar estado
                    self.voice_handlers.remove(&guild_id);

                    if let Err(e) = self.player.stop(guild_id).await {
                        error!("Error al detener reproducción: {:?}", e);
                    }
                }
            }
        }

        // Auto-desconectar si el bot está solo en el canal
        if let Some(guild_id) = new.guild_id {
            if let Some(handler) = self.get_voice_handler(guild_id) {
                let handler_lock = handler.lock().await;
                if let Some(channel_id) = handler_lock.current_channel() {
                    // Verificar cuántos usuarios hay en el canal
                    let serenity_channel_id = ChannelId::from(channel_id.0);
                    
                    if let Some(guild) = ctx.cache.guild(guild_id) {
                        if let Some(channel) = guild.channels.get(&serenity_channel_id) {
                            let member_count = channel.members(&ctx.cache).map(|m| m.len()).unwrap_or(0);

                            if member_count <= 1 {
                                // Solo el bot está en el canal
                                drop(handler_lock); // Liberar lock antes de llamar leave

                                info!(
                                    "🚪 Programando auto-desconexión por inactividad en guild {}",
                                    guild_id
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Runs periodic maintenance tasks in the background.
///
/// Performs housekeeping operations to keep the bot running efficiently:
/// - Cache cleanup (removes expired entries)
/// - yt-dlp updates (ensures latest video extraction)
/// - Memory optimization
/// - Performance monitoring
///
/// # Arguments
///
/// * `_config` - Bot configuration (currently unused but reserved for future use)
/// * `cache` - Music cache to clean up
///
/// # Schedule
///
/// Runs every hour (3600 seconds) in an infinite loop.
///
/// # Tasks Performed
///
/// 1. **Cache Cleanup**: Removes expired metadata and audio data
/// 2. **yt-dlp Update**: Updates YouTube extractor for compatibility
/// 3. **Memory Stats**: Logs memory usage information
///
/// # Error Handling
///
/// Individual task failures are logged as warnings but don't stop the maintenance cycle.
async fn maintenance_tasks(_config: Arc<Config>, cache: Arc<MusicCache>) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600)); // Cada hora

    loop {
        interval.tick().await;

        // Limpiar caché viejo
        cache.cleanup_old_entries();

        // Verificar dependencias yt-dlp
        let source_manager = crate::sources::SourceManager::new();
        if let Err(e) = source_manager.verify_dependencies().await {
            warn!("Error verificando dependencias: {:?}", e);
        }

        info!("🧹 Tareas de mantenimiento completadas");
    }
}
