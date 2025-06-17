use anyhow::Result;
use dashmap::DashMap;
use serenity::{
    all::{ChannelId, Context, EventHandler, GuildId, Interaction, Ready, VoiceState},
    async_trait,
};
use sqlx::SqlitePool;
use std::sync::Arc;
use tracing::{error, info, warn};

pub mod commands;
pub mod events;
pub mod handlers;
pub mod search;

use crate::{audio::player::AudioPlayer, cache::MusicCache, config::Config};

/// Handler principal del bot
pub struct OpenMusicBot {
    config: Arc<Config>,
    db_pool: SqlitePool,
    cache: Arc<MusicCache>,
    pub player: Arc<AudioPlayer>,
    voice_handlers: DashMap<GuildId, Arc<tokio::sync::Mutex<songbird::Call>>>,
}

impl OpenMusicBot {
    pub fn new(config: Config, db_pool: SqlitePool, cache: Arc<MusicCache>) -> Self {
        let config = Arc::new(config);
        let player = Arc::new(AudioPlayer::new());

        Self {
            config,
            db_pool,
            cache,
            player,
            voice_handlers: DashMap::new(),
        }
    }

    /// Registra comandos slash
    async fn register_commands(&self, ctx: &Context) -> Result<()> {
        info!("📝 Registrando comandos slash...");

        // Registrar comandos globales o por guild según configuración
        if let Some(guild_id) = self.config.guild_id {
            // Modo desarrollo: registrar en guild específica
            commands::register_guild_commands(ctx, GuildId::from(guild_id)).await?;
        } else {
            // Modo producción: registrar globalmente
            commands::register_global_commands(ctx).await?;
        }

        info!("✅ Comandos registrados exitosamente");
        Ok(())
    }

    /// Maneja la conexión a un canal de voz
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

    /// Desconecta del canal de voz
    pub async fn leave_voice_channel(&self, ctx: &Context, guild_id: GuildId) -> Result<()> {
        let manager = songbird::get(ctx)
            .await
            .ok_or_else(|| anyhow::anyhow!("Songbird no inicializado"))?;

        manager.remove(guild_id).await?;
        self.voice_handlers.remove(&guild_id);

        info!("👋 Desconectado del canal de voz en guild {}", guild_id);
        Ok(())
    }

    /// Obtiene el handler de voz para una guild
    pub fn get_voice_handler(
        &self,
        guild_id: GuildId,
    ) -> Option<Arc<tokio::sync::Mutex<songbird::Call>>> {
        self.voice_handlers.get(&guild_id).map(|h| h.clone())
    }
}

#[async_trait]
impl EventHandler for OpenMusicBot {
    /// Evento cuando el bot está listo
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

    /// Manejo de interacciones (comandos slash, botones, etc.)
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

    /// Evento de actualización de estado de voz
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

/// Tareas de mantenimiento periódicas
async fn maintenance_tasks(_config: Arc<Config>, cache: Arc<MusicCache>) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600)); // Cada hora

    loop {
        interval.tick().await;

        // Limpiar caché viejo
        cache.cleanup_old_entries();

        // Actualizar yt-dlp
        if let Err(e) = crate::sources::youtube::YouTubeClient::update_ytdlp().await {
            warn!("Error al actualizar yt-dlp: {:?}", e);
        }

        info!("🧹 Tareas de mantenimiento completadas");
    }
}
