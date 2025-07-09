pub mod direct_url;
pub mod youtube;
pub mod youtube_fast;
pub mod invidious;
pub mod youtube_rss;
pub mod youtube_enhanced;
pub mod youtube_api_v3;
pub mod smart_source;

use anyhow::Result;
use async_trait::async_trait;
use serenity::model::id::UserId;
use songbird::input::Input;
use std::time::Duration;
use tracing::{info, warn, debug};

pub use direct_url::DirectUrlClient;
pub use youtube::YouTubeClient;
pub use youtube_fast::YouTubeFastClient;
pub use invidious::InvidiousClient;
pub use youtube_rss::YouTubeRssClient;
pub use youtube_enhanced::EnhancedYouTubeClient;
pub use youtube_api_v3::YouTubeAPIv3Client;
pub use smart_source::SmartSource;


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

    /// Obtiene el input de audio para songbird con fallback a Invidious
    pub async fn get_input(&self) -> Result<Input> {
        use tracing::{info, error, warn};
        
        
        
        info!("🎵 Creando input para: {}", self.title);
        info!("🔗 URL: {}", self.url);
        
        // Si tenemos stream_url directo, usarlo
        if let Some(stream_url) = &self.stream_url {
            info!("🎯 Usando URL directa de stream: {}", stream_url);
            return self.create_direct_input(stream_url).await;
        }
        
        // Intentar con yt-dlp primero
        match self.try_ytdlp_input().await {
            Ok(input) => {
                info!("✅ Input creado con yt-dlp para: {}", self.title);
                return Ok(input);
            }
            Err(e) => {
                let error_msg = e.to_string();
                warn!("❌ yt-dlp falló: {}", error_msg);
                
                // Detectar errores SSAP y aplicar estrategias específicas
                if Self::is_ssap_error(&error_msg) {
                    warn!("🔍 Error SSAP detectado, aplicando recuperación...");
                    match self.handle_ssap_error().await {
                        Ok(input) => {
                            info!("✅ Recuperación SSAP exitosa para: {}", self.title);
                            return Ok(input);
                        }
                        Err(recovery_error) => {
                            warn!("❌ Recuperación SSAP falló: {}", recovery_error);
                        }
                    }
                } else {
                    warn!("❌ Error no-SSAP, intentando con Invidious...");
                }
            }
        }
        
        // Fallback a Invidious
        match self.try_invidious_input().await {
            Ok(input) => {
                info!("✅ Input creado con Invidious para: {}", self.title);
                Ok(input)
            }
            Err(e) => {
                error!("❌ Todos los métodos fallaron para: {} - {}", self.url, e);
                anyhow::bail!("No se pudo crear input de audio: {}", e)
            }
        }
    }

    /// Intenta crear input usando yt-dlp
    async fn try_ytdlp_input(&self) -> Result<Input> {
        use std::time::Duration;
        use tokio::time::timeout;
        use tracing::{info, warn};
        
        // Verificar que yt-dlp esté disponible
        self.verify_ytdlp_availability().await?;
        
        // Validar URL de YouTube
        self.validate_youtube_url()?;
        
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        
        // Configurar variables de entorno para yt-dlp con múltiples estrategias anti-SSAP
        let cookies_paths = vec![
            "/home/openmusic/.config/yt-dlp/cookies.txt".to_string(),
            std::env::var("HOME").unwrap_or_else(|_| ".".to_string()) + "/.config/yt-dlp/cookies.txt",
            "/app/.config/yt-dlp/cookies.txt".to_string(),
        ];
        
        let cookies_option = cookies_paths
            .iter()
            .find(|path| {
                let exists = std::path::Path::new(path).exists();
                if exists {
                    info!("🍪 Cookies encontradas en: {}", path);
                } else {
                    debug!("🍪 No se encontraron cookies en: {}", path);
                }
                exists
            })
            .map(|path| format!("--cookies '{}' ", path))
            .unwrap_or_else(|| {
                warn!("🍪 No se encontraron cookies en ningún path, usando configuración sin cookies");
                String::new()
            });
        
        let opts = format!(
            "{}--user-agent 'Mozilla/5.0 (Linux; Android 11; SM-A515F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36' \
            --extractor-args 'youtube:player_client=android_embedded,android_creator,tv_embed' \
            --extractor-args 'youtube:player_js_variant=main' \
            --extractor-args 'youtube:skip=dash,hls' \
            --no-check-certificate --socket-timeout 30 --retries 3 \
            --retry-sleep 1 --fragment-retries 3 \
            --http-chunk-size 5M --concurrent-fragments 1 \
            --format 'bestaudio[ext=m4a]/bestaudio[ext=webm]/bestaudio/best[height<=720]/best' \
            --ignore-errors --no-abort-on-error --quiet --no-warnings",
            cookies_option
        );
        
        std::env::set_var("YTDLP_OPTS", &opts);
        info!("🔧 Configurando yt-dlp con opciones anti-SSAP: {}", &opts[..std::cmp::min(100, opts.len())]);
        
        // Intentar múltiples configuraciones si la primera falla
        for attempt in 1..=3 {
            info!("🔄 Intento {} de creación de input con yt-dlp", attempt);
            
            let ytdl_future = async {
                let ytdl = songbird::input::YoutubeDl::new(client.clone(), self.url.clone());
                Input::from(ytdl)
            };
            
            match timeout(Duration::from_secs(25), ytdl_future).await {
                Ok(input) => {
                    info!("✅ Input creado exitosamente en intento {}", attempt);
                    return Ok(input);
                },
                Err(_) => {
                    warn!("⏰ Timeout en intento {} de yt-dlp", attempt);
                    if attempt < 3 {
                        // Cambiar estrategia para próximo intento
                        let fallback_opts = match attempt {
                            1 => format!("{}--user-agent 'Mozilla/5.0 (compatible; Googlebot/2.1)' --extractor-args 'youtube:player_client=android_embedded' --format 'bestaudio/best[height<=480]/best' --quiet --no-warnings", cookies_option),
                            2 => format!("{}--user-agent 'Mozilla/5.0 (iPad; CPU OS 14_0 like Mac OS X)' --extractor-args 'youtube:player_client=ios' --format 'bestaudio[ext=webm]/bestaudio/best[height<=360]/best' --quiet --no-warnings", cookies_option),
                            _ => opts.clone()
                        };
                        std::env::set_var("YTDLP_OPTS", &fallback_opts);
                        info!("🔄 Cambiando a estrategia fallback {}", attempt + 1);
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
        }
        
        anyhow::bail!("Todos los intentos de yt-dlp fallaron con timeout")
    }

    /// Intenta crear input usando Invidious
    async fn try_invidious_input(&self) -> Result<Input> {
        use tracing::info;
        
        let video_id = InvidiousClient::extract_video_id(&self.url)?;
        info!("🔗 Extrayendo audio directo para video ID: {}", video_id);
        
        let invidious_client = InvidiousClient::new();
        let audio_url = invidious_client.get_audio_url(&video_id).await?;
        
        info!("✅ URL de audio directo obtenida: {}", &audio_url[..50.min(audio_url.len())]);
        self.create_direct_input(&audio_url).await
    }

    /// Crea input desde URL directa
    async fn create_direct_input(&self, url: &str) -> Result<Input> {
        use std::time::Duration;
        
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        
        let input = songbird::input::HttpRequest::new(client, url.to_string());
        Ok(Input::from(input))
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
    #[allow(dead_code)]
    async fn verify_video_accessibility(&self) -> Result<()> {
        // Verificar si el video es accesible
        let client = reqwest::Client::new();
        let response = client.head(&self.url).send().await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Video no accesible: {}", response.status());
        }
        
        Ok(())
    }

    /// Detecta y maneja problemas relacionados con SSAP de YouTube
    pub fn is_ssap_error(error_msg: &str) -> bool {
        let error_lower = error_msg.to_lowercase();
        error_lower.contains("ssap") 
        || error_lower.contains("server-side ads") 
        || error_lower.contains("signature extraction failed")
        || error_lower.contains("some web client https formats have been skipped")
        || error_lower.contains("requested format is not available")
        || error_lower.contains("some formats may be missing")
        || error_lower.contains("yt-dlp failed with non-zero status code")
        || error_lower.contains("failed to get formats")
        || error_lower.contains("no video formats found")
        || (error_lower.contains("youtube") && error_lower.contains("non-zero status"))
    }

    /// Aplica estrategias de recuperación para errores SSAP
    pub async fn handle_ssap_error(&self) -> Result<Input> {
        use tracing::{warn, info};
        
        warn!("🔄 Detectado problema SSAP, aplicando estrategias de recuperación...");
        
        // Estrategia 1: Intentar con android_embedded
        let android_future = self.try_ytdlp_with_client("android_embedded").await;
        if android_future.is_ok() {
            info!("✅ Recuperación exitosa con cliente Android Embedded");
            return android_future;
        }
        
        // Estrategia 2: Usar ios
        let ios_future = self.try_ytdlp_with_client("ios").await;
        if ios_future.is_ok() {
            info!("✅ Recuperación exitosa con cliente iOS");
            return ios_future;
        }
        
        // Estrategia 3: Usar tv_embed
        let tv_future = self.try_ytdlp_with_client("tv_embed").await;
        if tv_future.is_ok() {
            info!("✅ Recuperación exitosa con cliente TV");
            return tv_future;
        }
        
        // Estrategia 4: Fallback a Invidious
        warn!("🔄 Todos los clientes yt-dlp fallaron, usando Invidious...");
        self.try_invidious_input().await
    }

    /// Intenta yt-dlp con un cliente específico
    async fn try_ytdlp_with_client(&self, client: &str) -> Result<Input> {
        use std::time::Duration;
        use tokio::time::timeout;
        
        let client_reqwest = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        
        // Verificar cookies - priorizar path del contenedor Docker
        let cookies_paths = vec![
            "/home/openmusic/.config/yt-dlp/cookies.txt".to_string(),
            std::env::var("HOME").unwrap_or_else(|_| ".".to_string()) + "/.config/yt-dlp/cookies.txt",
            "/app/.config/yt-dlp/cookies.txt".to_string(),
        ];
        
        let cookies_option = cookies_paths
            .iter()
            .find(|path| {
                let exists = std::path::Path::new(path).exists();
                if exists {
                    info!("🍪 Cookies encontradas en: {}", path);
                } else {
                    debug!("🍪 No se encontraron cookies en: {}", path);
                }
                exists
            })
            .map(|path| format!("--cookies '{}' ", path))
            .unwrap_or_else(|| {
                warn!("🍪 No se encontraron cookies en ningún path, usando configuración sin cookies");
                String::new()
            });
        
        // Configurar para cliente específico con fallback más agresivo
        let opts = format!(
            "{}--user-agent 'Mozilla/5.0 (Linux; Android 11; SM-A515F) AppleWebKit/537.36' \
            --extractor-args 'youtube:player_client={}' \
            --format 'bestaudio[ext=m4a]/bestaudio[ext=webm]/bestaudio/best[height<=720]/best' \
            --ignore-errors --no-abort-on-error --socket-timeout 20 --quiet --no-warnings",
            cookies_option, client
        );
        std::env::set_var("YTDLP_OPTS", &opts);
        
        let ytdl_future = async {
            let ytdl = songbird::input::YoutubeDl::new(client_reqwest, self.url.clone());
            Input::from(ytdl)
        };
        
        timeout(Duration::from_secs(25), ytdl_future).await
            .map_err(|_| anyhow::anyhow!("Timeout con cliente {}", client))
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
