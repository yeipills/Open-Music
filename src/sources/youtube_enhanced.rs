use anyhow::Result;
use serde_json::Value;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, warn, error, info};
use std::collections::HashMap;

use super::youtube::{TrackMetadata, YouTubeClient};

/// Cliente YouTube mejorado con reintentos automáticos y manejo robusto de errores
pub struct EnhancedYouTubeClient {
    #[allow(dead_code)]
    base_client: YouTubeClient,
    #[allow(dead_code)]
    retry_config: RetryConfig,
    #[allow(dead_code)]
    error_cache: HashMap<String, ErrorInfo>,
    #[allow(dead_code)]
    format_preferences: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    #[allow(dead_code)]
    pub max_retries: u8,
    #[allow(dead_code)]
    pub base_delay: Duration,
    #[allow(dead_code)]
    pub max_delay: Duration,
    #[allow(dead_code)]
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
struct ErrorInfo {
    #[allow(dead_code)]
    count: u8,
    #[allow(dead_code)]
    last_error: String,
    #[allow(dead_code)]
    last_attempt: chrono::DateTime<chrono::Utc>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(10),
            timeout: Duration::from_secs(30),
        }
    }
}

impl EnhancedYouTubeClient {
    pub fn new() -> Self {
        Self {
            base_client: YouTubeClient::new(),
            retry_config: RetryConfig::default(),
            error_cache: HashMap::new(),
            format_preferences: vec![
                // Formatos de audio preferidos para mejor calidad y compatibilidad
                "bestaudio[ext=m4a]".to_string(),
                "bestaudio[ext=webm]".to_string(),
                "bestaudio[ext=mp3]".to_string(),
                "bestaudio".to_string(),
                // Fallbacks con video si no hay solo audio
                "best[height<=720]".to_string(),
                "best".to_string(),
            ],
        }
    }

    /// Búsqueda con reintentos automáticos y recuperación de errores
    #[allow(dead_code)]
    pub async fn search_with_retry(&mut self, query: &str, max_results: usize) -> Result<Vec<TrackMetadata>> {
        let start_time = std::time::Instant::now();
        
        for attempt in 1..=self.retry_config.max_retries {
            match self.attempt_search(query, max_results, attempt).await {
                Ok(results) => {
                    let duration = start_time.elapsed();
                    info!("✅ Búsqueda exitosa en intento {}: {} resultados en {:?}", 
                          attempt, results.len(), duration);
                    return Ok(results);
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    warn!("❌ Intento {} falló: {}", attempt, error_msg);
                    
                    if attempt < self.retry_config.max_retries {
                        let delay = self.calculate_delay(attempt);
                        warn!("⏳ Esperando {:?} antes del siguiente intento...", delay);
                        tokio::time::sleep(delay).await;
                        
                        // Cachear el error para evitar reintentos innecesarios
                        self.cache_error(query, &error_msg);
                    } else {
                        error!("❌ Todos los intentos fallaron para: {}", query);
                        return Err(e);
                    }
                }
            }
        }
        
        anyhow::bail!("Búsqueda falló después de {} intentos", self.retry_config.max_retries)
    }

    /// Intenta una búsqueda individual con timeout
    async fn attempt_search(&self, query: &str, max_results: usize, attempt: u8) -> Result<Vec<TrackMetadata>> {
        let timeout_duration = if attempt == 0 {
            self.retry_config.timeout
        } else {
            // Aumentar timeout en intentos posteriores
            self.retry_config.timeout + Duration::from_secs(attempt as u64 * 10)
        };

        debug!("🔍 Intento {} para '{}' con timeout de {:?}", attempt + 1, query, timeout_duration);

        match timeout(timeout_duration, self.execute_search(query, max_results)).await {
            Ok(result) => result,
            Err(_) => {
                warn!("⏰ Timeout en búsqueda para '{}' (intento {})", query, attempt + 1);
                Err(anyhow::anyhow!("Timeout en búsqueda después de {:?}", timeout_duration))
            }
        }
    }

    /// Ejecuta la búsqueda real usando yt-dlp
    async fn execute_search(&self, query: &str, max_results: usize) -> Result<Vec<TrackMetadata>> {
        let mut cmd = Command::new("yt-dlp");
        
        cmd.args([
            "--extract-flat",
            "--quiet",
            "--no-warnings",
            "--dump-json",
            &format!("ytsearch{}:{}", max_results, query),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

        debug!("🔧 Ejecutando comando yt-dlp para búsqueda: {:?}", cmd);

        let output = cmd.output().await?;

        if !output.status.success() {
            let error_output = String::from_utf8_lossy(&output.stderr);
            error!("❌ yt-dlp falló: {}", error_output);
            return Err(anyhow::anyhow!("yt-dlp error: {}", error_output));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_search_results(&stdout)
    }

    /// Parsea los resultados de yt-dlp en formato JSON
    fn parse_search_results(&self, output: &str) -> Result<Vec<TrackMetadata>> {
        let mut results = Vec::new();

        for line in output.lines() {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<Value>(line) {
                Ok(json) => {
                    if let Some(metadata) = self.extract_metadata_from_json(&json) {
                        results.push(metadata);
                    }
                }
                Err(e) => {
                    warn!("⚠️ Error parseando línea JSON: {} - Error: {}", line, e);
                    continue;
                }
            }
        }

        if results.is_empty() {
            Err(anyhow::anyhow!("No se encontraron resultados válidos"))
        } else {
            Ok(results)
        }
    }

    /// Extrae metadata de un objeto JSON de yt-dlp
    fn extract_metadata_from_json(&self, json: &Value) -> Option<TrackMetadata> {
        let title = json.get("title")?.as_str()?.to_string();
        let id = json.get("id")?.as_str()?;
        let url = format!("https://www.youtube.com/watch?v={}", id);

        // Extraer información adicional
        let artist = json.get("uploader").and_then(|v| v.as_str()).map(|s| s.to_string());
        
        let duration = json.get("duration")
            .and_then(|v| v.as_f64())
            .map(|d| Duration::from_secs(d as u64));

        let thumbnail = json.get("thumbnail")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Some(TrackMetadata {
            title,
            artist,
            duration,
            url: Some(url),
            thumbnail,
            source_type: super::SourceType::YouTube,
            is_live: false,
            stream_url: None,
        })
    }

    /// Obtiene URL de streaming con reintentos y múltiples formatos
    #[allow(dead_code)]
    pub async fn get_stream_url_with_retry(&mut self, video_url: &str) -> Result<String> {
        debug!("🔗 Obteniendo URL de stream para: {}", video_url);

        let mut attempt = 0;
        let mut last_error = None;

        while attempt <= self.retry_config.max_retries {
            for format_selector in &self.format_preferences.clone() {
                match self.attempt_get_stream_url(video_url, format_selector, attempt).await {
                    Ok(url) => {
                        info!("✅ URL de stream obtenida para {} con formato {}", video_url, format_selector);
                        return Ok(url);
                    }
                    Err(e) => {
                        debug!("⚠️ Formato {} falló para {}: {}", format_selector, video_url, e);
                        last_error = Some(e);
                    }
                }
            }

            attempt += 1;
            if attempt <= self.retry_config.max_retries {
                let delay = self.calculate_delay(attempt);
                warn!("⚠️ Todos los formatos fallaron (intento {}), reintentando en {:?}", attempt, delay);
                tokio::time::sleep(delay).await;
            }
        }

        if let Some(error) = last_error {
            error!("❌ No se pudo obtener URL de stream para {}: {}", video_url, error);
            Err(error)
        } else {
            Err(anyhow::anyhow!("No se pudo obtener URL de stream"))
        }
    }

    /// Intenta obtener URL de stream con un formato específico
    async fn attempt_get_stream_url(&self, video_url: &str, format: &str, attempt: u8) -> Result<String> {
        let timeout_duration = self.retry_config.timeout + Duration::from_secs(attempt as u64 * 5);

        let future = async {
            let mut cmd = Command::new("yt-dlp");
            
            cmd.args([
                "--get-url",
                "--format", format,
                "--no-warnings",
                "--quiet",
                video_url,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

            let output = cmd.output().await?;

            if !output.status.success() {
                let error_output = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow::anyhow!("yt-dlp error para formato {}: {}", format, error_output));
            }

            let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
            
            if url.is_empty() {
                return Err(anyhow::anyhow!("URL vacía para formato {}", format));
            }

            Ok(url)
        };

        timeout(timeout_duration, future).await
            .map_err(|_| anyhow::anyhow!("Timeout obteniendo stream URL"))?
    }

    /// Búsqueda alternativa cuando la principal falla
    #[allow(dead_code)]
    async fn alternative_search(&mut self, query: &str, max_results: usize) -> Result<Vec<TrackMetadata>> {
        info!("🔄 Iniciando búsqueda alternativa para: '{}'", query);

        // Estrategia 1: Búsqueda simplificada
        let simplified_query = self.simplify_query(query);
        if simplified_query != query {
            match self.execute_search(&simplified_query, max_results).await {
                Ok(results) if !results.is_empty() => {
                    info!("✅ Búsqueda alternativa exitosa con query simplificada");
                    return Ok(results);
                }
                _ => debug!("⚠️ Búsqueda simplificada sin resultados"),
            }
        }

        // Estrategia 2: Búsqueda por palabras clave
        let keywords = self.extract_keywords(query);
        for keyword_query in keywords {
            match self.execute_search(&keyword_query, max_results / 2).await {
                Ok(results) if !results.is_empty() => {
                    info!("✅ Búsqueda alternativa exitosa con keywords");
                    return Ok(results);
                }
                _ => continue,
            }
        }

        Err(anyhow::anyhow!("Todas las estrategias de búsqueda alternativa fallaron"))
    }

    /// Simplifica la query removiendo caracteres problemáticos
    #[allow(dead_code)]
    fn simplify_query(&self, query: &str) -> String {
        query
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
    }

    /// Extrae palabras clave de la query para búsquedas alternativas
    #[allow(dead_code)]
    fn extract_keywords(&self, query: &str) -> Vec<String> {
        let words: Vec<&str> = query.split_whitespace().collect();
        let mut keywords = Vec::new();

        if words.len() > 1 {
            // Usar combinaciones de 2 palabras
            for chunk in words.chunks(2) {
                keywords.push(chunk.join(" "));
            }
        }

        if words.len() > 2 {
            // Usar primera y última palabra
            keywords.push(format!("{} {}", words[0], words[words.len() - 1]));
        }

        keywords
    }

    /// Calcula el delay exponencial para reintentos
    fn calculate_delay(&self, attempt: u8) -> Duration {
        let delay = self.retry_config.base_delay * (2_u32.pow(attempt as u32 - 1));
        delay.min(self.retry_config.max_delay)
    }

    /// Cachea información de error para una query
    fn cache_error(&mut self, query: &str, error: &str) {
        let error_info = self.error_cache.entry(query.to_string()).or_insert(ErrorInfo {
            count: 0,
            last_error: String::new(),
            last_attempt: chrono::Utc::now(),
        });

        error_info.count += 1;
        error_info.last_error = error.to_string();
        error_info.last_attempt = chrono::Utc::now();

        debug!("📝 Error cacheado para '{}': {} (total: {})", query, error, error_info.count);
    }

    /// Limpia caché de errores antiguos
    #[allow(dead_code)]
    pub fn cleanup_error_cache(&mut self) {
        let cutoff = chrono::Utc::now() - chrono::Duration::minutes(30);
        
        let before_count = self.error_cache.len();
        self.error_cache.retain(|_, error_info| error_info.last_attempt > cutoff);
        let after_count = self.error_cache.len();

        if before_count != after_count {
            debug!("🧹 Limpieza de caché de errores: {} -> {} entradas", before_count, after_count);
        }
    }

    /// Actualiza configuración de reintentos
    #[allow(dead_code)]
    pub fn configure_retry(&mut self, max_retries: u8, base_delay_ms: u64, timeout_secs: u64) {
        self.retry_config = RetryConfig {
            max_retries,
            base_delay: Duration::from_millis(base_delay_ms),
            max_delay: Duration::from_secs(timeout_secs / 2),
            timeout: Duration::from_secs(timeout_secs),
        };
        
        info!("⚙️ Configuración de reintentos actualizada: max_retries={}, base_delay={}ms, timeout={}s", 
              max_retries, base_delay_ms, timeout_secs);
    }

    /// Obtiene estadísticas del cliente
    #[allow(dead_code)]
    pub fn get_stats(&self) -> ClientStats {
        ClientStats {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            cache_hits: 0,
            cache_misses: 0,
            average_response_time: Duration::from_millis(0),
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ClientStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub average_response_time: Duration,
}

impl Default for EnhancedYouTubeClient {
    fn default() -> Self {
        Self::new()
    }
}