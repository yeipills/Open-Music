use anyhow::Result;
use dashmap::DashMap;
use parking_lot::RwLock;
use serenity::model::id::GuildId;
use songbird::{
    tracks::{PlayMode, TrackHandle},
    Call, Event, EventContext, EventHandler as VoiceEventHandler, TrackEvent,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use crate::{
    audio::{effects::AudioEffects, queue::{MusicQueue, QueueInfo, QueueItem}},
    sources::TrackSource,
};


pub struct AudioPlayer {
    queues: DashMap<GuildId, Arc<RwLock<MusicQueue>>>,
    effects: Arc<AudioEffects>,
    current_tracks: DashMap<GuildId, TrackHandle>,
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self {
            queues: DashMap::new(),
            effects: Arc::new(AudioEffects::new()),
            current_tracks: DashMap::new(),
        }
    }

    /// Reproduce una canción en el canal de voz (con handler)
    pub async fn play(
        &self,
        guild_id: GuildId,
        source: TrackSource,
        handler: Arc<Mutex<Call>>,
    ) -> Result<()> {
        let queue = self.get_or_create_queue(guild_id);

        // Agregar a la cola
        {
            let mut q = queue.write();
            q.add_track(source.clone())?;
        }

        // Si no hay nada reproduciéndose, iniciar reproducción
        if !self.is_playing(guild_id).await {
            self.play_next(guild_id, handler).await?;
        }

        Ok(())
    }

    /// Reproduce una canción en el canal de voz (sin handler - solo agrega a cola)
    pub async fn play_without_handler(&self, guild_id: GuildId, source: TrackSource) -> Result<()> {
        let queue = self.get_or_create_queue(guild_id);

        // Solo agregar a la cola
        {
            let mut q = queue.write();
            q.add_track(source.clone());
        }

        Ok(())
    }

    /// Reproduce la siguiente canción en la cola
    pub async fn play_next(&self, guild_id: GuildId, handler: Arc<Mutex<Call>>) -> Result<()> {
        let queue = self.get_or_create_queue(guild_id);

        let next_track = {
            let mut q = queue.write();
            q.next_track()
        };

        if let Some(track_source) = next_track {
            info!("🎵 Reproduciendo: {}", track_source.title());

            // Obtener stream de audio
            let input = track_source.get_input().await?;

            // Aplicar efectos si están habilitados
            let processed_input = self.effects.process_input(input).await?;

            // Reproducir en el handler
            let mut handler_lock = handler.lock().await;
            let track_handle = handler_lock.play_input(processed_input);

            // Configurar volumen por defecto
            let _ = track_handle.set_volume(0.5);

            // Registrar event handler para auto-play
            let player_clone = Arc::new(self.clone());
            let guild_id_clone = guild_id;
            let handler_clone = handler.clone();

            track_handle
                .add_event(
                    Event::Track(TrackEvent::End),
                    TrackEndHandler {
                        player: player_clone,
                        guild_id: guild_id_clone,
                        handler: handler_clone,
                    },
                )
                .map_err(|e| anyhow::anyhow!("Error al agregar event handler: {}", e))?;

            // Guardar referencia al track actual
            self.current_tracks.insert(guild_id, track_handle);
        } else {
            debug!("Cola vacía para guild {}", guild_id);
        }

        Ok(())
    }

    /// Pausa la reproducción actual
    pub async fn pause(&self, guild_id: GuildId) -> Result<()> {
        if let Some(track) = self.current_tracks.get(&guild_id) {
            let _ = track.pause();
            info!("⏸️ Reproducción pausada");
        }
        Ok(())
    }

    /// Reanuda la reproducción
    pub async fn resume(&self, guild_id: GuildId) -> Result<()> {
        if let Some(track) = self.current_tracks.get(&guild_id) {
            let _ = track.play();
            info!("▶️ Reproducción reanudada");
        }
        Ok(())
    }

    /// Salta a la siguiente canción
    pub async fn skip(&self, guild_id: GuildId, handler: Arc<Mutex<Call>>) -> Result<()> {
        // Detener track actual
        if let Some(track) = self.current_tracks.get(&guild_id) {
            let _ = track.stop();
        }

        // Reproducir siguiente
        self.play_next(guild_id, handler).await?;
        Ok(())
    }

    /// Detiene la reproducción y limpia la cola
    pub async fn stop(&self, guild_id: GuildId) -> Result<()> {
        // Detener track actual
        if let Some((_, track)) = self.current_tracks.remove(&guild_id) {
            let _ = track.stop();
        }

        // Limpiar cola
        if let Some(queue) = self.queues.get(&guild_id) {
            let mut q = queue.write();
            q.clear();
        }

        info!("⏹️ Reproducción detenida");
        Ok(())
    }

    /// Ajusta el volumen
    pub async fn set_volume(&self, guild_id: GuildId, volume: f32) -> Result<()> {
        let clamped_volume = volume.clamp(0.0, 2.0);

        if let Some(track) = self.current_tracks.get(&guild_id) {
            let _ = track.set_volume(clamped_volume);
            info!("🔊 Volumen ajustado a {}%", (clamped_volume * 100.0) as u8);
        }

        Ok(())
    }

    /// Activa el modo shuffle
    pub async fn toggle_shuffle(&self, guild_id: GuildId) -> Result<bool> {
        let queue = self.get_or_create_queue(guild_id);
        let mut q = queue.write();
        Ok(q.toggle_shuffle())
    }

    /// Activa el modo loop
    pub async fn toggle_loop(&self, guild_id: GuildId) -> Result<bool> {
        let queue = self.get_or_create_queue(guild_id);
        let mut q = queue.write();
        Ok(q.toggle_loop())
    }

    /// Obtiene información de la canción actual
    pub fn get_now_playing(&self, guild_id: GuildId) -> Option<TrackSource> {
        let queue = self.queues.get(&guild_id)?;
        let q = queue.read();
        q.current_track()
    }

    /// Obtiene la cola actual
    pub fn get_queue(&self, guild_id: GuildId) -> Vec<TrackSource> {
        if let Some(queue) = self.queues.get(&guild_id) {
            let q = queue.read();
            q.get_tracks()
        } else {
            Vec::new()
        }
    }

    /// Verifica si hay algo reproduciéndose
    pub async fn is_playing(&self, guild_id: GuildId) -> bool {
        if let Some(track) = self.current_tracks.get(&guild_id) {
            if let Ok(info) = track.get_info().await {
                info.playing != PlayMode::Stop
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Salta múltiples canciones
    pub async fn skip_tracks(&self, guild_id: GuildId, amount: usize) -> Result<()> {
        let queue = self.get_or_create_queue(guild_id);
        {
            let mut q = queue.write();
            for _ in 0..amount.saturating_sub(1) {
                q.next_track();
            }
        }

        // Detener track actual y reproducir el siguiente
        if let Some(track) = self.current_tracks.get(&guild_id) {
            let _ = track.stop();
        }

        Ok(())
    }

    /// Obtiene información de la cola
    pub async fn get_queue_info(&self, guild_id: GuildId) -> Result<QueueInfo> {
        let queue = self.get_or_create_queue(guild_id);
        let q = queue.read();
        Ok(q.get_info())
    }

    /// Obtiene el track actual
    pub async fn get_current_track(&self, guild_id: GuildId) -> Option<QueueItem> {
        let queue = self.get_or_create_queue(guild_id);
        let q = queue.read();
        q.current().cloned()
    }

    /// Configura el modo loop
    pub async fn set_loop_mode(&self, guild_id: GuildId, enabled: bool) -> Result<()> {
        let queue = self.get_or_create_queue(guild_id);
        let mut q = queue.write();
        q.set_loop(enabled);
        Ok(())
    }

    /// Obtiene el volumen actual
    pub async fn get_volume(&self, guild_id: GuildId) -> Option<f32> {
        if let Some(track) = self.current_tracks.get(&guild_id) {
            if let Ok(info) = track.get_info().await {
                Some(info.volume)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn get_or_create_queue(&self, guild_id: GuildId) -> Arc<RwLock<MusicQueue>> {
        self.queues
            .entry(guild_id)
            .or_insert_with(|| Arc::new(RwLock::new(MusicQueue::new(100))))
            .clone()
    }
}

// Implementar Clone manualmente para AudioPlayer
impl Clone for AudioPlayer {
    fn clone(&self) -> Self {
        Self {
            queues: self.queues.clone(),
            effects: self.effects.clone(),
            current_tracks: self.current_tracks.clone(),
        }
    }
}

/// Handler para cuando termina una canción
struct TrackEndHandler {
    player: Arc<AudioPlayer>,
    guild_id: GuildId,
    handler: Arc<Mutex<Call>>,
}

#[async_trait::async_trait]
impl VoiceEventHandler for TrackEndHandler {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        debug!("Track terminado, reproduciendo siguiente...");

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
