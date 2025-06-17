pub mod direct_url;
pub mod soundcloud;
pub mod spotify;
pub mod tidal;
pub mod youtube;

use anyhow::Result;
use async_trait::async_trait;
use serenity::model::id::UserId;
use songbird::input::Input;
use std::time::Duration;

pub use direct_url::DirectUrlClient;
pub use soundcloud::SoundCloudClient;
pub use spotify::SpotifyClient;
pub use tidal::TidalClient;
pub use youtube::YouTubeClient;

/// Trait común para todas las fuentes de música
#[async_trait]
pub trait MusicSource {
    /// Busca tracks en la fuente
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<TrackSource>>;

    /// Obtiene información detallada de un track por URL
    async fn get_track(&self, url: &str) -> Result<TrackSource>;

    /// Obtiene tracks de una playlist
    async fn get_playlist(&self, url: &str) -> Result<Vec<TrackSource>>;

    /// Verifica si la URL es válida para esta fuente
    fn is_valid_url(&self, url: &str) -> bool;

    /// Nombre de la fuente
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

    pub fn with_stream_url(mut self, stream_url: String) -> Self {
        self.stream_url = Some(stream_url);
        self
    }

    /// Obtiene el input de audio para songbird
    pub async fn get_input(&self) -> Result<Input> {
        match &self.stream_url {
            Some(_url) => {
                // URL directa de stream - usar YoutubeDl con client
                let client = reqwest::Client::new();
                let source = songbird::input::YoutubeDl::new(client, self.url.clone());
                Ok(source.into())
            }
            None => {
                // Usar URL principal (ej: YouTube)
                let client = reqwest::Client::new();
                let source = songbird::input::YoutubeDl::new(client, self.url.clone());
                Ok(source.into())
            }
        }
    }
}

/// Tipos de fuentes de música
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SourceType {
    YouTube,
    Spotify,
    SoundCloud,
    Tidal,
    DirectUrl,
}

impl SourceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceType::YouTube => "youtube",
            SourceType::Spotify => "spotify",
            SourceType::SoundCloud => "soundcloud",
            SourceType::Tidal => "tidal",
            SourceType::DirectUrl => "direct",
        }
    }
}

/// Información de resultado de búsqueda
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub tracks: Vec<TrackSource>,
    pub total: usize,
    pub source: SourceType,
}

/// Manager para todas las fuentes de música
pub struct SourceManager {
    youtube: YouTubeClient,
    spotify: Option<SpotifyClient>,
    soundcloud: Option<SoundCloudClient>,
    tidal: Option<TidalClient>,
    direct_url: DirectUrlClient,
}

impl SourceManager {
    pub fn new() -> Self {
        Self {
            youtube: YouTubeClient::new(),
            spotify: None,
            soundcloud: None,
            tidal: None,
            direct_url: DirectUrlClient::new(),
        }
    }

    pub fn with_spotify(mut self, client_id: String, client_secret: String) -> Self {
        self.spotify = Some(SpotifyClient::new(client_id, client_secret));
        self
    }

    pub fn with_soundcloud(mut self, client_id: String) -> Self {
        self.soundcloud = Some(SoundCloudClient::new(client_id));
        self
    }

    pub fn with_tidal(mut self, api_key: String) -> Self {
        self.tidal = Some(TidalClient::new(api_key));
        self
    }

    /// Busca en todas las fuentes disponibles
    pub async fn search_all(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();

        // YouTube (siempre disponible)
        if let Ok(tracks) = self.youtube.search(query, limit).await {
            results.push(SearchResult {
                tracks,
                total: limit,
                source: SourceType::YouTube,
            });
        }

        // Spotify (si está configurado)
        if let Some(ref spotify) = self.spotify {
            if let Ok(tracks) = spotify.search(query, limit).await {
                results.push(SearchResult {
                    tracks,
                    total: limit,
                    source: SourceType::Spotify,
                });
            }
        }

        // SoundCloud (si está configurado)
        if let Some(ref soundcloud) = self.soundcloud {
            if let Ok(tracks) = soundcloud.search(query, limit).await {
                results.push(SearchResult {
                    tracks,
                    total: limit,
                    source: SourceType::SoundCloud,
                });
            }
        }

        Ok(results)
    }

    /// Detecta y obtiene track de URL
    pub async fn get_track_from_url(&self, url: &str, _requested_by: UserId) -> Result<TrackSource> {
        // Intentar YouTube primero
        if self.youtube.is_valid_url(url) {
            return self.youtube.get_track(url).await;
        }

        // Intentar Spotify
        if let Some(ref spotify) = self.spotify {
            if spotify.is_valid_url(url) {
                return spotify.get_track(url).await;
            }
        }

        // Intentar SoundCloud
        if let Some(ref soundcloud) = self.soundcloud {
            if soundcloud.is_valid_url(url) {
                return soundcloud.get_track(url).await;
            }
        }

        // Intentar Tidal
        if let Some(ref tidal) = self.tidal {
            if tidal.is_valid_url(url) {
                return tidal.get_track(url).await;
            }
        }

        // Por último, intentar URL directa
        if self.direct_url.is_valid_url(url) {
            return self.direct_url.get_track(url).await;
        }

        anyhow::bail!("URL no soportada: {}", url)
    }
}
