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

    /// Obtiene el input de audio para songbird (Songbird 0.5.0) con validación mejorada
    pub async fn get_input(&self) -> Result<Input> {
        use tracing::{info, error, warn};
        use std::process::Command;
        use std::time::Duration;
        use tokio::time::timeout;
        
        info!("🎵 Creando input para: {}", self.title);
        info!("🔗 URL: {}", self.url);
        
        // 1. Verificar que yt-dlp esté disponible
        self.verify_ytdlp_availability().await?;
        
        // 2. Validar URL de YouTube más robustamente
        self.validate_youtube_url()?;
        
        // 3. Verificar que el video sea accesible (comentado temporalmente debido a bloqueos de YouTube)
        // self.verify_video_accessibility().await?;
        info!("⚠️ Saltando verificación de accesibilidad por bloqueos de YouTube - los errores se manejarán en reproducción");
        
        // 4. Crear input con timeout
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        
        info!("🔧 Creando YoutubeDl input...");
        
        // Configurar variables de entorno para yt-dlp antes de crear el input
        std::env::set_var("YTDLP_OPTS", "--user-agent 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36' --extractor-args 'youtube:player_client=android'");
        
        // Crear YoutubeDl input tradicional
        let ytdl_future = async {
            let ytdl = songbird::input::YoutubeDl::new(client, self.url.clone());
            Input::from(ytdl)
        };
        
        let input = match timeout(Duration::from_secs(45), ytdl_future).await {
            Ok(input) => input,
            Err(_) => {
                error!("❌ Timeout creando input para: {}", self.url);
                anyhow::bail!("Timeout al crear input de audio");
            }
        };
        
        info!("✅ Input creado exitosamente para: {}", self.title);
        Ok(input)
    }
    
    /// Verifica que yt-dlp esté disponible y funcional
    async fn verify_ytdlp_availability(&self) -> Result<()> {
        use tracing::{info, error};
        
        // Verificar que yt-dlp existe
        let output = tokio::process::Command::new("which")
            .arg("yt-dlp")
            .output()
            .await;
            
        match output {
            Ok(output) if output.status.success() => {
                info!("✅ yt-dlp encontrado");
            }
            _ => {
                error!("❌ yt-dlp no está instalado o no está en PATH");
                anyhow::bail!("yt-dlp no está disponible");
            }
        }
        
        // Verificar que yt-dlp puede ejecutarse
        let version_output = tokio::process::Command::new("yt-dlp")
            .arg("--version")
            .output()
            .await;
            
        match version_output {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                info!("✅ yt-dlp versión: {}", version.trim());
            }
            _ => {
                error!("❌ Error ejecutando yt-dlp");
                anyhow::bail!("yt-dlp no puede ejecutarse correctamente");
            }
        }
        
        Ok(())
    }
    
    /// Valida que la URL sea de YouTube y esté bien formada
    fn validate_youtube_url(&self) -> Result<()> {
        use url::Url;
        use tracing::{info, error};
        
        // Parsear URL
        let parsed_url = Url::parse(&self.url)
            .map_err(|_| anyhow::anyhow!("URL mal formada: {}", self.url))?;
        
        // Verificar dominio
        let host = parsed_url.host_str()
            .ok_or_else(|| anyhow::anyhow!("No se pudo extraer host de la URL"))?;
        
        let is_youtube = host == "www.youtube.com" 
            || host == "youtube.com" 
            || host == "youtu.be"
            || host == "m.youtube.com"
            || host == "music.youtube.com";
        
        if !is_youtube {
            error!("❌ URL no es de YouTube: {}", self.url);
            anyhow::bail!("URL no es de YouTube: {}", host);
        }
        
        info!("✅ URL de YouTube válida: {}", host);
        Ok(())
    }
    
    /// Verifica que el video sea accesible antes de crear el input
    async fn verify_video_accessibility(&self) -> Result<()> {
        use tracing::{info, warn, error};
        use std::time::Duration;
        use tokio::time::timeout;
        
        info!("🔍 Verificando accesibilidad del video...");
        
        // Usar yt-dlp para verificar que el video existe y es accesible
        let check_future = tokio::process::Command::new("yt-dlp")
            .args(&[
                "--simulate",
                "--no-warnings", 
                "--quiet",
                "--get-title",
                "--user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
                "--extractor-args", "youtube:player_client=web",
                "--no-check-certificate",
                &self.url
            ])
            .output();
        
        let output = match timeout(Duration::from_secs(15), check_future).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                error!("❌ Error ejecutando yt-dlp para verificación: {:?}", e);
                anyhow::bail!("Error verificando video: {:?}", e);
            }
            Err(_) => {
                warn!("⚠️ Timeout verificando video, continuando...");
                return Ok(()); // Continuar si hay timeout en verificación
            }
        };
        
        if output.status.success() {
            let title = String::from_utf8_lossy(&output.stdout);
            info!("✅ Video accesible: {}", title.trim());
        } else {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            error!("❌ Video no accesible: {}", error_msg.trim());
            
            // Verificar errores específicos
            if error_msg.contains("Video unavailable") || error_msg.contains("Private video") {
                anyhow::bail!("Video no disponible o privado");
            } else if error_msg.contains("Age-restricted") {
                anyhow::bail!("Video restringido por edad");
            } else {
                anyhow::bail!("Error accediendo al video: {}", error_msg.trim());
            }
        }
        
        Ok(())
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
