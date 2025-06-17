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
    /// Guarda información de track en cache con TTL de 1 hora
    pub fn cache_track(&self, url: String, info: CachedTrackInfo) {
        let ttl = Some(Duration::from_secs(3600)); // 1 hora
        self.insert_with_ttl(url, info, ttl);
    }

    /// Obtiene información de track del cache
    pub fn get_track(&self, url: &str) -> Option<CachedTrackInfo> {
        self.get(&url.to_string())
    }

    /// Limpia entradas antiguas y obtiene estadísticas
    pub fn cleanup_old_entries(&self) {
        let removed = self.cleanup_expired();
        if removed > 0 {
            info!("🧹 Limpiadas {} entradas expiradas del cache", removed);
        }
    }

    /// Obtiene estadísticas del cache para monitoreo
    pub fn get_stats(&self) -> CacheStats {
        let metrics = self.metrics();
        let size = self.len();
        let capacity = self.capacity();

        CacheStats {
            size,
            capacity,
            hit_rate: metrics.hit_rate(),
            miss_rate: metrics.miss_rate(),
            hits: metrics.hits,
            misses: metrics.misses,
            evictions: metrics.evictions,
            expired_removals: metrics.expired_removals,
            utilization: size as f64 / capacity as f64,
        }
    }
}

/// Estadísticas del cache para reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub size: usize,
    pub capacity: usize,
    pub hit_rate: f64,
    pub miss_rate: f64,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub expired_removals: u64,
    pub utilization: f64,
}
