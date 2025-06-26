use anyhow::Result;
use chrono::{DateTime, Utc};
use serenity::model::id::{GuildId, UserId};
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tokio::sync::{RwLock, Mutex};
use tracing::{debug, info, warn, error};

use crate::sources::TrackSource;
use super::queue::{QueueItem, LoopMode, QueueInfo, QueuePage};

/// Sistema robusto de cola con manejo de errores avanzado y recuperación automática
#[derive(Debug)]
pub struct RobustQueue {
    inner: Arc<RwLock<MusicQueue>>,
    error_recovery: Arc<Mutex<ErrorRecovery>>,
    guild_id: GuildId,
}

#[derive(Debug)]
struct MusicQueue {
    items: VecDeque<QueueItem>,
    current: Option<QueueItem>,
    history: Vec<QueueItem>,
    loop_mode: LoopMode,
    shuffle: bool,
    max_size: usize,
    max_history: usize,
    failed_tracks: Vec<QueueItem>, // Tracks que fallaron al reproducirse
    retry_count: std::collections::HashMap<String, u8>, // URL -> retry count
}

#[derive(Debug)]
struct ErrorRecovery {
    consecutive_failures: u8,
    last_failure_time: Option<DateTime<Utc>>,
    skip_failed_tracks: bool,
    max_retries: u8,
    recovery_mode: bool,
}

impl RobustQueue {
    pub fn new(guild_id: GuildId, max_size: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(MusicQueue {
                items: VecDeque::new(),
                current: None,
                history: Vec::new(),
                loop_mode: LoopMode::Off,
                shuffle: false,
                max_size,
                max_history: 50,
                failed_tracks: Vec::new(),
                retry_count: std::collections::HashMap::new(),
            })),
            error_recovery: Arc::new(Mutex::new(ErrorRecovery {
                consecutive_failures: 0,
                last_failure_time: None,
                skip_failed_tracks: true,
                max_retries: 3,
                recovery_mode: false,
            })),
            guild_id,
        }
    }

    /// Agrega un track a la cola con validación
    pub async fn add_track(&self, source: TrackSource) -> Result<()> {
        let mut queue = self.inner.write().await;
        
        if queue.items.len() >= queue.max_size {
            anyhow::bail!("La cola está llena (máximo {} canciones)", queue.max_size);
        }

        // Verificar si el track ya falló demasiadas veces
        if let Some(&retry_count) = queue.retry_count.get(source.url()) {
            let recovery = self.error_recovery.lock().await;
            if retry_count >= recovery.max_retries {
                warn!("🚫 Track {} ha fallado {} veces, no se agregará", source.title(), retry_count);
                anyhow::bail!("La canción ha fallado demasiadas veces y fue marcada como problemática");
            }
        }

        let item = QueueItem::from(source);
        info!("➕ Agregado a la cola de forma robusta: {}", item.title);
        queue.items.push_back(item);

        Ok(())
    }

    /// Obtiene el siguiente track con manejo robusto de errores
    pub async fn next_track(&self) -> Option<TrackSource> {
        let mut queue = self.inner.write().await;
        let mut recovery = self.error_recovery.lock().await;

        // Guardar current en history si existe
        if let Some(current) = queue.current.take() {
            queue.add_to_history(current.clone());

            // Si está en modo loop track, devolver el mismo (si no ha fallado)
            if queue.loop_mode == LoopMode::Track {
                let retry_count = queue.retry_count.get(&current.url).copied().unwrap_or(0);
                if retry_count < recovery.max_retries {
                    queue.current = Some(current.clone());
                    return Some(current.source);
                } else {
                    warn!("🔂 Track en loop ha fallado demasiadas veces, saltando");
                }
            }
        }

        // Intentar obtener siguiente track válido
        while let Some(next_item) = self.get_next_item(&mut queue).await {
            let retry_count = queue.retry_count.get(&next_item.url).copied().unwrap_or(0);
            
            // Si el track ha fallado demasiadas veces, saltarlo si está en modo recovery
            if recovery.skip_failed_tracks && retry_count >= recovery.max_retries {
                warn!("⏭️ Saltando track problemático: {}", next_item.title);
                queue.failed_tracks.push(next_item);
                continue;
            }

            // Si está en modo loop queue, agregar al final
            if queue.loop_mode == LoopMode::Queue {
                queue.items.push_back(next_item.clone());
            }

            queue.current = Some(next_item.clone());
            info!("🎵 Siguiente track seleccionado: {}", next_item.title);
            
            // Resetear contador de fallos consecutivos en track exitoso
            recovery.consecutive_failures = 0;
            recovery.recovery_mode = false;
            
            return Some(next_item.source);
        }

        // Si llegamos aquí, no hay más tracks válidos
        if !queue.items.is_empty() {
            warn!("⚠️ No hay tracks válidos disponibles, entrando en modo recovery");
            recovery.recovery_mode = true;
            
            // Intentar recuperar tracks fallidos si han pasado suficiente tiempo
            self.attempt_failed_track_recovery(&mut queue, &mut recovery).await
        } else {
            info!("📭 Cola vacía");
            None
        }
    }

    /// Obtiene el siguiente item considerando shuffle
    async fn get_next_item(&self, queue: &mut MusicQueue) -> Option<QueueItem> {
        if queue.items.is_empty() {
            return None;
        }

        if queue.shuffle {
            use rand::seq::SliceRandom;
            let mut rng = rand::thread_rng();
            let indices: Vec<usize> = (0..queue.items.len()).collect();
            if let Some(&index) = indices.choose(&mut rng) {
                queue.items.remove(index)
            } else {
                queue.items.pop_front()
            }
        } else {
            queue.items.pop_front()
        }
    }

    /// Intenta recuperar tracks fallidos después de un tiempo
    async fn attempt_failed_track_recovery(&self, queue: &mut MusicQueue, recovery: &mut ErrorRecovery) -> Option<TrackSource> {
        let now = Utc::now();
        
        // Solo intentar recovery si han pasado al menos 5 minutos desde el último fallo
        if let Some(last_failure) = recovery.last_failure_time {
            if now.signed_duration_since(last_failure).num_minutes() < 5 {
                return None;
            }
        }

        info!("🔄 Intentando recuperar tracks fallidos...");
        
        // Mover algunos tracks fallidos de vuelta a la cola para retry
        let failed_to_retry: Vec<QueueItem> = queue.failed_tracks.drain(..queue.failed_tracks.len().min(3)).collect();
        
        for item in failed_to_retry {
            // Resetear contador de retries para dar una nueva oportunidad
            queue.retry_count.insert(item.url.clone(), 0);
            queue.items.push_back(item);
            info!("🔄 Track recuperado para retry: {}", queue.items.back().unwrap().title);
        }

        // Intentar obtener siguiente track después de recovery
        if let Some(next_item) = self.get_next_item(queue).await {
            queue.current = Some(next_item.clone());
            Some(next_item.source)
        } else {
            None
        }
    }

    /// Reporta que un track falló al reproducirse
    pub async fn report_track_failure(&self, track_url: &str, error: &str) {
        let mut queue = self.inner.write().await;
        let mut recovery = self.error_recovery.lock().await;

        // Incrementar contador de retries para este track
        let retry_count = queue.retry_count.entry(track_url.to_string()).or_insert(0);
        *retry_count += 1;

        // Incrementar fallos consecutivos
        recovery.consecutive_failures += 1;
        recovery.last_failure_time = Some(Utc::now());

        error!("❌ Fallo en track (intento {}): {} - Error: {}", retry_count, track_url, error);

        // Si hay demasiados fallos consecutivos, activar modo recovery
        if recovery.consecutive_failures >= 3 {
            recovery.recovery_mode = true;
            recovery.skip_failed_tracks = true;
            warn!("🚨 Modo recovery activado debido a {} fallos consecutivos", recovery.consecutive_failures);
        }

        // Mover track actual a fallidos si excede máximo de retries
        if *retry_count >= recovery.max_retries {
            if let Some(current) = queue.current.take() {
                if current.url == track_url {
                    warn!("🚫 Moviendo track problemático a lista de fallidos: {}", current.title);
                    queue.failed_tracks.push(current);
                }
            }
        }
    }

    /// Reporta que un track se reprodujo exitosamente
    pub async fn report_track_success(&self, track_url: &str) {
        let mut queue = self.inner.write().await;
        let mut recovery = self.error_recovery.lock().await;

        // Limpiar contador de retries para este track
        queue.retry_count.remove(track_url);

        // Resetear contador de fallos consecutivos
        recovery.consecutive_failures = 0;
        recovery.recovery_mode = false;

        debug!("✅ Track reproducido exitosamente: {}", track_url);
    }

    /// Salta canciones con validación
    pub async fn skip(&self, amount: usize) -> usize {
        let mut queue = self.inner.write().await;
        let available = amount.min(queue.items.len());
        
        info!("⏭️ Saltando {} canciones", available);
        
        for _ in 0..available {
            if let Some(item) = queue.items.pop_front() {
                queue.add_to_history(item);
            }
        }

        available
    }

    /// Limpia la cola con confirmación
    pub async fn clear(&self) -> usize {
        let mut queue = self.inner.write().await;
        let cleared = queue.items.len();
        queue.items.clear();
        
        // También limpiar tracks fallidos y contadores
        queue.failed_tracks.clear();
        queue.retry_count.clear();
        
        info!("🗑️ Cola limpiada: {} tracks removidos", cleared);
        cleared
    }

    /// Limpia duplicados de forma inteligente
    pub async fn clear_duplicates(&self) -> usize {
        let mut queue = self.inner.write().await;
        let mut seen = std::collections::HashSet::new();
        let original_len = queue.items.len();

        queue.items.retain(|item| seen.insert(item.url.clone()));

        let removed = original_len - queue.items.len();
        if removed > 0 {
            info!("🗑️ Eliminados {} duplicados de forma inteligente", removed);
        }
        removed
    }

    /// Obtiene información completa de la cola
    pub async fn get_info(&self) -> QueueInfo {
        let queue = self.inner.read().await;
        let recovery = self.error_recovery.lock().await;
        
        let mut info = QueueInfo {
            current: queue.current.clone(),
            items: queue.items.iter().cloned().collect(),
            total_items: queue.items.len(),
            loop_mode: queue.loop_mode,
            shuffle: queue.shuffle,
            total_duration: self.calculate_total_duration(&queue).await,
        };

        // Agregar información de recovery si está activo
        if recovery.recovery_mode {
            debug!("🔄 Cola en modo recovery - {} tracks fallidos", queue.failed_tracks.len());
        }

        info
    }

    /// Calcula duración total con cache
    async fn calculate_total_duration(&self, queue: &MusicQueue) -> Duration {
        let queue_duration: Duration = queue.items.iter().filter_map(|item| item.duration).sum();
        let current_duration = queue.current.as_ref().and_then(|c| c.duration).unwrap_or_default();
        queue_duration + current_duration
    }

    /// Configuración avanzada de recovery
    pub async fn configure_recovery(&self, skip_failed: bool, max_retries: u8) {
        let mut recovery = self.error_recovery.lock().await;
        recovery.skip_failed_tracks = skip_failed;
        recovery.max_retries = max_retries;
        info!("⚙️ Recovery configurado: skip_failed={}, max_retries={}", skip_failed, max_retries);
    }

    /// Obtiene estadísticas de la cola
    pub async fn get_stats(&self) -> QueueStats {
        let queue = self.inner.read().await;
        let recovery = self.error_recovery.lock().await;

        QueueStats {
            total_items: queue.items.len(),
            failed_tracks: queue.failed_tracks.len(),
            retry_counts: queue.retry_count.len(),
            consecutive_failures: recovery.consecutive_failures,
            recovery_mode: recovery.recovery_mode,
            total_retries: queue.retry_count.values().sum::<u8>() as usize,
        }
    }

    /// Métodos de compatibilidad con la interfaz original
    
    pub async fn current(&self) -> Option<QueueItem> {
        let queue = self.inner.read().await;
        queue.current.clone()
    }

    pub async fn is_empty(&self) -> bool {
        let queue = self.inner.read().await;
        queue.items.is_empty() && queue.current.is_none()
    }

    pub async fn len(&self) -> usize {
        let queue = self.inner.read().await;
        queue.items.len()
    }

    pub async fn toggle_shuffle(&self) -> bool {
        let mut queue = self.inner.write().await;
        queue.shuffle = !queue.shuffle;
        if queue.shuffle {
            info!("🔀 Modo aleatorio activado (robusto)");
        } else {
            info!("➡️ Modo aleatorio desactivado (robusto)");
        }
        queue.shuffle
    }

    pub async fn set_loop_mode(&self, mode: LoopMode) {
        let mut queue = self.inner.write().await;
        queue.loop_mode = mode;
        match mode {
            LoopMode::Off => info!("➡️ Repetición desactivada (robusto)"),
            LoopMode::Track => info!("🔂 Repetir canción activado (robusto)"),
            LoopMode::Queue => info!("🔁 Repetir cola activado (robusto)"),
        }
    }

    pub async fn get_page(&self, page: usize, items_per_page: usize) -> QueuePage {
        let info = self.get_info().await;
        info.get_page(page, items_per_page)
    }
}

impl MusicQueue {
    fn add_to_history(&mut self, item: QueueItem) {
        self.history.push(item);
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueueStats {
    pub total_items: usize,
    pub failed_tracks: usize,
    pub retry_counts: usize,
    pub consecutive_failures: u8,
    pub recovery_mode: bool,
    pub total_retries: usize,
}