use anyhow::Result;
use dashmap::DashMap;
use std::{
    sync::{Arc, atomic::{AtomicU64, AtomicUsize, Ordering}},
    time::{Duration, Instant},
    collections::HashMap,
};
use tokio::{sync::RwLock, time::interval};
use tracing::{debug, info, warn};
use sysinfo::{System, SystemExt};

/// Caché optimizado para audio con gestión inteligente de memoria
#[derive(Debug)]
pub struct OptimizedCache {
    // Caché principal para URLs de stream
    stream_cache: DashMap<String, CacheEntry<String>>,
    
    // Caché para metadata de tracks
    metadata_cache: DashMap<String, CacheEntry<TrackMetadata>>,
    
    // Caché para resultados de búsqueda
    search_cache: DashMap<String, CacheEntry<Vec<SearchResult>>>,
    
    // Estadísticas y configuración
    stats: Arc<CacheStats>,
    config: CacheConfig,
    
    // Monitor de memoria
    memory_monitor: Arc<RwLock<MemoryMonitor>>,
}

#[derive(Debug, Clone)]
struct CacheEntry<T> {
    data: T,
    created_at: Instant,
    last_accessed: AtomicInstant,
    access_count: AtomicU64,
    size_bytes: usize,
}

/// Wrapper para Instant que permite acceso atómico
#[derive(Debug)]
struct AtomicInstant {
    timestamp: AtomicU64,
}

impl AtomicInstant {
    fn new(instant: Instant) -> Self {
        Self {
            timestamp: AtomicU64::new(instant.elapsed().as_nanos() as u64),
        }
    }
    
    fn load(&self) -> Instant {
        let nanos = self.timestamp.load(Ordering::Relaxed);
        Instant::now() - Duration::from_nanos(nanos)
    }
    
    fn store(&self, instant: Instant) {
        let nanos = instant.elapsed().as_nanos() as u64;
        self.timestamp.store(nanos, Ordering::Relaxed);
    }
    
    fn update_to_now(&self) {
        self.store(Instant::now());
    }
}

impl Clone for AtomicInstant {
    fn clone(&self) -> Self {
        Self::new(self.load())
    }
}

#[derive(Debug, Clone)]
pub struct TrackMetadata {
    pub title: String,
    pub artist: Option<String>,
    pub duration: Option<Duration>,
    pub thumbnail: Option<String>,
    pub quality: AudioQuality,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub artist: Option<String>,
    pub duration: Option<Duration>,
}

#[derive(Debug, Clone)]
pub enum AudioQuality {
    Low,      // 96kbps
    Medium,   // 128kbps
    High,     // 192kbps
    VeryHigh, // 320kbps
}

#[derive(Debug)]
pub struct CacheStats {
    hits: AtomicU64,
    misses: AtomicU64,
    evictions: AtomicU64,
    memory_usage: AtomicUsize,
    peak_memory: AtomicUsize,
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub max_memory_mb: usize,
    pub max_stream_entries: usize,
    pub max_metadata_entries: usize,
    pub max_search_entries: usize,
    pub stream_ttl: Duration,
    pub metadata_ttl: Duration,
    pub search_ttl: Duration,
    pub cleanup_interval: Duration,
    pub memory_pressure_threshold: f32,
}

#[derive(Debug)]
struct MemoryMonitor {
    system: System,
    last_check: Instant,
    current_usage_mb: usize,
    available_mb: usize,
    pressure_level: MemoryPressure,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum MemoryPressure {
    Low,      // < 70% usage
    Medium,   // 70-85% usage
    High,     // 85-95% usage
    Critical, // > 95% usage
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_memory_mb: 256,           // 256MB por defecto
            max_stream_entries: 1000,
            max_metadata_entries: 5000,
            max_search_entries: 500,
            stream_ttl: Duration::from_secs(3600),      // 1 hora
            metadata_ttl: Duration::from_secs(7200),    // 2 horas
            search_ttl: Duration::from_secs(1800),      // 30 minutos
            cleanup_interval: Duration::from_secs(300), // 5 minutos
            memory_pressure_threshold: 0.8,             // 80%
        }
    }
}

impl OptimizedCache {
    pub fn new(config: CacheConfig) -> Self {
        let cache = Self {
            stream_cache: DashMap::new(),
            metadata_cache: DashMap::new(),
            search_cache: DashMap::new(),
            stats: Arc::new(CacheStats {
                hits: AtomicU64::new(0),
                misses: AtomicU64::new(0),
                evictions: AtomicU64::new(0),
                memory_usage: AtomicUsize::new(0),
                peak_memory: AtomicUsize::new(0),
            }),
            config: config.clone(),
            memory_monitor: Arc::new(RwLock::new(MemoryMonitor::new())),
        };

        // Iniciar tarea de limpieza automática
        cache.start_cleanup_task();
        
        info!("🗄️ Caché optimizado iniciado con límite de {}MB", config.max_memory_mb);
        cache
    }

    /// Obtiene URL de stream del caché
    pub async fn get_stream_url(&self, video_url: &str) -> Option<String> {
        if let Some(entry) = self.stream_cache.get(video_url) {
            if !self.is_expired(&entry, self.config.stream_ttl) {
                entry.last_accessed.store(Instant::now(), Ordering::Relaxed);
                entry.access_count.fetch_add(1, Ordering::Relaxed);
                self.stats.hits.fetch_add(1, Ordering::Relaxed);
                debug!("✅ Cache hit para stream URL: {}", video_url);
                return Some(entry.data.clone());
            } else {
                // Entrada expirada, remover
                self.stream_cache.remove(video_url);
                debug!("⏰ Entrada de stream expirada removida: {}", video_url);
            }
        }

        self.stats.misses.fetch_add(1, Ordering::Relaxed);
        debug!("❌ Cache miss para stream URL: {}", video_url);
        None
    }

    /// Almacena URL de stream en el caché
    pub async fn put_stream_url(&self, video_url: String, stream_url: String) -> Result<()> {
        let size = self.estimate_string_size(&video_url) + self.estimate_string_size(&stream_url);
        
        // Verificar si necesitamos liberar memoria
        if await self.should_evict_for_size(size) {
            self.evict_lru_streams(5).await;
        }

        let entry = CacheEntry {
            data: stream_url,
            created_at: Instant::now(),
            last_accessed: AtomicInstant::new(Instant::now()),
            access_count: AtomicU64::new(1),
            size_bytes: size,
        };

        self.stream_cache.insert(video_url.clone(), entry);
        self.update_memory_usage(size as i64);
        
        // Verificar límites
        if self.stream_cache.len() > self.config.max_stream_entries {
            self.evict_lru_streams(1).await;
        }

        debug!("💾 Stream URL almacenada en caché: {}", video_url);
        Ok(())
    }

    /// Obtiene metadata del caché
    pub async fn get_metadata(&self, track_id: &str) -> Option<TrackMetadata> {
        if let Some(entry) = self.metadata_cache.get(track_id) {
            if !self.is_expired(&entry, self.config.metadata_ttl) {
                entry.access_count.fetch_add(1, Ordering::Relaxed);
                self.stats.hits.fetch_add(1, Ordering::Relaxed);
                return Some(entry.data.clone());
            } else {
                self.metadata_cache.remove(track_id);
            }
        }

        self.stats.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Almacena metadata en el caché
    pub async fn put_metadata(&self, track_id: String, metadata: TrackMetadata) -> Result<()> {
        let size = self.estimate_metadata_size(&metadata);
        
        if await self.should_evict_for_size(size) {
            self.evict_lru_metadata(5).await;
        }

        let entry = CacheEntry {
            data: metadata,
            created_at: Instant::now(),
            last_accessed: AtomicInstant::new(Instant::now()),
            access_count: AtomicU64::new(1),
            size_bytes: size,
        };

        self.metadata_cache.insert(track_id, entry);
        self.update_memory_usage(size as i64);

        if self.metadata_cache.len() > self.config.max_metadata_entries {
            self.evict_lru_metadata(1).await;
        }

        Ok(())
    }

    /// Obtiene resultados de búsqueda del caché
    pub async fn get_search_results(&self, query: &str) -> Option<Vec<SearchResult>> {
        let cache_key = self.normalize_search_query(query);
        
        if let Some(entry) = self.search_cache.get(&cache_key) {
            if !self.is_expired(&entry, self.config.search_ttl) {
                entry.access_count.fetch_add(1, Ordering::Relaxed);
                self.stats.hits.fetch_add(1, Ordering::Relaxed);
                return Some(entry.data.clone());
            } else {
                self.search_cache.remove(&cache_key);
            }
        }

        self.stats.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Almacena resultados de búsqueda en el caché
    pub async fn put_search_results(&self, query: String, results: Vec<SearchResult>) -> Result<()> {
        let cache_key = self.normalize_search_query(&query);
        let size = self.estimate_search_results_size(&results);
        
        if await self.should_evict_for_size(size) {
            self.evict_lru_search(3).await;
        }

        let entry = CacheEntry {
            data: results,
            created_at: Instant::now(),
            last_accessed: AtomicInstant::new(Instant::now()),
            access_count: AtomicU64::new(1),
            size_bytes: size,
        };

        self.search_cache.insert(cache_key, entry);
        self.update_memory_usage(size as i64);

        if self.search_cache.len() > self.config.max_search_entries {
            self.evict_lru_search(1).await;
        }

        Ok(())
    }

    /// Limpieza manual del caché
    pub async fn cleanup(&self) {
        let before_streams = self.stream_cache.len();
        let before_metadata = self.metadata_cache.len();
        let before_search = self.search_cache.len();

        self.cleanup_expired_entries().await;
        
        let after_streams = self.stream_cache.len();
        let after_metadata = self.metadata_cache.len();
        let after_search = self.search_cache.len();

        if before_streams + before_metadata + before_search != after_streams + after_metadata + after_search {
            info!("🧹 Limpieza completada: streams {}->{}, metadata {}->{}, search {}->{}",
                  before_streams, after_streams, before_metadata, after_metadata, before_search, after_search);
        }
    }

    /// Obtiene estadísticas del caché
    pub async fn get_stats(&self) -> CacheStatistics {
        let monitor = self.memory_monitor.read().await;
        
        CacheStatistics {
            hits: self.stats.hits.load(Ordering::Relaxed),
            misses: self.stats.misses.load(Ordering::Relaxed),
            evictions: self.stats.evictions.load(Ordering::Relaxed),
            memory_usage_mb: self.stats.memory_usage.load(Ordering::Relaxed) / (1024 * 1024),
            peak_memory_mb: self.stats.peak_memory.load(Ordering::Relaxed) / (1024 * 1024),
            stream_entries: self.stream_cache.len(),
            metadata_entries: self.metadata_cache.len(),
            search_entries: self.search_cache.len(),
            memory_pressure: monitor.pressure_level,
            hit_ratio: self.calculate_hit_ratio(),
        }
    }

    /// Optimización basada en patrones de uso
    pub async fn optimize(&self) {
        let mut monitor = self.memory_monitor.write().await;
        monitor.update().await;

        match monitor.pressure_level {
            MemoryPressure::Critical => {
                warn!("🚨 Presión de memoria crítica, liberando 50% del caché");
                self.evict_percentage(50).await;
            }
            MemoryPressure::High => {
                warn!("⚠️ Presión de memoria alta, liberando 25% del caché");
                self.evict_percentage(25).await;
            }
            MemoryPressure::Medium => {
                info!("💡 Presión de memoria media, limpiando entradas menos usadas");
                self.evict_least_used(10).await;
            }
            MemoryPressure::Low => {
                debug!("✅ Presión de memoria baja, optimización no necesaria");
            }
        }
    }

    // Métodos privados

    fn start_cleanup_task(&self) {
        let cache_ref = Arc::new(self);
        let interval_duration = self.config.cleanup_interval;
        
        tokio::spawn(async move {
            let mut cleanup_interval = interval(interval_duration);
            
            loop {
                cleanup_interval.tick().await;
                
                if let Err(e) = cache_ref.cleanup().await {
                    warn!("Error en limpieza automática del caché: {}", e);
                }
                
                cache_ref.optimize().await;
            }
        });
    }

    async fn cleanup_expired_entries(&self) {
        let now = Instant::now();
        
        // Limpiar streams expirados
        self.stream_cache.retain(|_, entry| {
            !self.is_expired(entry, self.config.stream_ttl)
        });

        // Limpiar metadata expirada
        self.metadata_cache.retain(|_, entry| {
            !self.is_expired(entry, self.config.metadata_ttl)
        });

        // Limpiar búsquedas expiradas
        self.search_cache.retain(|_, entry| {
            !self.is_expired(entry, self.config.search_ttl)
        });
    }

    fn is_expired<T>(&self, entry: &CacheEntry<T>, ttl: Duration) -> bool {
        entry.created_at.elapsed() > ttl
    }

    async fn should_evict_for_size(&self, additional_size: usize) -> bool {
        let current_usage = self.stats.memory_usage.load(Ordering::Relaxed);
        let projected_usage = current_usage + additional_size;
        let limit = self.config.max_memory_mb * 1024 * 1024;
        
        projected_usage > limit
    }

    async fn evict_lru_streams(&self, count: usize) {
        let mut entries: Vec<_> = self.stream_cache.iter()
            .map(|entry| (entry.key().clone(), entry.last_accessed.load(Ordering::Relaxed)))
            .collect();
        
        entries.sort_by_key(|(_, last_accessed)| *last_accessed);
        
        for (key, _) in entries.into_iter().take(count) {
            if let Some((_, entry)) = self.stream_cache.remove(&key) {
                self.update_memory_usage(-(entry.size_bytes as i64));
                self.stats.evictions.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    async fn evict_lru_metadata(&self, count: usize) {
        let mut entries: Vec<_> = self.metadata_cache.iter()
            .map(|entry| (entry.key().clone(), entry.last_accessed.load(Ordering::Relaxed)))
            .collect();
        
        entries.sort_by_key(|(_, last_accessed)| *last_accessed);
        
        for (key, _) in entries.into_iter().take(count) {
            if let Some((_, entry)) = self.metadata_cache.remove(&key) {
                self.update_memory_usage(-(entry.size_bytes as i64));
                self.stats.evictions.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    async fn evict_lru_search(&self, count: usize) {
        let mut entries: Vec<_> = self.search_cache.iter()
            .map(|entry| (entry.key().clone(), entry.last_accessed.load(Ordering::Relaxed)))
            .collect();
        
        entries.sort_by_key(|(_, last_accessed)| *last_accessed);
        
        for (key, _) in entries.into_iter().take(count) {
            if let Some((_, entry)) = self.search_cache.remove(&key) {
                self.update_memory_usage(-(entry.size_bytes as i64));
                self.stats.evictions.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    async fn evict_percentage(&self, percentage: usize) {
        let stream_count = (self.stream_cache.len() * percentage) / 100;
        let metadata_count = (self.metadata_cache.len() * percentage) / 100;
        let search_count = (self.search_cache.len() * percentage) / 100;

        self.evict_lru_streams(stream_count).await;
        self.evict_lru_metadata(metadata_count).await;
        self.evict_lru_search(search_count).await;
    }

    async fn evict_least_used(&self, count: usize) {
        // Recopilar entradas de streams por conteo de acceso
        let mut stream_entries: Vec<_> = self.stream_cache.iter()
            .map(|entry| (entry.key().clone(), entry.access_count.load(Ordering::Relaxed)))
            .collect();
        stream_entries.sort_by_key(|(_, access_count)| *access_count);
        
        // Evictar los menos usados
        let stream_evict_count = (count * self.stream_cache.len()) / 
            (self.stream_cache.len() + self.metadata_cache.len() + self.search_cache.len()).max(1);
            
        for (key, _) in stream_entries.into_iter().take(stream_evict_count) {
            if let Some((_, entry)) = self.stream_cache.remove(&key) {
                self.update_memory_usage(-(entry.size_bytes as i64));
                self.stats.evictions.fetch_add(1, Ordering::Relaxed);
            }
        }
        
        // Hacer lo mismo para metadata y search
        let mut metadata_entries: Vec<_> = self.metadata_cache.iter()
            .map(|entry| (entry.key().clone(), entry.access_count.load(Ordering::Relaxed)))
            .collect();
        metadata_entries.sort_by_key(|(_, access_count)| *access_count);
        
        let metadata_evict_count = count.saturating_sub(stream_evict_count).min(metadata_entries.len());
        for (key, _) in metadata_entries.into_iter().take(metadata_evict_count) {
            if let Some((_, entry)) = self.metadata_cache.remove(&key) {
                self.update_memory_usage(-(entry.size_bytes as i64));
                self.stats.evictions.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn update_memory_usage(&self, delta: i64) {
        let current = self.stats.memory_usage.load(Ordering::Relaxed) as i64;
        let new_usage = (current + delta).max(0) as usize;
        
        self.stats.memory_usage.store(new_usage, Ordering::Relaxed);
        
        // Update peak if necessary
        let current_peak = self.stats.peak_memory.load(Ordering::Relaxed);
        if new_usage > current_peak {
            self.stats.peak_memory.store(new_usage, Ordering::Relaxed);
        }
    }

    fn calculate_hit_ratio(&self) -> f64 {
        let hits = self.stats.hits.load(Ordering::Relaxed) as f64;
        let misses = self.stats.misses.load(Ordering::Relaxed) as f64;
        let total = hits + misses;
        
        if total > 0.0 {
            hits / total
        } else {
            0.0
        }
    }

    fn normalize_search_query(&self, query: &str) -> String {
        query.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
    }

    fn estimate_string_size(&self, s: &str) -> usize {
        s.len() + std::mem::size_of::<String>()
    }

    fn estimate_metadata_size(&self, metadata: &TrackMetadata) -> usize {
        self.estimate_string_size(&metadata.title) +
        metadata.artist.as_ref().map_or(0, |s| self.estimate_string_size(s)) +
        metadata.thumbnail.as_ref().map_or(0, |s| self.estimate_string_size(s)) +
        std::mem::size_of::<TrackMetadata>()
    }

    fn estimate_search_results_size(&self, results: &[SearchResult]) -> usize {
        results.iter().map(|r| {
            self.estimate_string_size(&r.title) +
            self.estimate_string_size(&r.url) +
            r.artist.as_ref().map_or(0, |s| self.estimate_string_size(s)) +
            std::mem::size_of::<SearchResult>()
        }).sum()
    }
}

impl MemoryMonitor {
    fn new() -> Self {
        Self {
            system: System::new_all(),
            last_check: Instant::now(),
            current_usage_mb: 0,
            available_mb: 0,
            pressure_level: MemoryPressure::Low,
        }
    }

    async fn update(&mut self) {
        if self.last_check.elapsed() < Duration::from_secs(30) {
            return; // Update max every 30 seconds
        }

        self.system.refresh_memory();
        
        let total_memory = self.system.total_memory();
        let used_memory = self.system.used_memory();
        let available_memory = total_memory - used_memory;
        
        self.current_usage_mb = (used_memory / 1024 / 1024) as usize;
        self.available_mb = (available_memory / 1024 / 1024) as usize;
        
        let usage_ratio = used_memory as f32 / total_memory as f32;
        
        self.pressure_level = match usage_ratio {
            r if r > 0.95 => MemoryPressure::Critical,
            r if r > 0.85 => MemoryPressure::High,
            r if r > 0.70 => MemoryPressure::Medium,
            _ => MemoryPressure::Low,
        };
        
        self.last_check = Instant::now();
        
        // Log warning if memory pressure is high
        match self.pressure_level {
            MemoryPressure::Critical => {
                warn!("🚨 Memoria crítica: {}MB usados / {}MB disponibles ({:.1}%)", 
                      self.current_usage_mb, self.current_usage_mb + self.available_mb, usage_ratio * 100.0);
            }
            MemoryPressure::High => {
                warn!("⚠️ Memoria alta: {}MB usados / {}MB disponibles ({:.1}%)", 
                      self.current_usage_mb, self.current_usage_mb + self.available_mb, usage_ratio * 100.0);
            }
            MemoryPressure::Medium => {
                debug!("📊 Memoria media: {}MB usados / {}MB disponibles ({:.1}%)", 
                       self.current_usage_mb, self.current_usage_mb + self.available_mb, usage_ratio * 100.0);
            }
            MemoryPressure::Low => {
                debug!("✅ Memoria baja: {}MB usados / {}MB disponibles ({:.1}%)", 
                       self.current_usage_mb, self.current_usage_mb + self.available_mb, usage_ratio * 100.0);
            }
        }
    }
    
    fn get_pressure_level(&self) -> MemoryPressure {
        self.pressure_level
    }
    
    fn get_available_mb(&self) -> usize {
        self.available_mb
    }
    
    fn should_trigger_cleanup(&self) -> bool {
        matches!(self.pressure_level, MemoryPressure::High | MemoryPressure::Critical)
    }
}

#[derive(Debug, Clone)]
pub struct CacheStatistics {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub memory_usage_mb: usize,
    pub peak_memory_mb: usize,
    pub stream_entries: usize,
    pub metadata_entries: usize,
    pub search_entries: usize,
    pub memory_pressure: MemoryPressure,
    pub hit_ratio: f64,
}

