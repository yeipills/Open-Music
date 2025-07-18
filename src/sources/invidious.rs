use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;
use tracing::{info, warn};
use serenity::model::id::UserId;

use super::{MusicSource, SourceType, TrackSource};

/// Cliente para Invidious API (alternativa a YouTube API)
pub struct InvidiousClient {
    client: reqwest::Client,
    instances: Vec<String>,
    current_instance: std::sync::atomic::AtomicUsize,
}

#[derive(Debug, Deserialize)]
pub struct InvidiousVideo {
    #[serde(rename = "videoId")]
    pub video_id: String,
    pub title: String,
    pub description: String,
    pub lengthSeconds: u64,
    pub author: String,
    pub authorId: String,
    pub published: u64,
    pub viewCount: u64,
    pub likeCount: u64,
    pub subCountText: String,
    pub thumbnails: Vec<Thumbnail>,
    pub adaptiveFormats: Vec<AdaptiveFormat>,
    pub formatStreams: Vec<FormatStream>,
}

#[derive(Debug, Deserialize)]
pub struct Thumbnail {
    pub url: String,
    pub width: u32,
    #[allow(dead_code)]
    pub height: u32,
}

#[derive(Debug, Deserialize)]
pub struct AdaptiveFormat {
    pub url: String,
    #[serde(rename = "type")]
    pub format_type: String,
    #[allow(dead_code)]
    pub bitrate: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct FormatStream {
    pub url: String,
    #[serde(rename = "type")]
    pub format_type: String,
    #[allow(dead_code)]
    pub quality: String,
}

#[derive(Debug, Deserialize)]
struct InvidiousSearchResult {
    #[serde(rename = "videoId")]
    video_id: String,
    title: String,
    #[serde(rename = "lengthSeconds")]
    length_seconds: Option<u64>,
    author: Option<String>,
    #[serde(rename = "videoThumbnails")]
    video_thumbnails: Option<Vec<Thumbnail>>,
}

impl InvidiousClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .expect("Failed to create HTTP client");

        // Lista actualizada de instancias públicas de Invidious (2025) - Más confiables
        let instances = vec![
            "https://invidious.projectsegfau.lt".to_string(),
            "https://invidious.slipfox.xyz".to_string(),
            "https://invidious.privacydev.net".to_string(),
            "https://invidious.fdn.fr".to_string(),
            "https://invidious.weblibre.org".to_string(),
            "https://invidious.dhusch.de".to_string(),
            "https://invidious.protokolla.fi".to_string(),
            "https://vid.puffyan.us".to_string(),
            "https://inv.bp.projectsegfau.lt".to_string(),
            "https://yewtu.be".to_string(),
        ];

        Self {
            client,
            instances,
            current_instance: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Obtiene la siguiente instancia de Invidious
    fn get_next_instance(&self) -> String {
        let current = self.current_instance.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let index = current % self.instances.len();
        self.instances[index].clone()
    }

    /// Busca videos en Invidious con mejor manejo de errores
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<TrackSource>> {
        info!("🔍 Buscando en Invidious: {}", query);

        let mut last_error = String::new();
        let mut successful_results: Vec<TrackSource> = Vec::new();
        
        // Intentar con todas las instancias disponibles
        for (i, instance) in self.instances.iter().enumerate() {
            let url = format!("{}/api/v1/search", instance);
            
            match self.try_search(&url, query, limit).await {
                Ok(results) if !results.is_empty() => {
                    info!("✅ Búsqueda exitosa en {}: {} resultados", instance, results.len());
                    return Ok(results);
                }
                Ok(_) => {
                    warn!("⚠️ {} devolvió 0 resultados", instance);
                    last_error = format!("No results from {}", instance);
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    warn!("❌ Falló búsqueda en {}: {}", instance, error_msg);
                    last_error = format!("{}: {}", instance, error_msg);
                    
                    // Si es un error 429 (rate limit), esperar un poco
                    if error_msg.contains("429") {
                        info!("⏳ Rate limit detectado, esperando 2 segundos...");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                    
                    continue;
                }
            }
        }

        // Si todas las instancias fallan, intentar método alternativo
        info!("🔄 Todas las instancias de Invidious fallaron, intentando scraping directo...");
        match self.search_youtube_direct(query, limit).await {
            Ok(results) if !results.is_empty() => {
                info!("✅ Scraping directo exitoso: {} resultados", results.len());
                return Ok(results);
            }
            Ok(_) => {
                warn!("⚠️ Scraping directo devolvió 0 resultados");
            }
            Err(e) => {
                warn!("❌ Scraping directo falló: {}", e);
            }
        }

        anyhow::bail!("Falló búsqueda en todas las instancias de Invidious. Último error: {}", last_error)
    }

    async fn try_search(&self, url: &str, query: &str, limit: usize) -> Result<Vec<TrackSource>> {
        let response = self.client
            .get(url)
            .query(&[
                ("q", query),
                ("type", "video"),
                ("sort_by", "relevance"),
                ("page", "1"),
            ])
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .context("Error en request a Invidious")?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP error: {}", response.status());
        }

        let search_results: Vec<InvidiousSearchResult> = response
            .json()
            .await
            .context("Error parseando respuesta JSON")?;

        let mut tracks = Vec::new();
        for result in search_results.into_iter().take(limit) {
            // Validar que el resultado tenga los campos mínimos necesarios
            if result.video_id.is_empty() || result.title.is_empty() {
                continue;
            }

            let duration = result.length_seconds.map(Duration::from_secs);
            let thumbnail = result.video_thumbnails
                .and_then(|thumbs| thumbs.into_iter().find(|t| t.width >= 320))
                .map(|t| t.url);

            let youtube_url = format!("https://www.youtube.com/watch?v={}", result.video_id);
            
            let mut track = TrackSource::new(
                result.title,
                youtube_url,
                SourceType::YouTube,
                UserId::default(),
            );

            if let Some(author) = result.author {
                track = track.with_artist(author);
            }

            if let Some(duration) = duration {
                track = track.with_duration(duration);
            }

            if let Some(thumbnail) = thumbnail {
                track = track.with_thumbnail(thumbnail);
            }

            tracks.push(track);
        }

        Ok(tracks)
    }

    /// Obtiene información de un video específico
    pub async fn get_video_info(&self, video_id: &str) -> Result<InvidiousVideo> {
        for _attempt in 0..3 {
            let instance = self.get_next_instance();
            let url = format!("{}/api/v1/videos/{}", instance, video_id);

            match self.try_get_video_info(&url).await {
                Ok(video) => {
                    info!("✅ Información obtenida de {}", instance);
                    return Ok(video);
                }
                Err(e) => {
                    warn!("❌ Falló obtener info en {}: {}", instance, e);
                    continue;
                }
            }
        }

        anyhow::bail!("Falló obtener información en todas las instancias")
    }

    async fn try_get_video_info(&self, url: &str) -> Result<InvidiousVideo> {
        let response = self.client
            .get(url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .context("Error en request a Invidious")?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP error: {}", response.status());
        }

        let video: InvidiousVideo = response
            .json()
            .await
            .context("Error parseando información del video")?;

        Ok(video)
    }

    /// Obtiene la URL de audio directo del video
    pub async fn get_audio_url(&self, video_id: &str) -> Result<String> {
        let video_info = self.get_video_info(video_id).await?;

        // Buscar formato de audio
        for format in &video_info.adaptiveFormats {
            if format.format_type.contains("audio") && format.format_type.contains("mp4") {
                return Ok(format.url.clone());
            }
        }

        // Fallback a format streams
        for format in &video_info.formatStreams {
            if format.format_type.contains("audio") {
                return Ok(format.url.clone());
            }
        }

        anyhow::bail!("No se encontró formato de audio válido")
    }

    /// Extrae el video ID de una URL de YouTube
    pub fn extract_video_id(url: &str) -> Result<String> {
        use regex::Regex;
        
        let regex = Regex::new(r"(?:youtube\.com/watch\?v=|youtu\.be/|youtube\.com/embed/)([a-zA-Z0-9_-]{11})")
            .context("Error creando regex")?;

        if let Some(captures) = regex.captures(url) {
            if let Some(video_id) = captures.get(1) {
                return Ok(video_id.as_str().to_string());
            }
        }

        anyhow::bail!("No se pudo extraer video ID de la URL: {}", url)
    }

    /// Método de último recurso: scraping directo de YouTube
    async fn search_youtube_direct(&self, query: &str, limit: usize) -> Result<Vec<TrackSource>> {
        use regex::Regex;
        
        info!("🔍 Intentando scraping directo de YouTube para: {}", query);
        
        let search_url = format!(
            "https://www.youtube.com/results?search_query={}",
            urlencoding::encode(query)
        );

        let response = self.client
            .get(&search_url)
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .context("Error en request directo a YouTube")?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP error en YouTube directo: {}", response.status());
        }

        let html = response.text().await.context("Error leyendo HTML de YouTube")?;
        
        // Extraer video IDs y títulos del HTML
        let video_regex = Regex::new(r#""videoId":"([a-zA-Z0-9_-]{11})""#)?;
        let title_regex = Regex::new(r#""title":\{"runs":\[\{"text":"([^"]+)""#)?;
        
        let mut results = Vec::new();
        let mut video_ids = std::collections::HashSet::new();
        
        // Extraer video IDs únicos
        for cap in video_regex.captures_iter(&html) {
            if let Some(video_id_match) = cap.get(1) {
                let video_id = video_id_match.as_str();
                if video_ids.insert(video_id.to_string()) && video_ids.len() <= limit {
                    // Buscar título correspondiente (aproximado)
                    let title = if let Some(title_match) = title_regex.captures(&html) {
                        if let Some(title_text) = title_match.get(1) {
                            title_text.as_str().to_string()
                        } else {
                            format!("YouTube Video {}", video_id)
                        }
                    } else {
                        format!("YouTube Video {}", video_id)
                    };
                    
                    let youtube_url = format!("https://www.youtube.com/watch?v={}", video_id);
                    
                    let track = TrackSource::new(
                        title,
                        youtube_url,
                        SourceType::YouTube,
                        UserId::default(),
                    );
                    
                    results.push(track);
                    
                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }
        
        info!("🔍 Scraping directo encontró {} resultados", results.len());
        Ok(results)
    }
}

#[async_trait]
impl MusicSource for InvidiousClient {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<TrackSource>> {
        self.search(query, limit).await
    }

    async fn get_track(&self, url: &str) -> Result<TrackSource> {
        let video_id = Self::extract_video_id(url)?;
        let video_info = self.get_video_info(&video_id).await?;

        let duration = Some(Duration::from_secs(video_info.lengthSeconds));
        let thumbnail = video_info.thumbnails
            .into_iter()
            .find(|t| t.width >= 320)
            .map(|t| t.url);

        let mut track = TrackSource::new(
            video_info.title,
            url.to_string(),
            SourceType::YouTube,
            UserId::default(),
        );

        track = track.with_artist(video_info.author);

        if let Some(duration) = duration {
            track = track.with_duration(duration);
        }

        if let Some(thumbnail) = thumbnail {
            track = track.with_thumbnail(thumbnail);
        }

        Ok(track)
    }

    async fn get_playlist(&self, _url: &str) -> Result<Vec<TrackSource>> {
        warn!("Playlists no soportadas en cliente Invidious");
        Ok(Vec::new())
    }

    fn is_valid_url(&self, url: &str) -> bool {
        url.contains("youtube.com") || url.contains("youtu.be")
    }

    fn source_name(&self) -> &'static str {
        "Invidious"
    }
}