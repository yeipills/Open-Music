//! Handlers de eventos de songbird.
//!
//! Son tres, y cada uno tiene una única responsabilidad:
//!
//! - [`TrackPlayHandler`] — al empezar una pista, le aplica las preferencias de
//!   la guild (volumen y repetición). Se hace al **arrancar** y no al terminar
//!   la anterior para no depender del orden en que corren los handlers.
//! - [`TrackEndHandler`] — guarda la pista terminada en el historial y, si el
//!   modo es `Queue`, la vuelve a encolar al final.
//! - [`IdleHandler`] — desconecta tras un rato sin reproducir nada.
//!
//! Nótese lo que **no** hay: nada que avance la cola. De eso se encarga la cola
//! nativa de songbird, que ya está suscrita a `TrackEvent::End` de cada pista.

use serenity::{
    async_trait,
    http::Http,
    model::id::{ChannelId, GuildId},
};
use songbird::{
    events::context_data::DisconnectReason, tracks::PlayMode, Event, EventContext, EventHandler,
    Songbird,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex, Weak,
};
use std::time::{Duration, Instant};
use tracing::{info, warn};

use crate::audio::{
    player::AudioPlayer,
    queue::{meta_of, LoopMode},
};

/// Aplica volumen y repetición a la pista que acaba de empezar.
pub struct TrackPlayHandler {
    pub guild_id: GuildId,
    pub player: Weak<AudioPlayer>,
}

#[async_trait]
impl EventHandler for TrackPlayHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        let EventContext::Track(tracks) = ctx else {
            return None;
        };
        let player = self.player.upgrade()?;

        for (_state, handle) in tracks.iter() {
            let handle: &songbird::tracks::TrackHandle = handle;

            if let Some(volume) = player.get_volume(self.guild_id).await {
                handle.set_volume(volume).ok();
            }

            match player.loop_mode(self.guild_id) {
                LoopMode::Track => {
                    handle.enable_loop().ok();
                }
                _ => {
                    handle.disable_loop().ok();
                }
            }

            info!("Sonando: {} (guild {})", meta_of(handle).title, self.guild_id);
        }

        player.preload_next(self.guild_id);
        None
    }
}

/// Historial y repetición de cola.
///
/// **Ojo con el evento**: songbird emite `TrackEvent::End` tanto cuando una
/// pista termina sola (`PlayMode::End`) como cuando alguien la detiene
/// (`PlayMode::Stop`, que es lo que hacen `/skip`, `/clear` o `/stop`) o cuando
/// falla (`PlayMode::Errored`). Sin distinguirlos, vaciar la cola con la
/// repetición activada reencolaría todo lo que se acaba de borrar. Por eso aquí
/// sólo cuenta el final natural; el historial de las pistas que se saltan lo
/// registra el propio [`AudioPlayer`] al saltarlas.
pub struct TrackEndHandler {
    pub guild_id: GuildId,
    pub player: Weak<AudioPlayer>,
}

#[async_trait]
impl EventHandler for TrackEndHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        let EventContext::Track(tracks) = ctx else {
            return None;
        };
        let player = self.player.upgrade()?;

        for (state, handle) in tracks.iter() {
            if !matches!(state.playing, PlayMode::End) {
                continue;
            }

            let handle: &songbird::tracks::TrackHandle = handle;
            let meta = (*meta_of(handle)).clone();

            if player.loop_mode(self.guild_id) == LoopMode::Queue {
                // Se reencola una pista nueva a partir de la misma fuente: como
                // el input es perezoso, esto no reabre ningún proceso hasta que
                // le toque sonar otra vez.
                if let Err(e) = player.play(self.guild_id, meta.source.clone()).await {
                    warn!("No se pudo reencolar «{}» en bucle: {:?}", meta.title, e);
                }
            }

            player.push_history(self.guild_id, meta);
        }

        None
    }
}

/// Limpia la conexión cuando el driver de voz se cae.
///
/// Songbird documenta que un `DriverDisconnect` con `reason` distinto de `None`
/// —sesión expirada, WebSocket cerrado por Discord, timeout— *requiere que la
/// aplicación lo gestione*: songbird ya agotó su estrategia de reconexión y deja
/// el `Call` registrado en el manager, pero muerto.
///
/// Ese `Call` zombi es el origen del bug de "lo saco del canal, lo vuelvo a
/// llamar y se queda mudo": `manager.get()` seguía devolviéndolo, así que el bot
/// se creía conectado y encolaba música en un driver que ya no existía. Sacarlo
/// del manager obliga a que el siguiente comando entre al canal de cero.
pub struct DriverDisconnectHandler {
    pub guild_id: GuildId,
    pub manager: Arc<Songbird>,
    pub player: Weak<AudioPlayer>,
}

#[async_trait]
impl EventHandler for DriverDisconnectHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        let EventContext::DriverDisconnect(data) = ctx else {
            return None;
        };

        // Una desconexión intencionada no es una caída. Songbird la señala de dos
        // formas distintas —`None`, y `Some(Requested)` cuando viene de
        // `Driver::leave`— y hay que descartar ambas: tratar un `/leave` o un
        // cambio de canal como avería cortaría la música al mover el bot.
        let reason = data.reason?;
        if reason == DisconnectReason::Requested {
            return None;
        }

        warn!(
            "Conexión de voz caída en guild {} ({:?}, {:?}); libero el Call",
            self.guild_id, data.kind, reason
        );

        let manager = self.manager.clone();
        let guild_id = self.guild_id;
        let player = self.player.clone();

        // En tarea aparte para no bloquear el bucle de eventos del driver
        // mientras se cierra la conexión.
        tokio::spawn(async move {
            manager.remove(guild_id).await.ok();
            if let Some(player) = player.upgrade() {
                player.forget_guild(guild_id);
            }
        });

        None
    }
}

/// Avisa en el canal de texto cuando una pista no se puede reproducir.
///
/// Sin esto, un fallo de descarga es invisible: songbird marca la pista como
/// `Errored`, la cola pasa a la siguiente y quien pidió la canción sólo percibe
/// silencio. El caso real que motivó el handler fue YouTube devolviendo una
/// respuesta degradada, con el error enterrado en las trazas del servidor.
///
/// Se limita a un aviso por minuto: cuando el problema es la conexión con
/// YouTube fallan todas las pistas de la cola en cadena, y no tiene sentido
/// publicar quince mensajes iguales.
pub struct TrackErrorHandler {
    pub http: Arc<Http>,
    pub channel_id: ChannelId,
    pub last_notice: Arc<Mutex<Option<Instant>>>,
}

impl TrackErrorHandler {
    const SILENCIO_ENTRE_AVISOS: Duration = Duration::from_secs(60);

    fn deberia_avisar(&self) -> bool {
        let Ok(mut ultimo) = self.last_notice.lock() else {
            return false;
        };
        let ahora = Instant::now();
        let toca = ultimo
            .map(|t| ahora.duration_since(t) >= Self::SILENCIO_ENTRE_AVISOS)
            .unwrap_or(true);
        if toca {
            *ultimo = Some(ahora);
        }
        toca
    }
}

#[async_trait]
impl EventHandler for TrackErrorHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        let EventContext::Track(tracks) = ctx else {
            return None;
        };

        for (state, handle) in tracks.iter() {
            let PlayMode::Errored(error) = &state.playing else {
                continue;
            };
            let handle: &songbird::tracks::TrackHandle = handle;
            let titulo = meta_of(handle).title.clone();

            warn!("No se pudo reproducir «{}»: {:?}", titulo, error);

            if !self.deberia_avisar() {
                continue;
            }

            let aviso = format!(
                "No pude reproducir **{titulo}** y paso a la siguiente. \
                 Si se repite con todas, es que YouTube está rechazando las \
                 descargas: suele arreglarse renovando las cookies."
            );
            if let Err(e) = self.channel_id.say(&self.http, aviso).await {
                warn!("No se pudo avisar del fallo de reproducción: {e}");
            }
        }

        None
    }
}

/// Desconecta del canal de voz tras `limit` segundos sin reproducir.
///
/// Se registra como `Event::Periodic(1s)`: en cada tick mira si hay alguna
/// pista sonando y, si la hay, reinicia el contador. Es más fiable que programar
/// un `sleep` al vaciarse la cola, porque no hay forma de que quede una tarea
/// huérfana desconectando al bot en mitad de una canción.
pub struct IdleHandler {
    pub guild_id: GuildId,
    pub http: Arc<Http>,
    pub manager: Arc<Songbird>,
    pub channel_id: ChannelId,
    pub player: Weak<AudioPlayer>,
    pub limit: usize,
    pub count: Arc<AtomicUsize>,
}

#[async_trait]
impl EventHandler for IdleHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        let EventContext::Track(track_list) = ctx else {
            return None;
        };

        // La lista no viene ordenada, así que hay que mirarla entera.
        let playing = track_list
            .iter()
            .any(|track| matches!(track.0.playing, PlayMode::Play));

        if playing {
            self.count.store(0, Ordering::Relaxed);
            return None;
        }

        if self.count.fetch_add(1, Ordering::Relaxed) < self.limit {
            return None;
        }

        if self.manager.remove(self.guild_id).await.is_ok() {
            info!("Desconectado por inactividad (guild {})", self.guild_id);

            if let Some(player) = self.player.upgrade() {
                player.forget_guild(self.guild_id);
            }

            if let Err(e) = self
                .channel_id
                .say(&self.http, "Me voy por inactividad. ¡Volvé cuando quieras!")
                .await
            {
                warn!("No se pudo avisar de la desconexión por inactividad: {e}");
            }
        }

        None
    }
}
