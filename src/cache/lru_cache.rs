use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tracing::debug;

/// Cache entry con TTL
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry<V> {
    value: V,
    created_at: u64,
    last_accessed: u64,
    access_count: u64,
    ttl: Option<Duration>,
}

impl<V> CacheEntry<V> {
    fn new(value: V, ttl: Option<Duration>) -> Self {
        let now = current_timestamp();
        Self {
            value,
            created_at: now,
            last_accessed: now,
            access_count: 1,
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl {
            let now = current_timestamp();
            now > self.created_at + ttl.as_secs()
        } else {
            false
        }
    }

    fn access(&mut self) -> &V {
        self.last_accessed = current_timestamp();
        self.access_count += 1;
        &self.value
    }
}

/// Estrategia de eviction para el cache
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EvictionPolicy {
    LRU,  // Least Recently Used
    LFU,  // Least Frequently Used
    FIFO, // First In, First Out
}

/// Cache LRU thread-safe con TTL y métricas
#[derive(Debug)]
pub struct LRUCache<K: Clone + Eq + Hash, V> {
    entries: DashMap<K, CacheEntry<V>>,
    access_order: Arc<RwLock<VecDeque<K>>>,
    max_size: usize,
    policy: EvictionPolicy,
    metrics: Arc<RwLock<CacheMetrics>>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CacheMetrics {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub expired_removals: u64,
    pub total_requests: u64,
}

impl CacheMetrics {
    pub fn hit_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.hits as f64 / self.total_requests as f64
        }
    }

    pub fn miss_rate(&self) -> f64 {
        1.0 - self.hit_rate()
    }
}

impl<K, V> LRUCache<K, V>
where
    K: Clone + Eq + Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Crea un nuevo cache LRU
    pub fn new(max_size: usize) -> Self {
        Self::with_policy(max_size, EvictionPolicy::LRU)
    }

    /// Crea un cache con política de eviction específica
    pub fn with_policy(max_size: usize, policy: EvictionPolicy) -> Self {
        Self {
            entries: DashMap::new(),
            access_order: Arc::new(RwLock::new(VecDeque::new())),
            max_size,
            policy,
            metrics: Arc::new(RwLock::new(CacheMetrics::default())),
        }
    }

    /// Inserta un valor en el cache
    pub fn insert(&self, key: K, value: V) -> Option<V> {
        self.insert_with_ttl(key, value, None)
    }

    /// Inserta un valor con TTL
    pub fn insert_with_ttl(&self, key: K, value: V, ttl: Option<Duration>) -> Option<V> {
        // Verificar si necesitamos hacer espacio
        if self.entries.len() >= self.max_size && !self.entries.contains_key(&key) {
            self.evict_one();
        }

        let entry = CacheEntry::new(value, ttl);
        let old_value = self.entries.insert(key.clone(), entry).map(|e| e.value);

        // Actualizar orden de acceso
        self.update_access_order(key);

        debug!("Cache: Insertado item");
        old_value
    }

    /// Obtiene un valor del cache
    pub fn get(&self, key: &K) -> Option<V> {
        let mut metrics = self.metrics.write();
        metrics.total_requests += 1;

        if let Some(mut entry_ref) = self.entries.get_mut(key) {
            if entry_ref.is_expired() {
                metrics.expired_removals += 1;
                metrics.misses += 1;
                drop(entry_ref);
                self.entries.remove(key);
                self.remove_from_access_order(key);
                return None;
            }

            let value = entry_ref.access().clone();
            metrics.hits += 1;
            drop(entry_ref);
            drop(metrics);

            self.update_access_order(key.clone());
            Some(value)
        } else {
            metrics.misses += 1;
            None
        }
    }

    /// Elimina un valor del cache
    pub fn remove(&self, key: &K) -> Option<V> {
        self.remove_from_access_order(key);
        self.entries.remove(key).map(|(_, entry)| entry.value)
    }

    /// Limpia el cache
    pub fn clear(&self) {
        self.entries.clear();
        self.access_order.write().clear();
        debug!("Cache limpiado");
    }

    /// Obtiene el tamaño actual del cache
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Verifica si el cache está vacío
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Obtiene la capacidad máxima
    pub fn capacity(&self) -> usize {
        self.max_size
    }

    /// Verifica si contiene una clave
    pub fn contains_key(&self, key: &K) -> bool {
        if let Some(entry) = self.entries.get(key) {
            !entry.is_expired()
        } else {
            false
        }
    }

    /// Limpia entradas expiradas
    pub fn cleanup_expired(&self) -> usize {
        let mut removed_count = 0;
        let mut expired_keys = Vec::new();

        // Recopilar claves expiradas
        for entry in self.entries.iter() {
            if entry.value().is_expired() {
                expired_keys.push(entry.key().clone());
            }
        }

        // Eliminar entradas expiradas
        for key in expired_keys {
            if self.entries.remove(&key).is_some() {
                self.remove_from_access_order(&key);
                removed_count += 1;
            }
        }

        if removed_count > 0 {
            let mut metrics = self.metrics.write();
            metrics.expired_removals += removed_count as u64;
            debug!("Cache: Eliminadas {} entradas expiradas", removed_count);
        }

        removed_count
    }

    /// Obtiene métricas del cache
    pub fn metrics(&self) -> CacheMetrics {
        self.metrics.read().clone()
    }

    /// Resetea métricas
    pub fn reset_metrics(&self) {
        *self.metrics.write() = CacheMetrics::default();
    }

    /// Obtiene todas las claves
    pub fn keys(&self) -> Vec<K> {
        self.entries
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Obtiene estadísticas detalladas
    pub fn stats(&self) -> HashMap<String, serde_json::Value> {
        let metrics = self.metrics.read();
        let mut stats = HashMap::new();

        stats.insert(
            "size".to_string(),
            serde_json::Value::Number(self.len().into()),
        );
        stats.insert(
            "capacity".to_string(),
            serde_json::Value::Number(self.capacity().into()),
        );
        stats.insert(
            "hits".to_string(),
            serde_json::Value::Number(metrics.hits.into()),
        );
        stats.insert(
            "misses".to_string(),
            serde_json::Value::Number(metrics.misses.into()),
        );
        stats.insert(
            "hit_rate".to_string(),
            serde_json::Value::Number(
                serde_json::Number::from_f64(metrics.hit_rate())
                    .unwrap_or_else(|| serde_json::Number::from(0)),
            ),
        );
        stats.insert(
            "evictions".to_string(),
            serde_json::Value::Number(metrics.evictions.into()),
        );
        stats.insert(
            "expired_removals".to_string(),
            serde_json::Value::Number(metrics.expired_removals.into()),
        );

        stats
    }

    // Métodos privados

    fn evict_one(&self) {
        let key_to_evict = match self.policy {
            EvictionPolicy::LRU => self.find_lru_key(),
            EvictionPolicy::LFU => self.find_lfu_key(),
            EvictionPolicy::FIFO => self.find_fifo_key(),
        };

        if let Some(key) = key_to_evict {
            self.entries.remove(&key);
            self.remove_from_access_order(&key);

            let mut metrics = self.metrics.write();
            metrics.evictions += 1;

            debug!("Cache: Evicted item usando política {:?}", self.policy);
        }
    }

    fn find_lru_key(&self) -> Option<K> {
        self.access_order.read().front().cloned()
    }

    fn find_lfu_key(&self) -> Option<K> {
        self.entries
            .iter()
            .min_by_key(|entry| entry.value().access_count)
            .map(|entry| entry.key().clone())
    }

    fn find_fifo_key(&self) -> Option<K> {
        self.entries
            .iter()
            .min_by_key(|entry| entry.value().created_at)
            .map(|entry| entry.key().clone())
    }

    fn update_access_order(&self, key: K) {
        if self.policy == EvictionPolicy::LRU {
            let mut order = self.access_order.write();

            // Remover si ya existe
            order.retain(|k| k != &key);

            // Agregar al final (más recientemente usado)
            order.push_back(key);
        }
    }

    fn remove_from_access_order(&self, key: &K) {
        if self.policy == EvictionPolicy::LRU {
            let mut order = self.access_order.write();
            order.retain(|k| k != key);
        }
    }
}

impl<K, V> Default for LRUCache<K, V>
where
    K: Clone + Eq + Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new(100)
    }
}

/// Obtiene timestamp actual en segundos
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
