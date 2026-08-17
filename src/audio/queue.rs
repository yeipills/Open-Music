//! Vista de la cola y metadatos de pista.
//!
//! ## Qué cambió respecto a la versión anterior
//!
//! Antes este módulo **era** la cola: un `VecDeque` propio que vivía en paralelo
//! a la cola de songbird y que había que mantener sincronizado a mano con la
//! reproducción real. Cada desincronización se veía como una canción que se
//! repetía, dos temas sonando encima o un `/queue` que mentía.
//!
//! Ahora la única cola es [`songbird::tracks::TrackQueue`]. Este módulo aporta
//! dos cosas:
//!
//! 1. [`QueueItem`]: los metadatos que van **pegados** a cada pista de songbird
//!    (`Track::new_with_data`), de modo que no puedan separarse del audio.
//! 2. [`QueueInfo`] / [`QueuePage`]: proyecciones de sólo lectura que la UI
//!    consume, construidas a partir de la cola real en el momento de mirarla.

use chrono::{DateTime, Utc};
use serenity::model::id::UserId;
use songbird::tracks::TrackHandle;
use std::{sync::Arc, time::Duration};

use crate::sources::TrackSource;

/// Metadatos de una pista encolada.
///
/// Se adjunta a cada `Track` de songbird como *user data* y se recupera con
/// [`meta_of`]. Al viajar dentro de la propia pista, es imposible que la cola
/// diga una cosa y suene otra.
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

/// Recupera los metadatos adjuntos a una pista de la cola.
///
/// # Pánico
///
/// `TrackHandle::data` revienta si el tipo no coincide con el que se guardó.
/// Todas las pistas del bot se crean en [`crate::audio::player::build_track`],
/// que siempre adjunta un `Arc<QueueItem>`, así que la invariante se mantiene
/// mientras esa sea la única puerta de entrada a la cola.
pub fn meta_of(handle: &TrackHandle) -> Arc<QueueItem> {
    handle.data::<QueueItem>()
}

/// Modo de repetición de la guild.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoopMode {
    #[default]
    Off,
    /// Repite la canción actual indefinidamente (lo hace songbird).
    Track,
    /// Al terminar una canción vuelve al final de la cola.
    Queue,
}

/// Instantánea de la cola para pintar embeds.
#[derive(Debug, Clone)]
pub struct QueueInfo {
    /// Pista sonando ahora (la cabeza de la cola de songbird).
    pub current: Option<QueueItem>,
    /// Pistas pendientes, sin incluir la actual.
    pub items: Vec<QueueItem>,
    pub total_items: usize,
    pub loop_mode: LoopMode,
    pub shuffle: bool,
    /// Suma de duraciones de la actual más las pendientes.
    pub total_duration: Duration,
}

impl QueueInfo {
    /// Construye la instantánea a partir de la cola real de songbird.
    ///
    /// `handles` es el resultado de `queue.current_queue()`: el primer elemento
    /// es la pista en curso y el resto son las pendientes, en orden.
    pub fn from_handles(handles: &[TrackHandle], loop_mode: LoopMode, shuffle: bool) -> Self {
        let mut iter = handles.iter().map(|h| (*meta_of(h)).clone());
        let current = iter.next();
        let items: Vec<QueueItem> = iter.collect();

        let total_duration = current
            .as_ref()
            .and_then(|c| c.duration)
            .unwrap_or_default()
            + items
                .iter()
                .filter_map(|i| i.duration)
                .sum::<Duration>();

        Self {
            total_items: items.len(),
            current,
            items,
            loop_mode,
            shuffle,
            total_duration,
        }
    }

    /// Cola vacía, para cuando el bot no está conectado.
    pub fn empty(loop_mode: LoopMode, shuffle: bool) -> Self {
        Self {
            current: None,
            items: Vec::new(),
            total_items: 0,
            loop_mode,
            shuffle,
            total_duration: Duration::ZERO,
        }
    }

    /// Extrae una página de la cola para la vista paginada.
    pub fn get_page(&self, page: usize, items_per_page: usize) -> QueuePage {
        let items_per_page = items_per_page.max(1);
        let total_pages = self.total_items.div_ceil(items_per_page).max(1);
        let safe_page = page.clamp(1, total_pages);

        let start = (safe_page - 1) * items_per_page;
        let end = (start + items_per_page).min(self.items.len());

        QueuePage {
            items: if start < self.items.len() {
                self.items[start..end].to_vec()
            } else {
                Vec::new()
            },
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
