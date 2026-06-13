pub mod ytdlp_optimized;

use anyhow::Result;
use async_trait::async_trait;
use serenity::model::id::UserId;
use songbird::input::Input;
use std::time::Duration;
use tracing::info;

pub use ytdlp_optimized::YtDlpOptimizedClient;


/// Trait común para todas las fuentes de música
#[async_trait]
pub trait MusicSource {
    /// Busca tracks en la fuente
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<TrackSource>>;

    /// Obtiene información detallada de un track por URL
    async fn get_track(&self, url: &str) -> Result<TrackSource>;

    /// Obtiene tracks de una playlist
    #[allow(dead_code)]
    async fn get_playlist(&self, url: &str) -> Result<Vec<TrackSource>>;

    /// Verifica si la URL es válida para esta fuente
    fn is_valid_url(&self, url: &str) -> bool;

    /// Nombre de la fuente
    #[allow(dead_code)]
    fn source_name(&self) -> &'static str;
}

/// Representa un track de música
#[derive(Debug, Clone)]
pub struct TrackSource {
    title: String,
    artist: Option<String>,
    duration: Option<Duration>,
    thumbnail: Option<String>,
    url: String,
    stream_url: Option<String>,
    source_type: SourceType,
    requested_by: UserId,
}

impl TrackSource {
    pub fn new(title: String, url: String, source_type: SourceType, requested_by: UserId) -> Self {
        Self {
            title,
            artist: None,
            duration: None,
            thumbnail: None,
            url,
            stream_url: None,
            source_type,
            requested_by,
        }
    }

    // Getters
    pub fn title(&self) -> String {
        self.title.clone()
    }
    pub fn artist(&self) -> Option<String> {
        self.artist.clone()
    }
    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }
    pub fn thumbnail(&self) -> Option<String> {
        self.thumbnail.clone()
    }
    pub fn url(&self) -> String {
        self.url.clone()
    }
    #[allow(dead_code)]
    pub fn stream_url(&self) -> Option<String> {
        self.stream_url.clone()
    }
    pub fn source_type(&self) -> SourceType {
        self.source_type
    }
    pub fn requested_by(&self) -> UserId {
        self.requested_by
    }

    // Setters
    pub fn with_artist(mut self, artist: String) -> Self {
        self.artist = Some(artist);
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn with_thumbnail(mut self, thumbnail: String) -> Self {
        self.thumbnail = Some(thumbnail);
        self
    }

    #[allow(dead_code)]
    pub fn with_stream_url(mut self, stream_url: String) -> Self {
        self.stream_url = Some(stream_url);
        self
    }

    #[allow(dead_code)]
    pub fn with_requested_by(mut self, user_id: UserId) -> Self {
        self.requested_by = user_id;
        self
    }

    #[allow(dead_code)]
    pub fn with_source_type(mut self, source_type: SourceType) -> Self {
        self.source_type = source_type;
        self
    }

    /// Obtiene el input de audio con efectos (yt-dlp | ffmpeg `-af <filter>`).
    ///
    /// `filter` es la cadena de filtros ffmpeg construida por `AudioEffects::build_filter`
    /// (loudnorm + EQ del preset). Si la cadena ffmpeg falla, cae a un fallback sin efectos.
    pub async fn get_input(&self, filter: &str) -> Result<Input> {
        info!("🎵 Creando input para: {}", self.title);

        // Camino principal: yt-dlp | ffmpeg con loudnorm + EQ reales
        match self.get_ffmpeg_input(filter).await {
            Ok(input) => Ok(input),
            Err(e) => {
                tracing::warn!("⚠️ Cadena ffmpeg falló: {:?}, fallback sin efectos...", e);
                // Fallback sin efectos: streaming directo vía songbird YoutubeDl
                self.get_simple_input().await
            }
        }
    }
}

/// Tipos de fuentes de música
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum SourceType {
    YouTube,
    DirectUrl,
}

impl SourceType {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceType::YouTube => "youtube",
            SourceType::DirectUrl => "direct",
        }
    }
}

/// Información de resultado de búsqueda
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SearchResult {
    pub tracks: Vec<TrackSource>,
    pub total: usize,
    pub source: SourceType,
}

/// Manager optimizado para extracción de música usando solo yt-dlp
pub struct SourceManager {
    ytdlp: YtDlpOptimizedClient,
}

impl SourceManager {
    pub fn new() -> Self {
        Self {
            ytdlp: YtDlpOptimizedClient::new(),
        }
    }

    /// Verifica que todas las dependencias estén disponibles
    pub async fn verify_dependencies(&self) -> Result<()> {
        self.ytdlp.verify_dependencies().await
    }

    /// Busca música usando yt-dlp optimizado
    pub async fn search_all(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let tracks = self.ytdlp.search(query, limit).await?;
        
        let results = vec![SearchResult {
            tracks,
            total: limit,
            source: SourceType::YouTube,
        }];

        Ok(results)
    }

    /// Obtiene track de URL usando yt-dlp optimizado
    pub async fn get_track_from_url(&self, url: &str, requested_by: UserId) -> Result<TrackSource> {
        if self.ytdlp.is_valid_url(url) {
            let track = self.ytdlp.get_track(url).await?;
            return Ok(track.with_requested_by(requested_by));
        }

        anyhow::bail!("URL no soportada (solo YouTube): {}", url)
    }
}
