use anyhow::Result;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, warn, error};

use super::{TrackSource, SourceType, youtube::YouTubeClient};

/// Optimizador de búsqueda musical con múltiples estrategias
pub struct SearchOptimizer {
    pub youtube_client: YouTubeClient,
    pub search_cache: HashMap<String, Vec<TrackSource>>,
    pub failed_queries: HashMap<String, u8>, // Query -> fail count
}

impl SearchOptimizer {
    pub fn new() -> Self {
        Self {
            youtube_client: YouTubeClient::new(),
            search_cache: HashMap::new(),
            failed_queries: HashMap::new(),
        }
    }

    /// Búsqueda inteligente con múltiples estrategias y fallbacks
    pub async fn smart_search(&mut self, query: &str, max_results: usize) -> Result<Vec<TrackSource>> {
        debug!("🔍 Iniciando búsqueda inteligente para: '{}'", query);

        // 1. Verificar caché primero
        if let Some(cached_results) = self.search_cache.get(query) {
            debug!("✅ Resultados encontrados en caché para: '{}'", query);
            return Ok(cached_results.clone());
        }

        // 2. Verificar si esta query ha fallado mucho
        if let Some(fail_count) = self.failed_queries.get(query) {
            if *fail_count >= 3 {
                debug!("⚠️ Query '{}' ha fallado {} veces, aplicando corrección", query, fail_count);
                return self.search_with_corrections(query, max_results).await;
            }
        }

        // 3. Búsqueda normal con timeout
        match self.search_with_strategies(query, max_results).await {
            Ok(results) if !results.is_empty() => {
                // Cachear resultados exitosos
                self.search_cache.insert(query.to_string(), results.clone());
                self.failed_queries.remove(query); // Limpiar fallos anteriores
                Ok(results)
            }
            Ok(_) => {
                // Búsqueda exitosa pero sin resultados
                warn!("🔍 Búsqueda sin resultados para: '{}'", query);
                self.increment_failure(query);
                self.search_with_corrections(query, max_results).await
            }
            Err(e) => {
                error!("❌ Error en búsqueda para '{}': {}", query, e);
                self.increment_failure(query);
                self.search_with_corrections(query, max_results).await
            }
        }
    }

    /// Búsqueda con múltiples estrategias secuenciales
    async fn search_with_strategies(&self, query: &str, max_results: usize) -> Result<Vec<TrackSource>> {
        // Estrategia 1: Búsqueda directa con timeout de 10 segundos
        if let Ok(results) = timeout(Duration::from_secs(10), self.direct_search(query, max_results)).await {
            match results {
                Ok(tracks) if !tracks.is_empty() => {
                    debug!("✅ Estrategia directa exitosa para: '{}'", query);
                    return Ok(tracks);
                }
                _ => debug!("⚠️ Estrategia directa sin resultados para: '{}'", query),
            }
        } else {
            warn!("⏰ Timeout en búsqueda directa para: '{}'", query);
        }

        // Estrategia 2: Búsqueda con términos mejorados
        if let Ok(results) = timeout(Duration::from_secs(8), self.enhanced_search(query, max_results)).await {
            match results {
                Ok(tracks) if !tracks.is_empty() => {
                    debug!("✅ Estrategia mejorada exitosa para: '{}'", query);
                    return Ok(tracks);
                }
                _ => debug!("⚠️ Estrategia mejorada sin resultados para: '{}'", query),
            }
        }

        // Estrategia 3: Búsqueda simplificada
        if let Ok(results) = timeout(Duration::from_secs(6), self.simplified_search(query, max_results)).await {
            match results {
                Ok(tracks) if !tracks.is_empty() => {
                    debug!("✅ Estrategia simplificada exitosa para: '{}'", query);
                    return Ok(tracks);
                }
                _ => debug!("⚠️ Estrategia simplificada sin resultados para: '{}'", query),
            }
        }

        Err(anyhow::anyhow!("Todas las estrategias de búsqueda fallaron"))
    }

    /// Búsqueda directa sin modificaciones
    async fn direct_search(&self, query: &str, max_results: usize) -> Result<Vec<TrackSource>> {
        let results = self.youtube_client.search_detailed(query, max_results * 2).await?;
        let filtered = self.youtube_client.filter_results(results, query);
        Ok(self.convert_to_track_sources(filtered, max_results))
    }

    /// Búsqueda con términos mejorados (agregar "lyrics", "official", etc.)
    async fn enhanced_search(&self, query: &str, max_results: usize) -> Result<Vec<TrackSource>> {
        let enhanced_queries = self.generate_enhanced_queries(query);
        
        for enhanced_query in enhanced_queries {
            debug!("🔍 Probando query mejorada: '{}'", enhanced_query);
            
            if let Ok(results) = self.youtube_client.search_detailed(&enhanced_query, max_results).await {
                let filtered = self.youtube_client.filter_results(results, query);
                if !filtered.is_empty() {
                    return Ok(self.convert_to_track_sources(filtered, max_results));
                }
            }
        }

        Err(anyhow::anyhow!("Búsqueda mejorada sin resultados"))
    }

    /// Búsqueda simplificada (quitar caracteres especiales, normalizar)
    async fn simplified_search(&self, query: &str, max_results: usize) -> Result<Vec<TrackSource>> {
        let simplified = self.simplify_query(query);
        debug!("🔍 Query simplificada: '{}' -> '{}'", query, simplified);
        
        let results = self.youtube_client.search_detailed(&simplified, max_results).await?;
        let filtered = self.youtube_client.filter_results(results, &simplified);
        Ok(self.convert_to_track_sources(filtered, max_results))
    }

    /// Búsqueda con correcciones ortográficas y sugerencias
    async fn search_with_corrections(&mut self, query: &str, max_results: usize) -> Result<Vec<TrackSource>> {
        debug!("🔧 Aplicando correcciones para: '{}'", query);
        
        let corrections = self.generate_corrections(query);
        
        for correction in corrections {
            debug!("🔍 Probando corrección: '{}'", correction);
            
            match self.search_with_strategies(&correction, max_results).await {
                Ok(results) if !results.is_empty() => {
                    debug!("✅ Corrección exitosa: '{}' -> '{}'", query, correction);
                    // Cachear bajo la query original
                    self.search_cache.insert(query.to_string(), results.clone());
                    return Ok(results);
                }
                _ => continue,
            }
        }

        // Si nada funciona, devolver error descriptivo
        Err(anyhow::anyhow!(
            "No se encontraron resultados para '{}' después de intentar múltiples estrategias y correcciones", 
            query
        ))
    }

    /// Genera variaciones mejoradas de la query
    fn generate_enhanced_queries(&self, query: &str) -> Vec<String> {
        let mut queries = Vec::new();
        let base_query = query.to_lowercase();

        // Agregar términos que mejoran los resultados
        let enhancers = ["official", "lyrics", "audio", "music", "song"];
        
        for enhancer in enhancers {
            if !base_query.contains(enhancer) {
                queries.push(format!("{} {}", query, enhancer));
            }
        }

        // Si la query es muy corta, agregar "music"
        if query.len() < 10 {
            queries.push(format!("{} music", query));
        }

        queries
    }

    /// Simplifica la query removiendo caracteres especiales
    fn simplify_query(&self, query: &str) -> String {
        query
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
    }

    /// Genera correcciones ortográficas básicas
    fn generate_corrections(&self, query: &str) -> Vec<String> {
        let mut corrections = Vec::new();
        
        // Correcciones comunes en español
        let common_corrections = [
            ("cancion", "canción"),
            ("musica", "música"),
            ("ñ", "n"),
            ("á", "a"), ("é", "e"), ("í", "i"), ("ó", "o"), ("ú", "u"),
            ("ü", "u"),
        ];

        let mut corrected = query.to_lowercase();
        for (wrong, correct) in common_corrections {
            if corrected.contains(wrong) {
                corrections.push(corrected.replace(wrong, correct));
            }
        }

        // Remover duplicados y queries vacías
        corrections.sort();
        corrections.dedup();
        corrections.retain(|s| !s.trim().is_empty() && s != query);

        // Si no hay correcciones, intentar dividir la query
        if corrections.is_empty() {
            let words: Vec<&str> = query.split_whitespace().collect();
            if words.len() > 2 {
                // Usar solo las primeras 2 palabras
                corrections.push(words[..2].join(" "));
                // Usar solo la primera palabra si es significativa
                if words[0].len() > 3 {
                    corrections.push(words[0].to_string());
                }
            }
        }

        corrections
    }

    /// Convierte metadata a TrackSource
    fn convert_to_track_sources(&self, metadata_list: Vec<crate::sources::youtube::TrackMetadata>, max_results: usize) -> Vec<TrackSource> {
        metadata_list
            .into_iter()
            .take(max_results)
            .map(|meta| {
                let mut track = TrackSource::new(
                    meta.title,
                    meta.url.unwrap_or_default(),
                    SourceType::YouTube,
                    serenity::model::id::UserId::new(0), // Será sobreescrito
                );

                if let Some(artist) = meta.artist {
                    track = track.with_artist(artist);
                }

                if let Some(duration) = meta.duration {
                    track = track.with_duration(duration);
                }

                if let Some(thumbnail) = meta.thumbnail {
                    track = track.with_thumbnail(thumbnail);
                }

                track
            })
            .collect()
    }

    /// Incrementa contador de fallos para una query
    fn increment_failure(&mut self, query: &str) {
        let count = self.failed_queries.entry(query.to_string()).or_insert(0);
        *count += 1;
        debug!("⚠️ Query '{}' ha fallado {} veces", query, count);
    }

    /// Limpia caché viejo y estadísticas de fallos
    pub fn cleanup_cache(&mut self) {
        if self.search_cache.len() > 1000 {
            debug!("🧹 Limpiando caché de búsqueda");
            self.search_cache.clear();
        }

        if self.failed_queries.len() > 500 {
            debug!("🧹 Limpiando estadísticas de fallos");
            self.failed_queries.clear();
        }
    }

    /// Obtiene estadísticas de rendimiento
    pub fn get_stats(&self) -> SearchStats {
        SearchStats {
            cached_queries: self.search_cache.len(),
            failed_queries: self.failed_queries.len(),
            total_failures: self.failed_queries.values().sum(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchStats {
    pub cached_queries: usize,
    pub failed_queries: usize,
    pub total_failures: u32,
}

impl Default for SearchOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait para búsqueda optimizada que pueden implementar diferentes fuentes
pub trait OptimizedSearch {
    async fn optimized_search(&mut self, query: &str, max_results: usize) -> Result<Vec<TrackSource>>;
    fn get_search_stats(&self) -> SearchStats;
    fn cleanup(&mut self);
}

impl OptimizedSearch for SearchOptimizer {
    async fn optimized_search(&mut self, query: &str, max_results: usize) -> Result<Vec<TrackSource>> {
        self.smart_search(query, max_results).await
    }

    fn get_search_stats(&self) -> SearchStats {
        self.get_stats()
    }

    fn cleanup(&mut self) {
        self.cleanup_cache()
    }
}