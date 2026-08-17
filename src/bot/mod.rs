//! # Bot Module
//!
//! Implementación del bot de Discord.
//!
//! ## Gestión de la conexión de voz
//!
//! No existe ningún mapa propio de conexiones: **songbird es la única fuente de
//! verdad**. Antes había un `DashMap<GuildId, Arc<Mutex<Call>>>` que había que
//! mantener sincronizado a mano, y bastaba con que alguien echara al bot del
//! canal desde Discord para que el mapa dijera "conectado" y todos los comandos
//! fallaran en silencio. Ahora la conexión se consulta siempre con
//! `manager.get(guild_id)`, y `voice_state_update` limpia el estado en cuanto el
//! bot deja un canal, sea quien sea quien lo haya sacado.

use anyhow::Result;
use serenity::{
    all::{ChannelId, Context, EventHandler, GuildId, Interaction, Ready, VoiceState},
    async_trait,
    gateway::ActivityData,
};
use songbird::{Event, Songbird, TrackEvent};
use std::sync::{atomic::AtomicUsize, Arc};
use std::time::Duration;
use tracing::{error, info, warn};

pub mod commands;
pub mod connection;
pub mod handlers;
pub mod search;

use crate::{
    audio::{
        events::{DriverDisconnectHandler, IdleHandler, TrackEndHandler, TrackPlayHandler},
        player::AudioPlayer,
    },
    cache::MusicCache,
    config::Config,
    monitoring::MonitoringSystem,
    storage::JsonStorage,
};

pub struct OpenMusicBot {
    config: Arc<Config>,
    #[allow(dead_code)]
    pub storage: Arc<tokio::sync::Mutex<JsonStorage>>,
    cache: Arc<MusicCache>,
    pub player: Arc<AudioPlayer>,
    pub monitoring: Arc<MonitoringSystem>,
}

impl OpenMusicBot {
    pub fn new(
        config: Config,
        storage: Arc<tokio::sync::Mutex<JsonStorage>>,
        cache: Arc<MusicCache>,
        monitoring: Arc<MonitoringSystem>,
        manager: Arc<Songbird>,
    ) -> Self {
        let config = Arc::new(config);
        let player = Arc::new(AudioPlayer::new(config.default_volume, manager));

        Self {
            config,
            storage,
            cache,
            player,
            monitoring,
        }
    }

    /// Registra los comandos slash (globales o de una guild concreta).
    async fn register_commands(&self, ctx: &Context) -> Result<()> {
        info!("Registrando comandos slash...");

        match self.config.guild_id {
            Some(guild_id) => {
                let guild_id = GuildId::from(guild_id);
                if !ctx.cache.guilds().contains(&guild_id) {
                    warn!("El bot no está en la guild configurada: {}", guild_id);
                    return Ok(());
                }
                commands::register_guild_commands(ctx, guild_id).await?;
                info!("Comandos registrados en la guild {}", guild_id);
            }
            None => {
                commands::register_global_commands(ctx).await?;
                info!("Comandos globales registrados");
            }
        }

        Ok(())
    }

    /// Conexión de voz activa de la guild, consultada a songbird.
    pub fn get_voice_handler(
        &self,
        guild_id: GuildId,
    ) -> Option<Arc<tokio::sync::Mutex<songbird::Call>>> {
        self.player.call(guild_id)
    }

    /// Entra al canal de voz y deja registrados los handlers de la sesión.
    ///
    /// Los eventos se registran **aquí y sólo aquí**, después de un
    /// `remove_all_global_events()`: si el bot vuelve a entrar a un canal, no se
    /// acumulan handlers duplicados de la conexión anterior (cada duplicado
    /// significaba un aviso repetido y un contador de inactividad de más).
    pub async fn join_voice_channel(
        &self,
        ctx: &Context,
        guild_id: GuildId,
        channel_id: ChannelId,
        text_channel_id: ChannelId,
    ) -> Result<()> {
        let manager = self.player.manager();

        // Si queda un `Call` de una sesión muerta, `join` puede agotar los 10 s de
        // timeout del gateway antes de fallar. En ese caso se descarta y se
        // reintenta una vez, que es lo que de verdad repara la conexión.
        let call = match manager.join(guild_id, channel_id).await {
            Ok(call) => call,
            Err(first_error) => {
                warn!("Reintento de conexión a voz en guild {guild_id}: {first_error}");
                manager.remove(guild_id).await.ok();
                manager
                    .join(guild_id, channel_id)
                    .await
                    .map_err(|e| anyhow::anyhow!("No pude entrar al canal de voz: {e}"))?
            }
        };

        let idle_limit = {
            let storage = self.storage.lock().await;
            storage.get_auto_leave_timeout(guild_id.get()) as usize
        };

        {
            let mut handler = call.lock().await;

            // Bitrate del encoder Opus (el tope real lo fija el canal de voz).
            handler.set_bitrate(songbird::driver::Bitrate::Bits(
                self.config.opus_bitrate as i32,
            ));

            handler.remove_all_global_events();

            handler.add_global_event(
                Event::Track(TrackEvent::Play),
                TrackPlayHandler {
                    guild_id,
                    player: Arc::downgrade(&self.player),
                },
            );

            handler.add_global_event(
                Event::Track(TrackEvent::End),
                TrackEndHandler {
                    guild_id,
                    player: Arc::downgrade(&self.player),
                },
            );

            handler.add_global_event(
                Event::Core(songbird::CoreEvent::DriverDisconnect),
                DriverDisconnectHandler {
                    guild_id,
                    manager: manager.clone(),
                    player: Arc::downgrade(&self.player),
                },
            );

            handler.add_global_event(
                Event::Periodic(Duration::from_secs(1), None),
                IdleHandler {
                    guild_id,
                    http: ctx.http.clone(),
                    manager: manager.clone(),
                    channel_id: text_channel_id,
                    player: Arc::downgrade(&self.player),
                    limit: idle_limit,
                    count: Arc::new(AtomicUsize::new(0)),
                },
            );
        }

        info!(
            "Conectado a voz en guild {} (Opus {} kbps, inactividad {}s)",
            guild_id,
            self.config.opus_bitrate / 1000,
            idle_limit
        );
        Ok(())
    }

    /// Sale del canal de voz y olvida el estado de la guild.
    pub async fn leave_voice_channel(&self, _ctx: &Context, guild_id: GuildId) -> Result<()> {
        self.player.manager().remove(guild_id).await.ok();
        self.player.forget_guild(guild_id);

        info!("Desconectado de voz en guild {}", guild_id);
        Ok(())
    }
}

#[async_trait]
impl EventHandler for OpenMusicBot {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} está en línea!", ready.user.name);
        info!("Conectado a {} servidores", ready.guilds.len());

        if let Err(e) = self.register_commands(&ctx).await {
            error!("Error al registrar comandos: {:?}", e);
        }

        ctx.set_activity(Some(ActivityData::listening("/play")));

        let config = self.config.clone();
        let cache = self.cache.clone();
        tokio::spawn(async move {
            maintenance_tasks(config, cache).await;
        });
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => {
                handlers::dispatch_command(&ctx, command, self).await;
            }
            Interaction::Component(component) => {
                if let Err(e) = handlers::handle_component(&ctx, component, self).await {
                    error!("Error manejando componente: {:?}", e);
                }
            }
            _ => {}
        }
    }

    /// Reacciona a los cambios de estado de voz.
    ///
    /// Dos casos, ambos del propio bot:
    ///
    /// - **Salió de un canal** (lo echaron, lo movieron o se fue): se libera la
    ///   conexión en songbird y se olvida el estado de la guild. Sin esto queda
    ///   un `Call` fantasma que hace fallar todo comando posterior.
    /// - **Entró a un canal**: se auto-ensordece, que es lo educado y ahorra
    ///   ancho de banda de recepción.
    ///
    /// Además, si al bot lo dejan solo en el canal se programa la salida; el
    /// caso de "nadie pone música" lo cubre el [`IdleHandler`].
    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
        let bot_id = ctx.cache.current_user().id;

        if new.user_id == bot_id {
            let Some(guild_id) = new.guild_id else {
                return;
            };

            match new.channel_id {
                Some(_) => {
                    if !new.deaf {
                        let edit = serenity::builder::EditMember::new().deafen(true);
                        if let Err(e) = guild_id.edit_member(&ctx.http, bot_id, edit).await {
                            warn!("No se pudo auto-ensordecer: {e}");
                        }
                    }
                }
                None => {
                    info!("El bot dejó el canal de voz en guild {}", guild_id);
                    self.player.manager().remove(guild_id).await.ok();
                    self.player.forget_guild(guild_id);
                }
            }
            return;
        }

        // Un usuario se movió: comprobar si el bot quedó solo.
        let Some(guild_id) = new.guild_id.or_else(|| old.as_ref().and_then(|o| o.guild_id)) else {
            return;
        };
        self.check_alone_in_channel(&ctx, guild_id).await;
    }
}

impl OpenMusicBot {
    /// Si el bot quedó solo en su canal, programa la desconexión.
    ///
    /// El temporizador vuelve a comprobar la soledad antes de irse, así que si
    /// alguien entra mientras tanto no pasa nada.
    async fn check_alone_in_channel(&self, ctx: &Context, guild_id: GuildId) {
        let Some(call) = self.player.call(guild_id) else {
            return;
        };
        let Some(bot_channel) = call.lock().await.current_channel() else {
            return;
        };
        let bot_channel = ChannelId::from(bot_channel.0);

        if !is_alone(ctx, guild_id, bot_channel) {
            return;
        }

        let (timeout_secs, enabled) = {
            let storage = self.storage.lock().await;
            (
                storage.get_auto_leave_timeout(guild_id.get()),
                storage.get_auto_leave_empty(guild_id.get()),
            )
        };
        if !enabled {
            return;
        }

        info!(
            "Bot solo en el canal, saldré en {}s si nadie vuelve (guild {})",
            timeout_secs, guild_id
        );

        let ctx = ctx.clone();
        let player = self.player.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(timeout_secs)).await;

            // Puede haber vuelto gente, o el bot haber cambiado de canal.
            let still_here = player
                .call(guild_id)
                .map(|c| c.try_lock().map(|c| c.current_channel().is_some()).unwrap_or(true))
                .unwrap_or(false);

            if !still_here || !is_alone(&ctx, guild_id, bot_channel) {
                return;
            }

            player.manager().remove(guild_id).await.ok();
            player.forget_guild(guild_id);
            info!("Salí del canal: me quedé solo (guild {})", guild_id);
        });
    }
}

/// `true` si en el canal no queda nadie más que el bot.
fn is_alone(ctx: &Context, guild_id: GuildId, channel_id: ChannelId) -> bool {
    ctx.cache
        .guild(guild_id)
        .map(|guild| {
            guild
                .voice_states
                .values()
                .filter(|state| state.channel_id == Some(channel_id))
                .filter(|state| state.user_id != ctx.cache.current_user().id)
                .count()
                == 0
        })
        .unwrap_or(false)
}

/// Mantenimiento periódico: limpieza de caché y verificación de dependencias.
async fn maintenance_tasks(_config: Arc<Config>, cache: Arc<MusicCache>) {
    let mut interval = tokio::time::interval(Duration::from_secs(3600));

    loop {
        interval.tick().await;
        cache.cleanup_old_entries();

        let source_manager = crate::sources::SourceManager::new();
        if let Err(e) = source_manager.verify_dependencies().await {
            warn!("Error verificando dependencias: {:?}", e);
        }

        info!("Tareas de mantenimiento completadas");
    }
}
