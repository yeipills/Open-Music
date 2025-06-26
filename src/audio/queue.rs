use anyhow::Result;
use chrono::{DateTime, Utc};
use rand::seq::SliceRandom;
use serenity::model::id::UserId;
use std::{collections::VecDeque, time::Duration};
use tracing::{debug, info};

use crate::sources::TrackSource;

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub source: TrackSource,
    pub title: String,
    pub artist: Option<String>,
    pub duration: Option<Duration>,
    pub thumbnail: Option<String>,
    pub url: String,
    pub requested_by: UserId,
    #[allow(dead_code)]
    pub added_at: DateTime<Utc>,
}

impl From<TrackSource> for QueueItem {
    fn from(source: TrackSource) -> Self {
        Self {
            title: source.title(),
            artist: source.artist(),
            duration: source.duration(),
            thumbnail: source.thumbnail(),
            url: source.url(),
            requested_by: source.requested_by(),
            added_at: Utc::now(),
            source,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoopMode {
    Off,
    Track,
    Queue,
}

#[derive(Debug)]
pub struct MusicQueue {
    items: VecDeque<QueueItem>,
    current: Option<QueueItem>,
    history: Vec<QueueItem>,
    loop_mode: LoopMode,
    shuffle: bool,
    max_size: usize,
    max_history: usize,
}

impl MusicQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            items: VecDeque::new(),
            current: None,
            history: Vec::new(),
            loop_mode: LoopMode::Off,
            shuffle: false,
            max_size,
            max_history: 50,
        }
    }

    /// Agrega un track a la cola
    pub fn add_track(&mut self, source: TrackSource) -> Result<()> {
        if self.items.len() >= self.max_size {
            anyhow::bail!("La cola está llena (máximo {} canciones)", self.max_size);
        }

        let item = QueueItem::from(source);
        info!("➕ Agregado a la cola: {}", item.title);
        self.items.push_back(item);

        Ok(())
    }

    /// Agrega múltiples tracks (playlist)
    #[allow(dead_code)]
    pub fn add_playlist(&mut self, sources: Vec<TrackSource>) -> Result<usize> {
        let available_space = self.max_size.saturating_sub(self.items.len());
        let to_add = sources.len().min(available_space);

        for source in sources.into_iter().take(to_add) {
            let item = QueueItem::from(source);
            self.items.push_back(item);
        }

        info!("➕ Agregadas {} canciones a la cola", to_add);
        Ok(to_add)
    }

    /// Obtiene el siguiente track (FIFO - First In, First Out)
    pub fn next_track(&mut self) -> Option<TrackSource> {
        // Guardar current en history si existe
        if let Some(current) = self.current.take() {
            self.add_to_history(current.clone());

            // Si está en modo loop track, devolver el mismo
            if self.loop_mode == LoopMode::Track {
                self.current = Some(current.clone());
                info!("🔂 Repitiendo track: {}", current.title);
                return Some(current.source);
            }
        }

        // Obtener siguiente de la cola - SIEMPRE en orden FIFO a menos que shuffle esté activo
        let next = if self.shuffle && !self.items.is_empty() {
            // Modo shuffle: elegir aleatorio
            let mut rng = rand::thread_rng();
            let index = (0..self.items.len())
                .collect::<Vec<_>>()
                .choose(&mut rng)
                .copied()
                .unwrap_or(0);
            let selected = self.items.remove(index);
            info!("🔀 Seleccionado aleatoriamente: {}", selected.as_ref().map(|s| s.title.as_str()).unwrap_or("Unknown"));
            selected
        } else {
            // Modo normal: ESTRICTO FIFO - primero en entrar, primero en salir
            let next_item = self.items.pop_front();
            if let Some(ref item) = next_item {
                info!("➡️ Siguiente en cola (FIFO): {}", item.title);
            }
            next_item
        };

        if let Some(next_item) = next {
            // Si está en modo loop queue, agregar al final
            if self.loop_mode == LoopMode::Queue {
                self.items.push_back(next_item.clone());
                info!("🔁 Track agregado al final por loop de cola: {}", next_item.title);
            }

            self.current = Some(next_item.clone());
            Some(next_item.source)
        } else {
            info!("📭 Cola vacía, no hay siguiente track");
            None
        }
    }

    /// Salta canciones
    #[allow(dead_code)]
    pub fn skip(&mut self, amount: usize) -> usize {
        let skipped = amount.min(self.items.len());

        for _ in 0..skipped {
            if let Some(item) = self.items.pop_front() {
                self.add_to_history(item);
            }
        }

        skipped
    }

    /// Limpia la cola
    pub fn clear(&mut self) {
        self.items.clear();
        info!("🗑️ Cola limpiada");
    }

    /// Limpia duplicados
    #[allow(dead_code)]
    pub fn clear_duplicates(&mut self) -> usize {
        let mut seen = std::collections::HashSet::new();
        let original_len = self.items.len();

        self.items.retain(|item| seen.insert(item.url.clone()));

        let removed = original_len - self.items.len();
        if removed > 0 {
            info!("🗑️ Eliminados {} duplicados", removed);
        }
        removed
    }

    /// Limpia tracks de un usuario específico
    #[allow(dead_code)]
    pub fn clear_user_tracks(&mut self, user_id: UserId) -> usize {
        let original_len = self.items.len();
        self.items.retain(|item| item.requested_by != user_id);

        let removed = original_len - self.items.len();
        if removed > 0 {
            info!("🗑️ Eliminadas {} canciones del usuario", removed);
        }
        removed
    }

    /// Mezcla la cola
    #[allow(dead_code)]
    pub fn shuffle_queue(&mut self) {
        let mut items: Vec<_> = self.items.drain(..).collect();
        let mut rng = rand::thread_rng();
        items.shuffle(&mut rng);
        self.items.extend(items);
        info!("🔀 Cola mezclada");
    }

    /// Cambia el modo de shuffle
    pub fn toggle_shuffle(&mut self) -> bool {
        self.shuffle = !self.shuffle;
        if self.shuffle {
            info!("🔀 Modo aleatorio activado");
        } else {
            info!("➡️ Modo aleatorio desactivado");
        }
        self.shuffle
    }

    /// Cambia el modo de loop
    pub fn set_loop_mode(&mut self, mode: LoopMode) {
        self.loop_mode = mode;
        match mode {
            LoopMode::Off => info!("➡️ Repetición desactivada"),
            LoopMode::Track => info!("🔂 Repetir canción activado"),
            LoopMode::Queue => info!("🔁 Repetir cola activado"),
        }
    }

    /// Obtiene información de la cola
    pub fn get_info(&self) -> QueueInfo {
        QueueInfo {
            current: self.current.clone(),
            items: self.items.iter().cloned().collect(),
            total_items: self.items.len(),
            loop_mode: self.loop_mode,
            shuffle: self.shuffle,
            total_duration: self.calculate_total_duration(),
        }
    }

    /// Obtiene el track actual
    #[allow(dead_code)]
    pub fn current(&self) -> Option<&QueueItem> {
        self.current.as_ref()
    }

    /// Verifica si la cola está vacía
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.current.is_none()
    }

    /// Obtiene el tamaño de la cola
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Mueve un track a una nueva posición
    #[allow(dead_code)]
    pub fn move_track(&mut self, from: usize, to: usize) -> Result<()> {
        if from >= self.items.len() || to >= self.items.len() {
            anyhow::bail!("Índice fuera de rango");
        }

        if from != to {
            let item = self
                .items
                .remove(from)
                .ok_or_else(|| anyhow::anyhow!("No se pudo remover el item"))?;
            self.items.insert(to, item);
            debug!("📍 Track movido de posición {} a {}", from, to);
        }

        Ok(())
    }

    /// Elimina un track específico
    #[allow(dead_code)]
    pub fn remove_track(&mut self, index: usize) -> Result<()> {
        if index >= self.items.len() {
            anyhow::bail!("Índice fuera de rango");
        }

        self.items.remove(index);
        debug!("❌ Track eliminado en posición {}", index);
        Ok(())
    }

    // Funciones privadas

    fn add_to_history(&mut self, item: QueueItem) {
        self.history.push(item);

        // Mantener solo los últimos N items
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }
    }

    fn calculate_total_duration(&self) -> Duration {
        let queue_duration: Duration = self.items.iter().filter_map(|item| item.duration).sum();

        let current_duration = self
            .current
            .as_ref()
            .and_then(|c| c.duration)
            .unwrap_or_default();

        queue_duration + current_duration
    }

    /// Métodos adicionales para compatibilidad

    /// Obtiene el track actual como TrackSource
    #[allow(dead_code)]
    pub fn current_track(&self) -> Option<TrackSource> {
        self.current.as_ref().map(|item| item.source.clone())
    }

    /// Obtiene todos los tracks como Vec<TrackSource>
    #[allow(dead_code)]
    pub fn get_tracks(&self) -> Vec<TrackSource> {
        self.items.iter().map(|item| item.source.clone()).collect()
    }

    /// Obtiene la posición actual
    #[allow(dead_code)]
    pub fn current_position(&self) -> usize {
        if self.current.is_some() {
            0
        } else {
            0
        }
    }

    /// Verifica si shuffle está activado
    #[allow(dead_code)]
    pub fn is_shuffle(&self) -> bool {
        self.shuffle
    }

    /// Verifica si loop está activado
    #[allow(dead_code)]
    pub fn is_loop(&self) -> bool {
        matches!(self.loop_mode, LoopMode::Track | LoopMode::Queue)
    }

    /// Activa/desactiva loop (modo simple)
    pub fn toggle_loop(&mut self) -> bool {
        match self.loop_mode {
            LoopMode::Off => {
                self.set_loop_mode(LoopMode::Queue);
                true
            }
            _ => {
                self.set_loop_mode(LoopMode::Off);
                false
            }
        }
    }

    /// Configura loop simple (on/off)
    #[allow(dead_code)]
    pub fn set_loop(&mut self, enabled: bool) {
        if enabled {
            self.set_loop_mode(LoopMode::Queue);
        } else {
            self.set_loop_mode(LoopMode::Off);
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueueInfo {
    pub current: Option<QueueItem>,
    pub items: Vec<QueueItem>,
    pub total_items: usize,
    pub loop_mode: LoopMode,
    pub shuffle: bool,
    pub total_duration: Duration,
}

impl QueueInfo {
    /// Obtiene una página específica de la cola
    pub fn get_page(&self, page: usize, items_per_page: usize) -> QueuePage {
        let safe_page = page.max(1);
        let start = (safe_page - 1) * items_per_page;
        let end = (start + items_per_page).min(self.items.len());
        let total_pages = if self.total_items == 0 { 1 } else { (self.total_items + items_per_page - 1) / items_per_page };

        QueuePage {
            items: if start < self.items.len() { self.items[start..end].to_vec() } else { Vec::new() },
            current_page: safe_page,
            total_pages,
            total_items: self.total_items,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueuePage {
    pub items: Vec<QueueItem>,
    pub current_page: usize,
    pub total_pages: usize,
    #[allow(dead_code)]
    pub total_items: usize,
}
