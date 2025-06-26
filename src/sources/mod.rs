pub mod direct_url;
pub mod youtube;
pub mod youtube_fast;

use anyhow::Result;
use async_trait::async_trait;
use serenity::model::id::UserId;
use songbird::input::Input;
use std::time::Duration;

pub use direct_url::DirectUrlClient;
pub use youtube::YouTubeClient;
pub use youtube_fast::YouTubeFastClient;

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

    /// Obtiene el input de audio para songbird (Songbird 0.5.0)
    pub async fn get_input(&self) -> Result<Input> {
        use tracing::{info, error};
        
        info!("🎵 Creando input para: {}", self.title);
        info!("🔗 URL: {}", self.url);
        
        // Songbird 0.5.0: Lazy input creation
        let client = reqwest::Client::new();
        
        // Verificar que la URL sea válida de YouTube
        if !self.url.contains("youtube.com") && !self.url.contains("youtu.be") {
            error!("❌ URL no es de YouTube: {}", self.url);
            anyhow::bail!("URL no compatible: {}", self.url);
        }
        
        // Crear YoutubeDl input (lazy)
        let ytdl = songbird::input::YoutubeDl::new(client, self.url.clone());
        let input = Input::from(ytdl);
        
        info!("✅ Input creado exitosamente para: {}", self.title);
        Ok(input)
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

/// Manager para todas las fuentes de música
pub struct SourceManager {
    youtube: YouTubeClient,
    youtube_fast: YouTubeFastClient,
    direct_url: DirectUrlClient,
}

impl SourceManager {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            youtube: YouTubeClient::new(),
            youtube_fast: YouTubeFastClient::new(),
            direct_url: DirectUrlClient::new(),
        }
    }

    /// Busca en todas las fuentes disponibles (prioriza velocidad)
    #[allow(dead_code)]
    pub async fn search_all(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();

        // Intentar YouTube Fast primero (más rápido)
        match tokio::time::timeout(
            std::time::Duration::from_secs(8),
            self.youtube_fast.search(query, limit)
        ).await {
            Ok(Ok(tracks)) => {
                results.push(SearchResult {
                    tracks,
                    total: limit,
                    source: SourceType::YouTube,
                });
                return Ok(results);
            }
            _ => {
                tracing::warn!("YouTube Fast falló, usando YouTube normal");
            }
        }

        // Fallback a YouTube normal si fast falla
        if let Ok(tracks) = self.youtube.search(query, limit).await {
            results.push(SearchResult {
                tracks,
                total: limit,
                source: SourceType::YouTube,
            });
        }

        Ok(results)
    }

    /// Detecta y obtiene track de URL
    #[allow(dead_code)]
    pub async fn get_track_from_url(&self, url: &str, _requested_by: UserId) -> Result<TrackSource> {
        // Intentar YouTube primero
        if self.youtube.is_valid_url(url) {
            return self.youtube.get_track(url).await;
        }

        // Por último, intentar URL directa
        if self.direct_url.is_valid_url(url) {
            return self.direct_url.get_track(url).await;
        }

        anyhow::bail!("URL no soportada: {}", url)
    }
}
