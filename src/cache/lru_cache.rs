use dashmap::DashMap;
use std::{
    hash::Hash,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tracing::debug;

/// Cache entry con TTL simplificado
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct CacheEntry<V> {
    value: V,
    created_at: u64,
    ttl: Option<Duration>,
}

impl<V> CacheEntry<V> {
    #[allow(dead_code)]
    fn new(value: V, ttl: Option<Duration>) -> Self {
        Self {
            value,
            created_at: current_timestamp(),
            ttl,
        }
    }

    #[allow(dead_code)]
    fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl {
            let now = current_timestamp();
            now > self.created_at + ttl.as_secs()
        } else {
            false
        }
    }
}

/// Cache LRU simplificado para el proyecto
#[derive(Debug)]
pub struct LRUCache<K: Clone + Eq + Hash, V> {
    data: Arc<DashMap<K, CacheEntry<V>>>,
}

impl<K, V> LRUCache<K, V>
where
    K: Clone + Eq + Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub fn new(_capacity: usize) -> Self {
        Self {
            data: Arc::new(DashMap::new()),
        }
    }

    #[allow(dead_code)]
    pub fn insert_with_ttl(&self, key: K, value: V, ttl: Option<Duration>) -> Option<V> {
        let entry = CacheEntry::new(value, ttl);
        self.data.insert(key, entry).map(|old| old.value)
    }

    #[allow(dead_code)]
    pub fn get(&self, key: &K) -> Option<V> {
        if let Some(entry) = self.data.get(key) {
            if entry.is_expired() {
                drop(entry);
                self.data.remove(key);
                None
            } else {
                Some(entry.value.clone())
            }
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[allow(dead_code)]
    pub fn capacity(&self) -> usize {
        1000 // Capacidad fija simplificada
    }

    /// Limpia entradas expiradas y retorna el número de elementos removidos
    #[allow(dead_code)]
    pub fn cleanup_expired(&self) -> usize {
        let mut removed = 0;
        let keys_to_remove: Vec<K> = self.data
            .iter()
            .filter_map(|entry| {
                if entry.value().is_expired() {
                    Some(entry.key().clone())
                } else {
                    None
                }
            })
            .collect();

        for key in keys_to_remove {
            if self.data.remove(&key).is_some() {
                removed += 1;
            }
        }

        if removed > 0 {
            debug!("Limpiadas {} entradas expiradas del cache", removed);
        }

        removed
    }

    #[allow(dead_code)]
    pub fn metrics(&self) -> CacheMetrics {
        CacheMetrics {
            hits: 0,
            misses: 0,
            evictions: 0,
            expired_removals: 0,
        }
    }
}

impl<K, V> Clone for LRUCache<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
        }
    }
}

/// Métricas básicas del cache
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CacheMetrics {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub expired_removals: u64,
}

impl CacheMetrics {
    #[allow(dead_code)]
    pub fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 {
            0.0
        } else {
            self.hits as f64 / (self.hits + self.misses) as f64
        }
    }

    #[allow(dead_code)]
    pub fn miss_rate(&self) -> f64 {
        1.0 - self.hit_rate()
    }
}

/// Obtiene timestamp actual en segundos
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}