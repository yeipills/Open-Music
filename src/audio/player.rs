//! Reproductor: una capa fina sobre la cola nativa de songbird.
//!
//! ## Principio de diseño
//!
//! La versión anterior mantenía una cola propia, un `current_track` por guild,
//! un contador de generación, un `advance_lock` y un bucle de avance con hasta
//! 10 reintentos. Todo eso existía para reimplementar algo que songbird ya trae
//! resuelto: [`songbird::tracks::TrackQueue`] avanza sola al recibir
//! `TrackEvent::End` —incluido el caso de una pista que falla, porque una pista
//! en estado `Errored` también emite `End`— y descarta las que no se pueden
//! reproducir.
//!
//! Así que aquí no hay estado de reproducción. La cola de songbird es la única
//! fuente de verdad; este módulo sólo:
//!
//! - construye pistas perezosas con sus metadatos adjuntos ([`build_track`]),
//! - guarda las **preferencias** por guild que songbird no conoce (volumen,
//!   modo de repetición, aleatorio, historial),
//! - y traduce las operaciones del bot a manipulaciones de esa cola.
//!
//! El único punto delicado es saltar de pista, documentado en
//! [`force_skip_top_track`].

use anyhow::Result;
use dashmap::DashMap;
use rand::seq::SliceRandom;
use serenity::model::id::{GuildId, UserId};
use songbird::{
    tracks::{PlayMode, Track, TrackHandle},
    Call, Songbird,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::{
    audio::{
        effects::{AudioEffects, EqualizerPreset},
        queue::{meta_of, LoopMode, QueueInfo, QueueItem},
    },
    sources::{lazy::LazyFfmpegSource, TrackSource},
};

/// Máximo de pistas pendientes por guild.
const MAX_QUEUE_LEN: usize = 1000;
/// Máximo de pistas recordadas para `/previous`.
const MAX_HISTORY: usize = 50;

/// Preferencias por guild que la cola de songbird no modela.
struct PlayerInner {
    /// Gestor de voz: **la** fuente de verdad de las conexiones. Tener el
    /// manager aquí evita duplicar un mapa de `Call` que se desincronice.
    manager: Arc<Songbird>,
    effects: Arc<AudioEffects>,
    /// Volumen efectivo por guild (0.0–2.0), aplicado a cada pista nueva para
    /// que el ajuste sobreviva al cambio de canción.
    volumes: DashMap<GuildId, f32>,
    default_volume: f32,
    loop_modes: DashMap<GuildId, LoopMode>,
    shuffle: DashMap<GuildId, bool>,
    /// Pistas ya reproducidas, para `/previous`.
    history: DashMap<GuildId, Vec<QueueItem>>,
}

impl PlayerInner {
    fn effective_volume(&self, guild_id: GuildId) -> f32 {
        self.volumes
            .get(&guild_id)
            .map(|v| *v)
            .unwrap_or(self.default_volume)
    }
}

pub struct AudioPlayer {
    inner: Arc<PlayerInner>,
}

impl AudioPlayer {
    pub fn new(default_volume: f32, manager: Arc<Songbird>) -> Self {
        Self {
            inner: Arc::new(PlayerInner {
                manager,
                effects: Arc::new(AudioEffects::new()),
                volumes: DashMap::new(),
                default_volume: default_volume.clamp(0.0, 2.0),
                loop_modes: DashMap::new(),
                shuffle: DashMap::new(),
                history: DashMap::new(),
            }),
        }
    }

    /// Conexión de voz activa de la guild, si la hay.
    pub fn call(&self, guild_id: GuildId) -> Option<Arc<Mutex<Call>>> {
        self.inner.manager.get(guild_id)
    }

    pub fn manager(&self) -> Arc<Songbird> {
        self.inner.manager.clone()
    }

    /// Instantánea de la cola real (`[actual, siguiente, ...]`).
    pub async fn handles(&self, guild_id: GuildId) -> Vec<TrackHandle> {
        match self.call(guild_id) {
            Some(call) => call.lock().await.queue().current_queue(),
            None => Vec::new(),
        }
    }

    // ---------------------------------------------------------------- encolar

    /// Construye la pista perezosa de una fuente, con sus metadatos adjuntos.
    ///
    /// El [`LazyFfmpegSource`] no lanza ningún proceso hasta que songbird lo
    /// pide, así que encolar 200 temas cuesta lo mismo que encolar uno.
    fn build_track(&self, guild_id: GuildId, source: TrackSource) -> Track {
        let effects = self.inner.effects.clone();
        let filter = Arc::new(move || effects.build_filter(guild_id));

        let lazy = LazyFfmpegSource::new(&source, filter);
        let meta = Arc::new(QueueItem::from(source));

        Track::new_with_data(lazy.into(), meta).volume(self.inner.effective_volume(guild_id))
    }

    /// Agrega una canción al final de la cola.
    ///
    /// No hay que "arrancar" nada: si la cola estaba vacía, songbird reproduce
    /// la pista recién encolada de inmediato.
    pub async fn play(&self, guild_id: GuildId, source: TrackSource) -> Result<()> {
        let call = self
            .call(guild_id)
            .ok_or_else(|| anyhow::anyhow!("No hay conexión de voz activa"))?;

        let title = source.title();
        let track = self.build_track(guild_id, source);

        let mut handler = call.lock().await;
        if handler.queue().len() >= MAX_QUEUE_LEN {
            anyhow::bail!("La cola está llena (máximo {} canciones)", MAX_QUEUE_LEN);
        }
        handler.enqueue(track).await;
        drop(handler);

        info!("Encolado: {}", title);
        Ok(())
    }

    // ------------------------------------------------------------ transporte

    pub async fn pause(&self, guild_id: GuildId) -> Result<()> {
        if let Some(call) = self.call(guild_id) {
            call.lock().await.queue().pause().ok();
            info!("Pausado en guild {}", guild_id);
        }
        Ok(())
    }

    pub async fn resume(&self, guild_id: GuildId) -> Result<()> {
        if let Some(call) = self.call(guild_id) {
            call.lock().await.queue().resume().ok();
            info!("Reanudado en guild {}", guild_id);
        }
        Ok(())
    }

    /// Detiene la reproducción y vacía la cola por completo.
    pub async fn stop(&self, guild_id: GuildId) -> Result<()> {
        if let Some(call) = self.call(guild_id) {
            call.lock().await.queue().stop();
        }
        self.inner.history.remove(&guild_id);
        info!("Detenido y cola vaciada en guild {}", guild_id);
        Ok(())
    }

    /// Salta `amount` canciones.
    ///
    /// Descarta las `amount - 1` pendientes y fuerza el salto de la actual.
    pub async fn skip_tracks(&self, guild_id: GuildId, amount: usize) -> Result<()> {
        let call = self
            .call(guild_id)
            .ok_or_else(|| anyhow::anyhow!("No hay conexión de voz activa"))?;

        let handler = call.lock().await;
        let queue = handler.queue();

        if queue.is_empty() {
            anyhow::bail!("No hay nada reproduciéndose");
        }

        let to_drop = amount.min(queue.len()).saturating_sub(1);
        if to_drop > 0 {
            // Las pistas descartadas hay que detenerlas explícitamente: sacarlas
            // de la cola no libera sus recursos.
            queue.modify_queue(|q| {
                for queued in q.drain(1..=to_drop) {
                    queued.stop().ok();
                }
            });
        }

        // El evento `End` de una pista detenida no llega al historial (ver
        // `TrackEndHandler`), así que se apunta acá, que es donde se sabe que
        // esta pista se saltó de verdad.
        if let Some(current) = queue.current() {
            self.push_history(guild_id, (*meta_of(&current)).clone());
        }

        force_skip_top_track(queue);
        info!("Saltadas {} canciones en guild {}", amount, guild_id);
        Ok(())
    }

    /// Vuelve a la canción anterior del historial.
    pub async fn play_previous(&self, guild_id: GuildId) -> Result<QueueItem> {
        let previous = self
            .inner
            .history
            .get_mut(&guild_id)
            .and_then(|mut h| h.pop())
            .ok_or_else(|| anyhow::anyhow!("No hay canciones anteriores en el historial"))?;

        self.play_at_front(guild_id, previous.source.clone(), None)
            .await?;
        Ok(previous)
    }

    /// Reinicia la canción actual desde el principio.
    pub async fn restart_current(&self, guild_id: GuildId) -> Result<QueueItem> {
        let current = self
            .current_meta(guild_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("No hay nada reproduciéndose"))?;

        self.play_at_front(guild_id, current.source.clone(), None)
            .await?;
        Ok((*current).clone())
    }

    /// Salta a una posición concreta del stream de la canción actual.
    ///
    /// El audio llega por una tubería, que no se puede rebobinar: se vuelve a
    /// abrir el stream con `-ss` en ffmpeg y se reemplaza la pista en curso.
    pub async fn seek_current(&self, guild_id: GuildId, position: Duration) -> Result<QueueItem> {
        let current = self
            .current_meta(guild_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("No hay nada reproduciéndose"))?;

        self.play_at_front(guild_id, current.source.clone(), Some(position))
            .await?;
        Ok((*current).clone())
    }

    /// Encola una fuente y la convierte en la pista actual de inmediato.
    ///
    /// Se apoya en las mismas primitivas que el resto (encolar + reordenar +
    /// saltar) en vez de reproducir "por fuera" de la cola, que es justo lo que
    /// antes provocaba dos temas sonando a la vez.
    async fn play_at_front(
        &self,
        guild_id: GuildId,
        source: TrackSource,
        seek: Option<Duration>,
    ) -> Result<()> {
        let call = self
            .call(guild_id)
            .ok_or_else(|| anyhow::anyhow!("No hay conexión de voz activa"))?;

        let track = match seek {
            Some(position) => {
                let effects = self.inner.effects.clone();
                let filter = Arc::new(move || effects.build_filter(guild_id));
                let lazy = LazyFfmpegSource::seeking(&source, filter, position);
                let meta = Arc::new(QueueItem::from(source));
                Track::new_with_data(lazy.into(), meta)
                    .volume(self.inner.effective_volume(guild_id))
            }
            None => self.build_track(guild_id, source),
        };

        let mut handler = call.lock().await;
        handler.enqueue(track).await;

        let queue = handler.queue();
        let was_playing = queue.len() > 1;
        if was_playing {
            // Ponerla justo detrás de la actual y saltar: queda al frente sin
            // que la anterior siga sonando.
            queue.modify_queue(|q| {
                if let Some(last) = q.pop_back() {
                    q.insert(1, last);
                }
            });
            force_skip_top_track(queue);
        }

        self.apply_loop_mode_to_current(guild_id, queue.current());
        Ok(())
    }

    // ------------------------------------------------------------- consultas

    pub async fn is_playing(&self, guild_id: GuildId) -> bool {
        let Some(call) = self.call(guild_id) else {
            return false;
        };
        let current = call.lock().await.queue().current();
        match current {
            Some(track) => matches!(
                track.get_info().await.map(|i| i.playing),
                Ok(PlayMode::Play)
            ),
            None => false,
        }
    }

    /// Metadatos de la pista en curso.
    pub async fn current_meta(&self, guild_id: GuildId) -> Option<Arc<QueueItem>> {
        let call = self.call(guild_id)?;
        let current = call.lock().await.queue().current()?;
        Some(meta_of(&current))
    }

    pub async fn get_current_track(&self, guild_id: GuildId) -> Option<TrackSource> {
        self.current_meta(guild_id)
            .await
            .map(|m| m.source.clone())
    }

    /// Posición de reproducción de la pista actual.
    pub async fn current_position(&self, guild_id: GuildId) -> Option<Duration> {
        let call = self.call(guild_id)?;
        let current = call.lock().await.queue().current()?;
        current.get_info().await.ok().map(|i| i.position)
    }

    pub async fn get_queue_info(&self, guild_id: GuildId) -> Result<QueueInfo> {
        let loop_mode = self.loop_mode(guild_id);
        let shuffle = self.is_shuffle(guild_id);

        match self.call(guild_id) {
            Some(call) => {
                let handles = call.lock().await.queue().current_queue();
                Ok(QueueInfo::from_handles(&handles, loop_mode, shuffle))
            }
            None => Ok(QueueInfo::empty(loop_mode, shuffle)),
        }
    }

    /// Pistas pendientes (sin la actual).
    pub async fn get_queue(&self, guild_id: GuildId) -> Option<Vec<QueueItem>> {
        let info = self.get_queue_info(guild_id).await.ok()?;
        Some(info.items)
    }

    // ------------------------------------------------- modos de reproducción

    pub fn loop_mode(&self, guild_id: GuildId) -> LoopMode {
        self.inner
            .loop_modes
            .get(&guild_id)
            .map(|m| *m)
            .unwrap_or_default()
    }

    pub fn is_shuffle(&self, guild_id: GuildId) -> bool {
        self.inner.shuffle.get(&guild_id).map(|s| *s).unwrap_or(false)
    }

    pub async fn toggle_loop(&self, guild_id: GuildId) -> Result<bool> {
        let next = match self.loop_mode(guild_id) {
            LoopMode::Off => LoopMode::Queue,
            _ => LoopMode::Off,
        };
        self.set_loop_mode_specific(guild_id, next).await?;
        Ok(next != LoopMode::Off)
    }

    pub async fn set_loop_mode_specific(&self, guild_id: GuildId, mode: LoopMode) -> Result<()> {
        self.inner.loop_modes.insert(guild_id, mode);

        // El modo `Track` lo implementa songbird sobre la pista en curso; el
        // modo `Queue` lo aplica el handler de fin de pista al reencolarla.
        let current = match self.call(guild_id) {
            Some(call) => call.lock().await.queue().current(),
            None => None,
        };
        self.apply_loop_mode_to_current(guild_id, current);

        info!("Modo de repetición {:?} en guild {}", mode, guild_id);
        Ok(())
    }

    fn apply_loop_mode_to_current(&self, guild_id: GuildId, current: Option<TrackHandle>) {
        let Some(track) = current else { return };
        match self.loop_mode(guild_id) {
            LoopMode::Track => {
                track.enable_loop().ok();
            }
            _ => {
                track.disable_loop().ok();
            }
        }
    }

    /// Activa/desactiva el modo aleatorio. Al activarlo, mezcla lo pendiente.
    pub async fn toggle_shuffle(&self, guild_id: GuildId) -> Result<bool> {
        let enabled = !self.is_shuffle(guild_id);
        self.inner.shuffle.insert(guild_id, enabled);

        if enabled {
            self.shuffle_pending(guild_id).await;
        }

        info!(
            "Aleatorio {} en guild {}",
            if enabled { "activado" } else { "desactivado" },
            guild_id
        );
        Ok(enabled)
    }

    /// Mezcla las pistas pendientes dejando intacta la que suena.
    pub async fn shuffle_pending(&self, guild_id: GuildId) {
        let Some(call) = self.call(guild_id) else {
            return;
        };
        let handler = call.lock().await;
        handler.queue().modify_queue(|q| {
            if q.len() <= 2 {
                return;
            }
            let mut rest: Vec<_> = q.drain(1..).collect();
            rest.shuffle(&mut rand::thread_rng());
            q.extend(rest);
        });
    }

    // ------------------------------------------------------ edición de cola

    /// Vacía las pistas pendientes sin cortar la que suena.
    pub async fn clear_queue(&self, guild_id: GuildId) -> Result<()> {
        if let Some(call) = self.call(guild_id) {
            let handler = call.lock().await;
            handler.queue().modify_queue(|q| {
                for queued in q.drain(1..) {
                    queued.stop().ok();
                }
            });
        }
        info!("Cola limpiada en guild {}", guild_id);
        Ok(())
    }

    pub async fn clear_duplicates(&self, guild_id: GuildId) -> Result<usize> {
        self.retain_pending(guild_id, {
            let mut seen = std::collections::HashSet::new();
            move |item: &QueueItem| seen.insert(item.url.clone())
        })
        .await
    }

    pub async fn clear_user_tracks(&self, guild_id: GuildId, user_id: UserId) -> Result<usize> {
        self.retain_pending(guild_id, move |item: &QueueItem| item.requested_by != user_id)
            .await
    }

    /// Filtra las pistas pendientes; devuelve cuántas se quitaron.
    async fn retain_pending<F>(&self, guild_id: GuildId, mut keep: F) -> Result<usize>
    where
        F: FnMut(&QueueItem) -> bool,
    {
        let Some(call) = self.call(guild_id) else {
            return Ok(0);
        };
        let handler = call.lock().await;

        let removed = handler.queue().modify_queue(|q| {
            let mut removed = 0;
            let mut kept = std::collections::VecDeque::with_capacity(q.len());

            for (idx, queued) in q.drain(..).enumerate() {
                // El índice 0 es la pista sonando: nunca se toca.
                if idx == 0 || keep(&meta_of(&queued)) {
                    kept.push_back(queued);
                } else {
                    queued.stop().ok();
                    removed += 1;
                }
            }

            *q = kept;
            removed
        });

        if removed > 0 {
            info!("Quitadas {} canciones de la cola", removed);
        }
        Ok(removed)
    }

    /// Quita la pista en la posición `position` (1 = la siguiente en sonar).
    pub async fn remove_track(&self, guild_id: GuildId, position: usize) -> Result<QueueItem> {
        let call = self
            .call(guild_id)
            .ok_or_else(|| anyhow::anyhow!("No hay conexión de voz activa"))?;
        let handler = call.lock().await;
        let queue = handler.queue();

        let pending = queue.len().saturating_sub(1);
        if position == 0 || position > pending {
            anyhow::bail!("Posición {} fuera de rango (1-{})", position, pending);
        }

        let removed = queue
            .dequeue(position)
            .ok_or_else(|| anyhow::anyhow!("No se pudo quitar la canción"))?;
        let meta = (*meta_of(&removed)).clone();
        removed.stop().ok();

        Ok(meta)
    }

    /// Salta directamente a la posición `position` de la cola (1 = la siguiente).
    pub async fn jump_to(&self, guild_id: GuildId, position: usize) -> Result<QueueItem> {
        let call = self
            .call(guild_id)
            .ok_or_else(|| anyhow::anyhow!("No hay conexión de voz activa"))?;
        let handler = call.lock().await;
        let queue = handler.queue();

        let pending = queue.len().saturating_sub(1);
        if position == 0 || position > pending {
            anyhow::bail!("Posición {} fuera de rango (1-{})", position, pending);
        }

        // Descartar todo lo que quede por delante del objetivo.
        queue.modify_queue(|q| {
            for queued in q.drain(1..position) {
                queued.stop().ok();
            }
        });
        if let Some(current) = queue.current() {
            self.push_history(guild_id, (*meta_of(&current)).clone());
        }
        force_skip_top_track(queue);

        let target = queue
            .current()
            .ok_or_else(|| anyhow::anyhow!("No se pudo saltar a esa posición"))?;
        Ok((*meta_of(&target)).clone())
    }

    // ------------------------------------------------------------- historial

    /// Registra una pista terminada en el historial de la guild.
    pub fn push_history(&self, guild_id: GuildId, item: QueueItem) {
        let mut history = self.inner.history.entry(guild_id).or_default();
        history.push(item);
        if history.len() > MAX_HISTORY {
            history.remove(0);
        }
    }

    // --------------------------------------------------------------- volumen

    pub async fn set_volume(&self, guild_id: GuildId, volume: f32) -> Result<()> {
        let volume = volume.clamp(0.0, 2.0);
        self.inner.volumes.insert(guild_id, volume);

        // Aplicarlo a todo lo encolado, no sólo a lo que suena: así el ajuste
        // no se pierde al cambiar de canción.
        for handle in self.handles(guild_id).await {
            handle.set_volume(volume).ok();
        }

        info!("Volumen a {:.0}% en guild {}", volume * 100.0, guild_id);
        Ok(())
    }

    pub async fn get_volume(&self, guild_id: GuildId) -> Option<f32> {
        Some(self.inner.effective_volume(guild_id))
    }

    // ------------------------------------------------------------ ecualizador

    pub async fn apply_equalizer_preset(
        &self,
        guild_id: GuildId,
        preset: EqualizerPreset,
    ) -> Result<()> {
        self.inner.effects.apply_equalizer_preset(guild_id, preset);
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn reset_equalizer(&self, guild_id: GuildId) -> Result<()> {
        self.inner.effects.reset_equalizer(guild_id);
        Ok(())
    }

    pub fn get_equalizer_details(&self, guild_id: GuildId) -> String {
        self.inner.effects.get_equalizer_details(guild_id)
    }

    /// Olvida todo el estado de una guild (al desconectar).
    pub fn forget_guild(&self, guild_id: GuildId) {
        self.inner.history.remove(&guild_id);
    }
}

/// Fuerza el salto de la pista que está sonando.
///
/// El orden importa y no es intuitivo (mismo truco que usa Parrot):
///
/// 1. `stop()` sobre la actual — pero songbird tarda en retirarla de la cola;
/// 2. `dequeue(0)` la saca a mano — pero quitarla no dispara la siguiente;
/// 3. `resume()` pone a sonar la que quedó al frente.
///
/// Hacerlo en otro orden deja o dos pistas sonando a la vez, o la cola parada
/// con canciones dentro.
pub fn force_skip_top_track(queue: &songbird::tracks::TrackQueue) {
    if let Some(track) = queue.current() {
        track.stop().ok();
    }
    let _ = queue.dequeue(0);
    if let Err(e) = queue.resume() {
        warn!("No se pudo reanudar la cola tras saltar: {:?}", e);
    }
}
