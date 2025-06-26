pub mod lru_cache;

use lru_cache::LRUCache;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::info;

/// Cache principal para metadata de música
pub type MusicCache = LRUCache<String, CachedTrackInfo>;

/// Información de track cacheada
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTrackInfo {
    pub title: String,
    pub artist: Option<String>,
    pub duration: Option<Duration>,
    pub thumbnail: Option<String>,
    pub url: String,
    pub source: String, // "youtube", "spotify", etc.
}

impl MusicCache {
    /// Limpia entradas antiguas y obtiene estadísticas
    pub fn cleanup_old_entries(&self) {
        let removed = self.cleanup_expired();
        if removed > 0 {
            info!("🧹 Limpiadas {} entradas expiradas del cache", removed);
        }
    }
}

