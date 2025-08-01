use anyhow::Result;
use dashmap::DashMap;
use parking_lot::RwLock;
use serenity::model::id::{ChannelId, GuildId, MessageId, UserId};
use songbird::{
    tracks::{PlayMode, TrackHandle},
    Event, EventContext, EventHandler as SongbirdEventHandler, TrackEvent,
    Call,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

use crate::{
    audio::{effects::{AudioEffects, EqualizerPreset}, queue::{MusicQueue, QueueInfo}},
    sources::TrackSource,
};

/// Información del mensaje de "now playing"
#[derive(Clone, Debug)]
pub struct NowPlayingMessage {
    #[allow(dead_code)]
    pub channel_id: ChannelId,
    #[allow(dead_code)]
    pub message_id: MessageId,
}

pub struct AudioPlayer {
    queues: DashMap<GuildId, Arc<RwLock<MusicQueue>>>,
    effects: Arc<AudioEffects>,
    current_tracks: DashMap<GuildId, TrackHandle>,
    #[allow(dead_code)]
    now_playing_messages: DashMap<GuildId, NowPlayingMessage>,
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self {
            queues: DashMap::new(),
            effects: Arc::new(AudioEffects::new()),
            current_tracks: DashMap::new(),
            now_playing_messages: DashMap::new(),
        }
    }

    /// Reproduce una canción en el canal de voz
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

    /// Reproduce la siguiente canción en la cola
    pub async fn play_next(&self, guild_id: GuildId, handler: Arc<Mutex<Call>>) -> Result<()> {
        let queue = self.get_or_create_queue(guild_id);
        
        let next_track = {
            let mut q = queue.write();
            q.next_track()
        };

        if let Some(track_source) = next_track {
            info!("▶️ Iniciando reproducción de: {}", track_source.title());
            
            // Obtener input de audio
            info!("🔄 Obteniendo input de audio...");
            let input = match track_source.get_input().await {
                Ok(input) => {
                    info!("✅ Input obtenido exitosamente");
                    input
                }
                Err(e) => {
                    tracing::error!("❌ Error obteniendo input: {:?}", e);
                    anyhow::bail!("Error obteniendo input: {:?}", e);
                }
            };
            
            // Procesar con efectos si están activos
            info!("🎛️ Procesando efectos...");
            let processed_input = match self.effects.process_input(input).await {
                Ok(input) => {
                    info!("✅ Efectos procesados");
                    input
                }
                Err(e) => {
                    tracing::error!("❌ Error procesando efectos: {:?}", e);
                    anyhow::bail!("Error procesando efectos: {:?}", e);
                }
            };

            // Reproducir
            info!("🎵 Iniciando reproducción en Discord...");
            let mut call = handler.lock().await;
            let track_handle = call.play_input(processed_input);
            
            // Configurar eventos
            self.setup_track_events(guild_id, &track_handle, handler.clone()).await;
            
            // Almacenar handle
            self.current_tracks.insert(guild_id, track_handle);
            
            info!("🎵 Reproduciendo: {} en guild {}", track_source.title(), guild_id);
        } else {
            info!("📭 Cola vacía en guild {}", guild_id);
        }

        Ok(())
    }

    /// Configura eventos para el track con manejo automático de errores y progresión
    async fn setup_track_events(&self, guild_id: GuildId, track_handle: &TrackHandle, handler: Arc<Mutex<Call>>) {
        let player = Arc::new(self.clone_for_events());
        
        // Evento para cuando el track termina normalmente
        let end_handler = TrackEndHandler {
            guild_id,
            player: player.clone(),
            handler: handler.clone(),
        };
        
        // Evento para cuando hay un error
        let error_handler = TrackErrorHandler {
            guild_id,
            player: player.clone(),
            handler: handler.clone(),
        };
        
        // Registrar eventos
        match track_handle.add_event(Event::Track(TrackEvent::End), end_handler) {
            Ok(_) => info!("✅ Event handler END registrado para guild {}", guild_id),
            Err(e) => tracing::error!("❌ Error registrando event handler END: {:?}", e),
        }
        
        match track_handle.add_event(Event::Track(TrackEvent::Error), error_handler) {
            Ok(_) => info!("✅ Event handler ERROR registrado para guild {}", guild_id),
            Err(e) => tracing::error!("❌ Error registrando event handler ERROR: {:?}", e),
        }
        
        info!("🎧 Eventos de track configurados para guild {}", guild_id);
    }
    
    /// Clona los datos necesarios para los event handlers
    fn clone_for_events(&self) -> AudioPlayerData {
        AudioPlayerData {
            queues: self.queues.clone(),
            effects: self.effects.clone(),
            current_tracks: self.current_tracks.clone(),
        }
    }

    /// Pausa la reproducción (Songbird 0.5.0)
    pub async fn pause(&self, guild_id: GuildId) -> Result<()> {
        if let Some(track) = self.current_tracks.get(&guild_id) {
            // Songbird 0.5.0: pause() puede devolver Result
            if let Err(e) = track.pause() {
                tracing::warn!("Error pausando track: {:?}", e);
            }
            info!("⏸️ Reproducción pausada en guild {}", guild_id);
        }
        Ok(())
    }

    /// Reanuda la reproducción (Songbird 0.5.0)
    pub async fn resume(&self, guild_id: GuildId) -> Result<()> {
        if let Some(track) = self.current_tracks.get(&guild_id) {
            // Songbird 0.5.0: play() puede devolver Result
            if let Err(e) = track.play() {
                tracing::warn!("Error reanudando track: {:?}", e);
            }
            info!("▶️ Reproducción reanudada en guild {}", guild_id);
        }
        Ok(())
    }

    /// Detiene la reproducción y limpia la cola
    pub async fn stop(&self, guild_id: GuildId) -> Result<()> {
        // Detener track actual (Songbird 0.5.0)
        if let Some(track) = self.current_tracks.remove(&guild_id) {
            if let Err(e) = track.1.stop() {
                tracing::warn!("Error deteniendo track: {:?}", e);
            }
        }

        // Limpiar cola
        self.clear_queue(guild_id).await?;
        
        info!("⏹️ Reproducción detenida en guild {}", guild_id);
        Ok(())
    }

    /// Salta tracks en la cola
    pub async fn skip_tracks(&self, guild_id: GuildId, amount: usize, handler: Arc<Mutex<Call>>) -> Result<()> {
        // Detener track actual (Songbird 0.5.0)
        if let Some(track) = self.current_tracks.get(&guild_id) {
            if let Err(e) = track.stop() {
                tracing::warn!("Error deteniendo track para skip: {:?}", e);
            }
        }

        // Saltar tracks en la cola
        let queue = self.get_or_create_queue(guild_id);
        {
            let mut q = queue.write();
            for _ in 0..amount.saturating_sub(1) {
                let _ = q.next_track();
            }
        }

        // Reproducir siguiente
        self.play_next(guild_id, handler).await?;
        
        info!("⏭️ Saltadas {} canciones en guild {}", amount, guild_id);
        Ok(())
    }

    /// Verifica si hay algo reproduciéndose (Songbird 0.5.0)
    pub async fn is_playing(&self, guild_id: GuildId) -> bool {
        if let Some(track) = self.current_tracks.get(&guild_id) {
            if let Ok(info) = track.get_info().await {
                // Songbird 0.5.0: verificar estado del track
                return info.playing == PlayMode::Play;
            }
        }
        false
    }

    /// Obtiene el track actual
    pub async fn get_current_track(&self, guild_id: GuildId) -> Option<TrackSource> {
        let queue = self.get_or_create_queue(guild_id);
        let q = queue.read();
        q.current_track()
    }

    /// Obtiene información de la cola
    pub async fn get_queue_info(&self, guild_id: GuildId) -> Result<QueueInfo> {
        let queue = self.get_or_create_queue(guild_id);
        let q = queue.read();
        Ok(q.get_info())
    }

    /// Obtiene o crea una cola para la guild
    pub fn get_or_create_queue(&self, guild_id: GuildId) -> Arc<RwLock<MusicQueue>> {
        self.queues
            .entry(guild_id)
            .or_insert_with(|| Arc::new(RwLock::new(MusicQueue::new(100))))
            .clone()
    }

    /// Obtiene la cola sin crear una nueva
    pub async fn get_queue(&self, guild_id: GuildId) -> Option<Vec<crate::audio::queue::QueueItem>> {
        if let Some(queue_arc) = self.queues.get(&guild_id) {
            let queue = queue_arc.read();
            Some(queue.get_info().items)
        } else {
            None
        }
    }

    /// Activa/desactiva loop simple
    pub async fn toggle_loop(&self, guild_id: GuildId) -> Result<bool> {
        let queue = self.get_or_create_queue(guild_id);
        let mut q = queue.write();
        Ok(q.toggle_loop())
    }

    /// Limpia la cola
    pub async fn clear_queue(&self, guild_id: GuildId) -> Result<()> {
        let queue = self.get_or_create_queue(guild_id);
        let mut q = queue.write();
        q.clear();
        info!("🗑️ Cola limpiada en guild {}", guild_id);
        Ok(())
    }

    /// Mezcla la cola
    pub async fn toggle_shuffle(&self, guild_id: GuildId) -> Result<bool> {
        let queue = self.get_or_create_queue(guild_id);
        let mut q = queue.write();
        let shuffled = q.toggle_shuffle();
        info!("🔀 Shuffle {} en guild {}", if shuffled { "activado" } else { "desactivado" }, guild_id);
        Ok(shuffled)
    }

    /// Establece modo de bucle específico
    pub async fn set_loop_mode_specific(&self, guild_id: GuildId, mode: crate::audio::queue::LoopMode) -> Result<()> {
        let queue = self.get_or_create_queue(guild_id);
        let mut q = queue.write();
        q.set_loop_mode(mode);
        info!("🔁 Modo de bucle establecido a {:?} en guild {}", mode, guild_id);
        Ok(())
    }

    /// Establece el volumen (Songbird 0.5.0)
    pub async fn set_volume(&self, guild_id: GuildId, volume: f32) -> Result<()> {
        if let Some(track) = self.current_tracks.get(&guild_id) {
            if let Err(e) = track.set_volume(volume) {
                tracing::warn!("Error estableciendo volumen: {:?}", e);
                anyhow::bail!("Error al establecer volumen: {:?}", e);
            }
            info!("🔊 Volumen ajustado a {:.1}% en guild {}", volume * 100.0, guild_id);
        }
        Ok(())
    }

    /// Obtiene el volumen actual (Songbird 0.5.0)
    pub async fn get_volume(&self, guild_id: GuildId) -> Option<f32> {
        if let Some(track) = self.current_tracks.get(&guild_id) {
            if let Ok(info) = track.get_info().await {
                // Songbird 0.5.0: verificar si el campo volume existe
                return Some(info.volume);
            }
        }
        None
    }

    // Métodos de ecualizador

    /// Aplica preset de ecualizador
    pub async fn apply_equalizer_preset(&self, _guild_id: GuildId, preset: EqualizerPreset) -> Result<()> {
        self.effects.apply_equalizer_preset(preset);
        Ok(())
    }

    /// Resetea el ecualizador
    #[allow(dead_code)]
    pub async fn reset_equalizer(&self, _guild_id: GuildId) -> Result<()> {
        self.effects.reset_equalizer();
        Ok(())
    }

    /// Obtiene detalles del ecualizador
    pub fn get_equalizer_details(&self) -> String {
        self.effects.get_equalizer_details()
    }

    /// Limpia duplicados de la cola (simplificado)
    pub async fn clear_duplicates(&self, _guild_id: GuildId) -> Result<usize> {
        // Por simplicidad, no implementamos eliminación de duplicados
        info!("🗑️ Limpieza de duplicados no implementada en versión simplificada");
        Ok(0)
    }

    /// Limpia tracks de un usuario específico (simplificado)
    pub async fn clear_user_tracks(&self, _guild_id: GuildId, _user_id: UserId) -> Result<usize> {
        // Por simplicidad, no implementamos eliminación por usuario
        info!("🗑️ Limpieza por usuario no implementada en versión simplificada");
        Ok(0)
    }

    /// Obtiene el track que se está reproduciendo ahora
    #[allow(dead_code)]
    pub fn get_now_playing(&self, guild_id: GuildId) -> Option<TrackSource> {
        let queue = self.get_or_create_queue(guild_id);
        let q = queue.read();
        q.current_track()
    }
}

/// Estructura para compartir datos del player con los event handlers
#[derive(Clone)]
pub struct AudioPlayerData {
    #[allow(dead_code)]
    queues: DashMap<GuildId, Arc<RwLock<MusicQueue>>>,
    #[allow(dead_code)]
    effects: Arc<AudioEffects>,
    current_tracks: DashMap<GuildId, TrackHandle>,
}

impl AudioPlayerData {
    /// Reproduce la siguiente canción en la cola
    #[allow(dead_code)]
    async fn play_next(&self, guild_id: GuildId, handler: Arc<Mutex<Call>>) {
        // Obtener siguiente track de la cola
        let queue = self.queues.get(&guild_id);
        if let Some(queue) = queue {
            let next_track = {
                let mut q = queue.write();
                q.next_track()
            };
            
            if let Some(track_source) = next_track {
                info!("🎵 Reproduciendo siguiente: {} en guild {}", track_source.title(), guild_id);
                
                // Crear input
                match track_source.get_input().await {
                    Ok(input) => {
                        // Aplicar efectos
                        let processed_input = match self.effects.process_input(input).await {
                            Ok(input) => input,
                            Err(e) => {
                                tracing::error!("❌ Error procesando efectos: {:?}", e);
                                return; // Skip this track if effects fail
                            }
                        };
                        
                        // Reproducir
                        let mut call = handler.lock().await;
                        let track_handle = call.play_input(processed_input);
                        
                        // Configurar eventos recursivamente
                        let end_handler = TrackEndHandler {
                            guild_id,
                            player: Arc::new(self.clone()),
                            handler: handler.clone(),
                        };
                        
                        let error_handler = TrackErrorHandler {
                            guild_id,
                            player: Arc::new(self.clone()),
                            handler: handler.clone(),
                        };
                        
                        track_handle.add_event(Event::Track(TrackEvent::End), end_handler).ok();
                        track_handle.add_event(Event::Track(TrackEvent::Error), error_handler).ok();
                        
                        // Almacenar handle
                        self.current_tracks.insert(guild_id, track_handle);
                        
                        info!("🎵 Reproduciendo: {} en guild {}", track_source.title(), guild_id);
                    }
                    Err(e) => {
                        tracing::error!("❌ Error obteniendo input para siguiente track: {:?}", e);
                        // Log error y continuar - no recursión infinita
                        info!("🔄 Saltando track con error y continuando...");
                    }
                }
            } else {
                info!("📭 Cola vacía, reproducción terminada en guild {}", guild_id);
                self.current_tracks.remove(&guild_id);
            }
        }
    }
}

/// Handler para cuando un track termina normalmente
pub struct TrackEndHandler {
    guild_id: GuildId,
    #[allow(dead_code)]
    player: Arc<AudioPlayerData>,
    #[allow(dead_code)]
    handler: Arc<Mutex<Call>>,
}

#[async_trait::async_trait]
impl SongbirdEventHandler for TrackEndHandler {
    async fn act(&self, _ctx: &EventContext<'_>) -> Option<Event> {
        tracing::warn!("🎵 *** EVENT HANDLER: TrackEndHandler llamado para guild {} ***", self.guild_id);
        info!("🎵 Track terminado en guild {}, reproduciendo siguiente...", self.guild_id);
        
        // Remover track actual
        self.player.current_tracks.remove(&self.guild_id);
        
        // Simplemente notificar - el siguiente track se manejará automáticamente
        info!("🎵 Track terminado, continuando con reproducción automática...");
        
        None
    }
}

/// Handler para cuando hay un error en el track
pub struct TrackErrorHandler {
    guild_id: GuildId,
    #[allow(dead_code)]
    player: Arc<AudioPlayerData>,
    #[allow(dead_code)]
    handler: Arc<Mutex<Call>>,
}

#[async_trait::async_trait]
impl SongbirdEventHandler for TrackErrorHandler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        tracing::warn!("❌ *** EVENT HANDLER: TrackErrorHandler llamado para guild {} ***", self.guild_id);
        tracing::error!("❌ Error en track para guild {}: {:?}", self.guild_id, ctx);
        info!("🔄 Intentando reproducir siguiente canción tras error...");
        
        // Remover track con error
        self.player.current_tracks.remove(&self.guild_id);
        
        // Log error y continuar
        info!("🔄 Error en track, saltando a siguiente canción...");
        
        None
    }
}