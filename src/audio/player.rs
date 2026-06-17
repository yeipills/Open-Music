use anyhow::Result;
use dashmap::DashMap;
use parking_lot::RwLock;
use serenity::model::id::{GuildId, UserId};
use songbird::{
    tracks::{PlayMode, TrackHandle},
    Call, Event, EventContext, EventHandler as SongbirdEventHandler, TrackEvent,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::{
    audio::{
        effects::{AudioEffects, EqualizerPreset},
        queue::{LoopMode, MusicQueue, QueueInfo, QueueItem},
    },
    sources::TrackSource,
};

/// Tiempo de gracia tras vaciarse la cola antes de desconectar del canal de voz.
const AUTO_LEAVE_GRACE: Duration = Duration::from_secs(60);

/// Estado compartido del reproductor.
///
/// Vive detrás de un único `Arc`, de modo que tanto [`AudioPlayer`] como los
/// event handlers de songbird operan sobre **los mismos** mapas. Esto es crítico:
/// si cada handler tuviera su propia copia (como ocurría con `DashMap::clone`),
/// `is_playing` leería handles obsoletos y se reproducirían pistas superpuestas.
struct PlayerInner {
    /// Cola por guild (el `Arc` interno sí se comparte).
    queues: DashMap<GuildId, Arc<RwLock<MusicQueue>>>,
    /// Efectos de audio (EQ + loudnorm) compartidos.
    effects: Arc<AudioEffects>,
    /// Handle de la pista que suena actualmente, por guild.
    current_tracks: DashMap<GuildId, TrackHandle>,
    /// Volumen efectivo por guild (0.0–2.0). Se aplica a cada pista nueva para
    /// que el ajuste persista entre canciones, no solo en la que suena.
    volumes: DashMap<GuildId, f32>,
    /// Volumen por defecto (de la config) cuando una guild no tiene ajuste propio.
    default_volume: f32,
    /// Contador de "generación" por guild. Cada vez que arranca una pista nueva
    /// se incrementa; el event handler de fin sólo avanza si su generación sigue
    /// vigente. Así distinguimos un fin natural de un stop/skip/leave manual.
    generations: DashMap<GuildId, Arc<AtomicU64>>,
    /// Lock por guild para serializar las transiciones de pista (evita carreras
    /// entre el avance automático y un `/play` simultáneo).
    advance_locks: DashMap<GuildId, Arc<Mutex<()>>>,
}

impl PlayerInner {
    /// Volumen efectivo de la guild (ajuste propio o el default de la config).
    fn effective_volume(&self, guild_id: GuildId) -> f32 {
        self.volumes
            .get(&guild_id)
            .map(|v| *v)
            .unwrap_or(self.default_volume)
    }

    fn queue(&self, guild_id: GuildId) -> Arc<RwLock<MusicQueue>> {
        self.queues
            .entry(guild_id)
            .or_insert_with(|| Arc::new(RwLock::new(MusicQueue::new(100))))
            .clone()
    }

    fn generation(&self, guild_id: GuildId) -> Arc<AtomicU64> {
        self.generations
            .entry(guild_id)
            .or_insert_with(|| Arc::new(AtomicU64::new(0)))
            .clone()
    }

    fn advance_lock(&self, guild_id: GuildId) -> Arc<Mutex<()>> {
        self.advance_locks
            .entry(guild_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// `true` si hay una pista activa (reproduciéndose o en pausa) en la guild.
    async fn is_occupied(&self, guild_id: GuildId) -> bool {
        if let Some(track) = self.current_tracks.get(&guild_id) {
            if let Ok(info) = track.get_info().await {
                return matches!(info.playing, PlayMode::Play | PlayMode::Pause);
            }
        }
        false
    }
}

pub struct AudioPlayer {
    inner: Arc<PlayerInner>,
}

impl AudioPlayer {
    pub fn new(default_volume: f32) -> Self {
        Self {
            inner: Arc::new(PlayerInner {
                queues: DashMap::new(),
                effects: Arc::new(AudioEffects::new()),
                current_tracks: DashMap::new(),
                volumes: DashMap::new(),
                default_volume: default_volume.clamp(0.0, 2.0),
                generations: DashMap::new(),
                advance_locks: DashMap::new(),
            }),
        }
    }

    /// Agrega una canción a la cola y comienza a reproducir si no hay nada sonando.
    pub async fn play(
        &self,
        guild_id: GuildId,
        source: TrackSource,
        handler: Arc<Mutex<Call>>,
    ) -> Result<()> {
        {
            let queue = self.inner.queue(guild_id);
            let mut q = queue.write();
            q.add_track(source)?;
        }

        // Sólo arranca si está libre. La decisión se toma dentro del lock para
        // que dos `/play` simultáneos no inicien dos pistas a la vez.
        Self::try_start_if_idle(&self.inner, guild_id, handler).await;
        Ok(())
    }

    /// Reproduce inmediatamente una fuente concreta (usado por `/previous`),
    /// deteniendo lo que esté sonando. No toca la cola.
    pub async fn play_source_now(
        &self,
        guild_id: GuildId,
        source: TrackSource,
        handler: Arc<Mutex<Call>>,
    ) -> Result<()> {
        let lock = self.inner.advance_lock(guild_id);
        let _guard = lock.lock().await;
        Self::start_track(&self.inner, guild_id, source, &handler).await
    }

    /// Arranca la siguiente canción de la cola **solo si no hay nada sonando**.
    /// Lo usan el flujo de playlist y el arranque inicial.
    pub async fn play_next(&self, guild_id: GuildId, handler: Arc<Mutex<Call>>) -> Result<()> {
        Self::try_start_if_idle(&self.inner, guild_id, handler).await;
        Ok(())
    }

    /// Pausa la reproducción.
    pub async fn pause(&self, guild_id: GuildId) -> Result<()> {
        if let Some(track) = self.inner.current_tracks.get(&guild_id) {
            if let Err(e) = track.pause() {
                warn!("Error pausando track: {:?}", e);
            }
            info!("⏸️ Reproducción pausada en guild {}", guild_id);
        }
        Ok(())
    }

    /// Reanuda la reproducción.
    pub async fn resume(&self, guild_id: GuildId) -> Result<()> {
        if let Some(track) = self.inner.current_tracks.get(&guild_id) {
            if let Err(e) = track.play() {
                warn!("Error reanudando track: {:?}", e);
            }
            info!("▶️ Reproducción reanudada en guild {}", guild_id);
        }
        Ok(())
    }

    /// Detiene la reproducción y limpia la cola.
    ///
    /// Incrementa la generación **antes** de detener la pista, de modo que el
    /// evento `End` que dispara el `stop()` quede obsoleto y el handler no
    /// re-reproduzca nada.
    pub async fn stop(&self, guild_id: GuildId) -> Result<()> {
        self.inner.generation(guild_id).fetch_add(1, Ordering::AcqRel);

        if let Some((_, track)) = self.inner.current_tracks.remove(&guild_id) {
            if let Err(e) = track.stop() {
                warn!("Error deteniendo track: {:?}", e);
            }
        }

        self.clear_queue(guild_id).await?;
        info!("⏹️ Reproducción detenida en guild {}", guild_id);
        Ok(())
    }

    /// Salta `amount` canciones.
    ///
    /// Descarta las `amount - 1` siguientes de la cola y avanza a la próxima.
    /// `force_advance` detiene la pista actual antes de reproducir, así nunca
    /// se solapan.
    pub async fn skip_tracks(
        &self,
        guild_id: GuildId,
        amount: usize,
        handler: Arc<Mutex<Call>>,
    ) -> Result<()> {
        {
            let queue = self.inner.queue(guild_id);
            let mut q = queue.write();
            q.skip(amount.saturating_sub(1));
        }

        Self::force_advance(&self.inner, guild_id, handler).await;
        info!("⏭️ Saltadas {} canciones en guild {}", amount, guild_id);
        Ok(())
    }

    /// `true` sólo si hay una pista reproduciéndose activamente.
    pub async fn is_playing(&self, guild_id: GuildId) -> bool {
        if let Some(track) = self.inner.current_tracks.get(&guild_id) {
            if let Ok(info) = track.get_info().await {
                return info.playing == PlayMode::Play;
            }
        }
        false
    }

    pub async fn get_current_track(&self, guild_id: GuildId) -> Option<TrackSource> {
        let queue = self.inner.queue(guild_id);
        let q = queue.read();
        q.current_track()
    }

    pub async fn get_queue_info(&self, guild_id: GuildId) -> Result<QueueInfo> {
        let queue = self.inner.queue(guild_id);
        let q = queue.read();
        Ok(q.get_info())
    }

    /// Obtiene o crea la cola de una guild.
    pub fn get_or_create_queue(&self, guild_id: GuildId) -> Arc<RwLock<MusicQueue>> {
        self.inner.queue(guild_id)
    }

    /// Obtiene la cola sin crear una nueva.
    pub async fn get_queue(&self, guild_id: GuildId) -> Option<Vec<QueueItem>> {
        self.inner
            .queues
            .get(&guild_id)
            .map(|queue_arc| queue_arc.read().get_info().items)
    }

    pub async fn toggle_loop(&self, guild_id: GuildId) -> Result<bool> {
        let queue = self.inner.queue(guild_id);
        let mut q = queue.write();
        Ok(q.toggle_loop())
    }

    pub async fn clear_queue(&self, guild_id: GuildId) -> Result<()> {
        let queue = self.inner.queue(guild_id);
        let mut q = queue.write();
        q.clear();
        info!("🗑️ Cola limpiada en guild {}", guild_id);
        Ok(())
    }

    pub async fn toggle_shuffle(&self, guild_id: GuildId) -> Result<bool> {
        let queue = self.inner.queue(guild_id);
        let mut q = queue.write();
        let shuffled = q.toggle_shuffle();
        info!(
            "🔀 Shuffle {} en guild {}",
            if shuffled { "activado" } else { "desactivado" },
            guild_id
        );
        Ok(shuffled)
    }

    pub async fn set_loop_mode_specific(&self, guild_id: GuildId, mode: LoopMode) -> Result<()> {
        let queue = self.inner.queue(guild_id);
        let mut q = queue.write();
        q.set_loop_mode(mode);
        info!("🔁 Modo de bucle establecido a {:?} en guild {}", mode, guild_id);
        Ok(())
    }

    pub async fn set_volume(&self, guild_id: GuildId, volume: f32) -> Result<()> {
        let volume = volume.clamp(0.0, 2.0);
        // Persistir el ajuste para que se aplique también a las próximas canciones.
        self.inner.volumes.insert(guild_id, volume);

        if let Some(track) = self.inner.current_tracks.get(&guild_id) {
            if let Err(e) = track.set_volume(volume) {
                warn!("Error estableciendo volumen: {:?}", e);
                anyhow::bail!("Error al establecer volumen: {:?}", e);
            }
        }
        info!("🔊 Volumen ajustado a {:.1}% en guild {}", volume * 100.0, guild_id);
        Ok(())
    }

    pub async fn get_volume(&self, guild_id: GuildId) -> Option<f32> {
        if let Some(track) = self.inner.current_tracks.get(&guild_id) {
            if let Ok(info) = track.get_info().await {
                return Some(info.volume);
            }
        }
        // Sin pista activa: devolver el volumen efectivo (ajuste guardado o default).
        Some(self.inner.effective_volume(guild_id))
    }

    // ---- Ecualizador ----

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

    pub async fn clear_duplicates(&self, guild_id: GuildId) -> Result<usize> {
        let queue = self.inner.queue(guild_id);
        let mut q = queue.write();
        Ok(q.clear_duplicates())
    }

    pub async fn clear_user_tracks(&self, guild_id: GuildId, user_id: UserId) -> Result<usize> {
        let queue = self.inner.queue(guild_id);
        let mut q = queue.write();
        Ok(q.clear_user_tracks(user_id))
    }

    #[allow(dead_code)]
    pub fn get_now_playing(&self, guild_id: GuildId) -> Option<TrackSource> {
        let queue = self.inner.queue(guild_id);
        let q = queue.read();
        q.current_track()
    }

    // ---- Lógica interna de transiciones ----

    /// Arranca la siguiente canción **solo si no hay nada sonando ni en pausa**.
    /// La comprobación de ocupación se hace dentro del lock para evitar TOCTOU.
    async fn try_start_if_idle(inner: &Arc<PlayerInner>, guild_id: GuildId, handler: Arc<Mutex<Call>>) {
        let lock = inner.advance_lock(guild_id);
        let _guard = lock.lock().await;

        if inner.is_occupied(guild_id).await {
            return;
        }
        Self::advance_pop_and_start(inner, guild_id, &handler).await;
    }

    /// Avanza a la siguiente canción de forma incondicional (fin natural / skip).
    async fn force_advance(inner: &Arc<PlayerInner>, guild_id: GuildId, handler: Arc<Mutex<Call>>) {
        let lock = inner.advance_lock(guild_id);
        let _guard = lock.lock().await;
        Self::advance_pop_and_start(inner, guild_id, &handler).await;
    }

    /// Saca de la cola la siguiente pista y la reproduce. Si falla la obtención
    /// del audio, salta a la siguiente (con un tope para no quedar en bucle).
    /// Si la cola queda vacía, programa la auto-desconexión.
    ///
    /// Debe llamarse con el `advance_lock` de la guild tomado.
    async fn advance_pop_and_start(
        inner: &Arc<PlayerInner>,
        guild_id: GuildId,
        handler: &Arc<Mutex<Call>>,
    ) {
        for _ in 0..10 {
            let next = {
                let queue = inner.queue(guild_id);
                let mut q = queue.write();
                q.next_track()
            };

            match next {
                Some(source) => match Self::start_track(inner, guild_id, source, handler).await {
                    Ok(()) => return,
                    Err(e) => {
                        warn!("❌ Error reproduciendo track, saltando al siguiente: {:?}", e);
                        continue;
                    }
                },
                None => {
                    info!("📭 Cola vacía en guild {}", guild_id);
                    inner.current_tracks.remove(&guild_id);
                    Self::schedule_auto_leave(inner.clone(), guild_id, handler.clone());
                    return;
                }
            }
        }

        warn!("⚠️ Demasiados errores consecutivos en guild {}, deteniendo", guild_id);
        inner.current_tracks.remove(&guild_id);
        Self::schedule_auto_leave(inner.clone(), guild_id, handler.clone());
    }

    /// Reproduce una fuente concreta deteniendo antes la pista anterior, y
    /// registra los handlers de fin/error con la generación vigente.
    async fn start_track(
        inner: &Arc<PlayerInner>,
        guild_id: GuildId,
        source: TrackSource,
        handler: &Arc<Mutex<Call>>,
    ) -> Result<()> {
        // Invalida la generación previa antes de detener la pista actual: así el
        // evento `End` de la pista que cortamos ya no coincide y se ignora.
        let new_gen = inner
            .generation(guild_id)
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);

        if let Some(old) = inner.current_tracks.get(&guild_id) {
            let _ = old.stop();
        }

        info!("▶️ Iniciando reproducción de: {}", source.title());
        let filter = inner.effects.build_filter(guild_id);
        let input = source
            .get_input(&filter)
            .await
            .map_err(|e| anyhow::anyhow!("Error obteniendo input: {:?}", e))?;

        let track_handle = {
            let mut call = handler.lock().await;
            call.play_input(input)
        };

        // Aplicar el volumen efectivo de la guild a la pista nueva, para que el
        // ajuste persista entre canciones.
        let _ = track_handle.set_volume(inner.effective_volume(guild_id));

        let end_handler = TrackEndHandler {
            guild_id,
            generation: new_gen,
            inner: inner.clone(),
            handler: handler.clone(),
        };
        let error_handler = TrackErrorHandler {
            guild_id,
            generation: new_gen,
            inner: inner.clone(),
            handler: handler.clone(),
        };
        track_handle
            .add_event(Event::Track(TrackEvent::End), end_handler)
            .ok();
        track_handle
            .add_event(Event::Track(TrackEvent::Error), error_handler)
            .ok();

        inner.current_tracks.insert(guild_id, track_handle);
        info!("🎵 Reproduciendo: {} en guild {}", source.title(), guild_id);
        Ok(())
    }

    /// Programa la desconexión del canal de voz tras un periodo de gracia,
    /// siempre que siga sin haber pista ni cola.
    fn schedule_auto_leave(inner: Arc<PlayerInner>, guild_id: GuildId, handler: Arc<Mutex<Call>>) {
        tokio::spawn(async move {
            tokio::time::sleep(AUTO_LEAVE_GRACE).await;

            let occupied = inner.current_tracks.contains_key(&guild_id);
            let has_queue = inner
                .queues
                .get(&guild_id)
                .map(|q| !q.read().is_empty())
                .unwrap_or(false);

            if occupied || has_queue {
                return; // se reanudó la actividad, no desconectar
            }

            let mut call = handler.lock().await;
            if let Err(e) = call.leave().await {
                warn!("Error al auto-desconectar en guild {}: {:?}", guild_id, e);
            } else {
                info!("👋 Auto-desconectado: cola finalizada (guild {})", guild_id);
            }
        });
    }
}

/// Handler de fin de pista. Sólo avanza si su generación sigue vigente.
struct TrackEndHandler {
    guild_id: GuildId,
    generation: u64,
    inner: Arc<PlayerInner>,
    handler: Arc<Mutex<Call>>,
}

#[async_trait::async_trait]
impl SongbirdEventHandler for TrackEndHandler {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        let current_gen = self.inner.generation(self.guild_id).load(Ordering::Acquire);
        if current_gen != self.generation {
            // La pista fue detenida/saltada manualmente: este fin es obsoleto.
            return None;
        }

        info!("🎵 Track terminó en guild {}, avanzando al siguiente...", self.guild_id);
        AudioPlayer::force_advance(&self.inner, self.guild_id, self.handler.clone()).await;
        None
    }
}

/// Handler de error de pista. Misma guarda de generación que el de fin.
struct TrackErrorHandler {
    guild_id: GuildId,
    generation: u64,
    inner: Arc<PlayerInner>,
    handler: Arc<Mutex<Call>>,
}

#[async_trait::async_trait]
impl SongbirdEventHandler for TrackErrorHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        tracing::error!("❌ Error en track para guild {}: {:?}", self.guild_id, ctx);

        let current_gen = self.inner.generation(self.guild_id).load(Ordering::Acquire);
        if current_gen != self.generation {
            return None;
        }

        AudioPlayer::force_advance(&self.inner, self.guild_id, self.handler.clone()).await;
        None
    }
}
