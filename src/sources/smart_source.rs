use anyhow::Result;
use async_trait::async_trait;
use std::time::{Duration, Instant};
use tracing::{info, warn, error, debug};
use tokio::time::timeout;

use super::{
    MusicSource, TrackSource, SourceType,
    YouTubeAPIv3Client, InvidiousClient, YouTubeFastClient, 
    EnhancedYouTubeClient, YouTubeRssClient, DirectUrlClient
};

/// Estrategia de fallback jerárquica
#[derive(Debug, Clone)]
pub struct HierarchicalStrategy {
    sources: Vec<SourceConfig>,
    #[allow(dead_code)]
    timeout_per_source: Duration,
    #[allow(dead_code)]
    max_retries_per_source: u8,
}

#[derive(Debug, Clone)]
struct SourceConfig {
    name: &'static str,
    priority: u8, // 1 = más alta prioridad
    timeout: Duration,
    retries: u8,
    enabled: bool,
}

/// Cliente de música inteligente con fallback jerárquico
pub struct SmartMusicClient {
    strategy: HierarchicalStrategy,
    youtube_api: Option<YouTubeAPIv3Client>,
    invidious: InvidiousClient,
    youtube_fast: YouTubeFastClient,
    #[allow(dead_code)]
    youtube_enhanced: EnhancedYouTubeClient,
    youtube_rss: YouTubeRssClient,
    direct_url: DirectUrlClient,
}

/// Alias para compatibilidad
pub type SmartSource = SmartMusicClient;

impl SmartMusicClient {
    pub fn new() -> Self {
        let strategy = HierarchicalStrategy {
            sources: vec![
                // 1. yt-dlp directo (más confiable y rápido)
                SourceConfig {
                    name: "yt-dlp directo",
                    priority: 1,
                    timeout: Duration::from_secs(10),
                    retries: 1,
                    enabled: true,
                },
                // 2. YouTube API v3 (si está configurado)
                SourceConfig {
                    name: "YouTube API v3",
                    priority: 2,
                    timeout: Duration::from_secs(3),
                    retries: 1,
                    enabled: true,
                },
                // 3. Invidious (sin cookies, muy confiable)
                SourceConfig {
                    name: "Invidious",
                    priority: 3,
                    timeout: Duration::from_secs(5),
                    retries: 2,
                    enabled: true,
                },
                // 4. YouTube Fast (scraping optimizado)
                SourceConfig {
                    name: "YouTube Fast",
                    priority: 4,
                    timeout: Duration::from_secs(8),
                    retries: 1,
                    enabled: true,
                },
                // 5. YouTube RSS (último recurso)
                SourceConfig {
                    name: "YouTube RSS",
                    priority: 5,
                    timeout: Duration::from_secs(10),
                    retries: 1,
                    enabled: true,
                },
            ],
            timeout_per_source: Duration::from_secs(20),
            max_retries_per_source: 3,
        };

        Self {
            strategy,
            youtube_api: Self::create_youtube_api_client(),
            invidious: InvidiousClient::new(),
            youtube_fast: YouTubeFastClient::new(),
            youtube_enhanced: EnhancedYouTubeClient::new(),
            youtube_rss: YouTubeRssClient::new(),
            direct_url: DirectUrlClient::new(),
        }
    }

    /// Búsqueda jerárquica inteligente
    pub async fn search_hierarchical(&self, query: &str, limit: usize) -> Result<Vec<TrackSource>> {
        let start_time = Instant::now();
        info!("🎯 Iniciando búsqueda jerárquica para: '{}'", query);

        // Verificar disponibilidad de yt-dlp primero
        if self.is_ytdlp_available().await {
            info!("🔧 yt-dlp disponible, intentando búsqueda directa...");
            match self.try_ytdlp_search(query, limit).await {
                Ok(results) if !results.is_empty() => {
                    let elapsed = start_time.elapsed();
                    info!("✅ Éxito con yt-dlp directo: {} resultados en {:?}", results.len(), elapsed);
                    return Ok(results);
                }
                Ok(_) => {
                    warn!("⚠️ yt-dlp devolvió 0 resultados");
                }
                Err(e) => {
                    warn!("❌ yt-dlp falló: {}", e);
                }
            }
        } else {
            info!("⚠️ yt-dlp no disponible, saltando a otras fuentes");
        }

        // Ordenar fuentes por prioridad (excluyendo yt-dlp directo)
        let mut sorted_sources = self.strategy.sources.clone();
        sorted_sources.retain(|s| s.name != "yt-dlp directo");
        sorted_sources.sort_by_key(|s| s.priority);

        for source_config in sorted_sources {
            if !source_config.enabled {
                continue;
            }

            info!("🔍 Intentando fuente: {} (prioridad {})", source_config.name, source_config.priority);

            match self.try_source(&source_config, query, limit).await {
                Ok(results) if !results.is_empty() => {
                    let elapsed = start_time.elapsed();
                    info!("✅ Éxito en {}: {} resultados en {:?}", source_config.name, results.len(), elapsed);
                    return Ok(results);
                }
                Ok(_) => {
                    warn!("⚠️ {} devolvió 0 resultados", source_config.name);
                }
                Err(e) => {
                    warn!("❌ {} falló: {}", source_config.name, e);
                }
            }
        }

        let total_elapsed = start_time.elapsed();
        error!("❌ Todas las fuentes fallaron después de {:?}", total_elapsed);
        Err(anyhow::anyhow!("No se encontraron resultados en ninguna fuente"))
    }

    /// Verifica si yt-dlp está disponible
    async fn is_ytdlp_available(&self) -> bool {
        use std::process::Command;
        
        match Command::new("yt-dlp").arg("--version").output() {
            Ok(output) => {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    info!("✅ yt-dlp disponible: {}", version);
                    true
                } else {
                    warn!("❌ yt-dlp no disponible (status: {})", output.status);
                    false
                }
            }
            Err(e) => {
                warn!("❌ yt-dlp no encontrado: {}", e);
                false
            }
        }
    }

    /// Intenta búsqueda directa con yt-dlp
    async fn try_ytdlp_search(&self, query: &str, limit: usize) -> Result<Vec<TrackSource>> {
        use tokio::process::Command as TokioCommand;
        
        // Usar yt-dlp para buscar videos
        let output = TokioCommand::new("yt-dlp")
            .args(&[
                "ytsearch3", // Buscar 3 videos
                &format!("{}", query),
                "--print", "id,title,uploader,duration,thumbnail",
                "--no-playlist",
                "--no-warnings",
                "--quiet"
            ])
            .output()
            .await?;

        if !output.status.success() {
            anyhow::bail!("yt-dlp falló con status: {}", output.status);
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = output_str.lines().collect();
        
        let mut tracks = Vec::new();
        let mut i = 0;
        
        while i < lines.len() && tracks.len() < limit {
            if let Some(video_id) = lines.get(i) {
                if let Some(title) = lines.get(i + 1) {
                    if let Some(uploader) = lines.get(i + 2) {
                        let youtube_url = format!("https://www.youtube.com/watch?v={}", video_id.trim());
                        
                        let mut track = TrackSource::new(
                            title.trim().to_string(),
                            youtube_url,
                            SourceType::YouTube,
                            serenity::model::id::UserId::default(),
                        );

                        // Configurar artista si está disponible
                        if !uploader.trim().is_empty() {
                            track = track.with_artist(uploader.trim().to_string());
                        }

                        // Configurar duración si está disponible
                        if let Some(duration_str) = lines.get(i + 3) {
                            if let Ok(duration_secs) = duration_str.trim().parse::<u64>() {
                                track = track.with_duration(Duration::from_secs(duration_secs));
                            }
                        }

                        // Configurar thumbnail si está disponible
                        if let Some(thumbnail) = lines.get(i + 4) {
                            if !thumbnail.trim().is_empty() {
                                track = track.with_thumbnail(thumbnail.trim().to_string());
                            }
                        }

                        tracks.push(track);
                    }
                }
            }
            i += 5; // Saltar al siguiente video
        }

        Ok(tracks)
    }

    /// Intenta una fuente específica con timeout y reintentos
    async fn try_source(&self, config: &SourceConfig, query: &str, limit: usize) -> Result<Vec<TrackSource>> {
        let mut last_error = None;

        for attempt in 0..config.retries {
            match timeout(config.timeout, self.execute_source(config, query, limit)).await {
                Ok(result) => return result,
                Err(_) => {
                    warn!("⏰ Timeout en {} (intento {})", config.name, attempt + 1);
                    last_error = Some(anyhow::anyhow!("Timeout después de {:?}", config.timeout));
                }
            }

            if attempt < config.retries - 1 {
                let delay = Duration::from_millis(500 * (attempt + 1) as u64);
                debug!("⏳ Esperando {:?} antes del siguiente intento", delay);
                tokio::time::sleep(delay).await;
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Fuente falló sin error específico")))
    }

    /// Ejecuta la búsqueda en una fuente específica
    async fn execute_source(&self, config: &SourceConfig, query: &str, limit: usize) -> Result<Vec<TrackSource>> {
        match config.name {
            "yt-dlp directo" => {
                self.try_ytdlp_search(query, limit).await
            }
            "YouTube API v3" => {
                if let Some(api_client) = &self.youtube_api {
                    api_client.search(query, limit).await
                } else {
                    Err(anyhow::anyhow!("YouTube API v3 no está configurado"))
                }
            }
            "Invidious" => {
                self.invidious.search(query, limit).await
            }
            "YouTube Fast" => {
                self.youtube_fast.search(query, limit).await
            }
            "YouTube RSS" => {
                self.youtube_rss.search(query, limit).await
            }
            _ => Err(anyhow::anyhow!("Fuente desconocida: {}", config.name))
        }
    }

    /// Crea cliente de YouTube API v3 si está configurado
    fn create_youtube_api_client() -> Option<YouTubeAPIv3Client> {
        if let Ok(api_key) = std::env::var("YOUTUBE_API_KEY") {
            if !api_key.is_empty() {
                info!("🔑 YouTube API v3 configurado");
                Some(YouTubeAPIv3Client::new(api_key))
            } else {
                warn!("⚠️ YOUTUBE_API_KEY está vacío");
                None
            }
        } else {
            info!("ℹ️ YOUTUBE_API_KEY no configurado, saltando YouTube API v3");
            None
        }
    }

    /// Obtiene estadísticas de rendimiento
    #[allow(dead_code)]
    pub fn get_performance_stats(&self) -> PerformanceStats {
        PerformanceStats {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            average_response_time: Duration::from_millis(0),
        }
    }

    /// Habilita o deshabilita una fuente específica
    #[allow(dead_code)]
    pub fn set_source_enabled(&mut self, source_name: &str, enabled: bool) {
        if let Some(source) = self.strategy.sources.iter_mut().find(|s| s.name == source_name) {
            source.enabled = enabled;
            info!("{} {}: {}", if enabled { "✅" } else { "❌" }, source_name, if enabled { "habilitado" } else { "deshabilitado" });
        }
    }

    /// Configura el timeout para una fuente específica
    #[allow(dead_code)]
    pub fn set_source_timeout(&mut self, source_name: &str, timeout: Duration) {
        if let Some(source) = self.strategy.sources.iter_mut().find(|s| s.name == source_name) {
            source.timeout = timeout;
            info!("⏱️ Timeout de {} configurado a {:?}", source_name, timeout);
        }
    }

    /// Configura el número de reintentos para una fuente específica
    #[allow(dead_code)]
    pub fn set_source_retries(&mut self, source_name: &str, retries: u8) {
        if let Some(source) = self.strategy.sources.iter_mut().find(|s| s.name == source_name) {
            source.retries = retries;
            info!("🔄 Reintentos de {} configurado a {}", source_name, retries);
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PerformanceStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_response_time: Duration,
}

// Implementación del trait MusicSource
#[async_trait]
impl MusicSource for SmartMusicClient {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<TrackSource>> {
        self.search_hierarchical(query, limit).await
    }

    async fn get_track(&self, url: &str) -> Result<TrackSource> {
        // Determinar la mejor fuente para la URL
        if self.youtube_api.is_some() && Self::is_youtube_url(url) {
            // Intentar YouTube API v3 primero
            if let Ok(track) = self.youtube_api.as_ref().unwrap().get_track(url).await {
                return Ok(track);
            }
        }

        // Fallback a Invidious para URLs de YouTube
        if Self::is_youtube_url(url) {
            return self.invidious.get_track(url).await;
        }

        // Para URLs directas
        self.direct_url.get_track(url).await
    }

    async fn get_playlist(&self, url: &str) -> Result<Vec<TrackSource>> {
        // Implementar lógica similar para playlists
        if Self::is_youtube_url(url) {
            if let Some(api_client) = &self.youtube_api {
                if let Ok(playlist) = api_client.get_playlist(url).await {
                    return Ok(playlist);
                }
            }
            return self.invidious.get_playlist(url).await;
        }

        Err(anyhow::anyhow!("Playlists no soportadas para esta URL"))
    }

    fn is_valid_url(&self, url: &str) -> bool {
        Self::is_youtube_url(url) || self.direct_url.is_valid_url(url)
    }

    fn source_name(&self) -> &'static str {
        "Smart Music Client"
    }
}

impl SmartMusicClient {
    fn is_youtube_url(url: &str) -> bool {
        url.contains("youtube.com") || url.contains("youtu.be")
    }
}

impl Default for SmartMusicClient {
    fn default() -> Self {
        Self::new()
    }
} 