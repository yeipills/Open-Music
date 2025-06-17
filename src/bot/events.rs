use anyhow::Result;
use serenity::{
    async_trait,
    model::id::{ChannelId, GuildId, UserId},
    prelude::Context,
};
use songbird::{Event as VoiceEvent, EventContext, EventHandler as VoiceEventHandler, TrackEvent};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::audio::{player::AudioPlayer, queue::QueueItem};

/// Handler para eventos de tracks de audio
pub struct TrackEndHandler {
    pub guild_id: GuildId,
    pub player: Arc<AudioPlayer>,
    pub handler: Arc<tokio::sync::Mutex<songbird::Call>>,
}

#[async_trait]
impl VoiceEventHandler for TrackEndHandler {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<VoiceEvent> {
        info!("🎵 Track terminó en guild {}", self.guild_id);

        // Reproducir siguiente track en la cola
        if let Err(e) = self
            .player
            .play_next(self.guild_id, self.handler.clone())
            .await
        {
            error!("Error al reproducir siguiente track: {:?}", e);
        }

        None
    }
}

/// Handler para errores de tracks
pub struct TrackErrorHandler {
    pub guild_id: GuildId,
}

#[async_trait]
impl VoiceEventHandler for TrackErrorHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<VoiceEvent> {
        if let EventContext::Track(track_list) = ctx {
            for (state, _handle) in *track_list {
                error!(
                    "❌ Error en track para guild {}: {:?}",
                    self.guild_id, state.playing
                );
            }
        }

        None
    }
}

/// Handler para cuando un track comienza
pub struct TrackStartHandler {
    pub guild_id: GuildId,
    pub ctx: Context,
    pub channel_id: ChannelId,
    pub track_info: QueueItem,
}

#[async_trait]
impl VoiceEventHandler for TrackStartHandler {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<VoiceEvent> {
        info!(
            "▶️ Reproduciendo: {} en guild {}",
            self.track_info.title, self.guild_id
        );

        // Enviar mensaje de "Now Playing"
        if let Err(e) = send_now_playing(&self.ctx, self.channel_id, &self.track_info).await {
            error!("Error al enviar mensaje now playing: {:?}", e);
        }

        None
    }
}

/// Handler para cambios de estado de voz (usuarios entrando/saliendo)
pub struct VoiceStateHandler {
    pub guild_id: GuildId,
    pub bot_user_id: UserId,
}

#[async_trait]
impl VoiceEventHandler for VoiceStateHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<VoiceEvent> {
        if let EventContext::ClientDisconnect(_) = ctx {
            warn!(
                "🔌 Bot desconectado del canal de voz en guild {}",
                self.guild_id
            );
        }

        None
    }
}

/// Handler para recibir audio de usuarios (para futuras features como grabación)
pub struct ReceiveHandler;

#[async_trait]
impl VoiceEventHandler for ReceiveHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<VoiceEvent> {
        if let EventContext::SpeakingStateUpdate(state) = ctx {
            // Por ahora solo loguear, en el futuro se puede implementar grabación
            debug!("📡 Estado de habla cambiado: {:?}", state);
        }

        None
    }
}

/// Handler para reconexiones automáticas
pub struct ReconnectHandler {
    pub guild_id: GuildId,
}

#[async_trait]
impl VoiceEventHandler for ReconnectHandler {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<VoiceEvent> {
        info!("🔄 Reconectando al canal de voz en guild {}", self.guild_id);
        None
    }
}

/// Envía un mensaje de "Now Playing" al canal
async fn send_now_playing(ctx: &Context, channel_id: ChannelId, track: &QueueItem) -> Result<()> {
    let embed = crate::ui::embeds::create_now_playing_embed(track);
    let buttons = crate::ui::buttons::create_player_buttons();

    channel_id
        .send_message(
            &ctx.http,
            serenity::builder::CreateMessage::new()
                .embed(embed)
                .components(buttons),
        )
        .await?;

    Ok(())
}

/// Registra todos los event handlers necesarios para un guild
pub fn register_voice_events(
    handler: &mut songbird::Call,
    guild_id: GuildId,
    player: Arc<AudioPlayer>,
    _ctx: Context,
    _channel_id: ChannelId,
    bot_user_id: UserId,
    handler_arc: Arc<tokio::sync::Mutex<songbird::Call>>,
) {
    // Handler para cuando termina un track
    handler.add_global_event(
        VoiceEvent::Track(TrackEvent::End),
        TrackEndHandler {
            guild_id,
            player: player.clone(),
            handler: handler_arc.clone(),
        },
    );

    // Handler para errores en tracks
    handler.add_global_event(
        VoiceEvent::Track(TrackEvent::Error),
        TrackErrorHandler { guild_id },
    );

    // Handler para desconexiones
    handler.add_global_event(
        VoiceEvent::Core(songbird::events::CoreEvent::ClientDisconnect),
        VoiceStateHandler {
            guild_id,
            bot_user_id,
        },
    );

    // Handler para reconexiones
    handler.add_global_event(
        VoiceEvent::Core(songbird::events::CoreEvent::DriverConnect),
        ReconnectHandler { guild_id },
    );
}
