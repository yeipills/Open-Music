use anyhow::Result;
use async_trait::async_trait;
use serenity::model::id::UserId;
use songbird::input::Input;
use std::time::Duration;
use tracing::{info, warn, error};

use super::{MusicSource, TrackSource, SourceType};

/// Cliente optimizado que usa solo yt-dlp + FFmpeg con streaming directo
pub struct YtDlpOptimizedClient;

impl YtDlpOptimizedClient {
    pub fn new() -> Self {
        Self
    }

    /// Verifica que yt-dlp y ffmpeg estén disponibles
    pub async fn verify_dependencies(&self) -> Result<()> {
        // Verificar yt-dlp
        let ytdlp_check = tokio::process::Command::new("yt-dlp")
            .arg("--version")
            .output()
            .await;
        
        match ytdlp_check {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                info!("✅ yt-dlp versión: {}", version.trim());
            }
            _ => {
                error!("❌ yt-dlp no encontrado. Instala con: pip install yt-dlp");
                anyhow::bail!("yt-dlp no disponible");
            }
        }

        // Verificar ffmpeg
        let ffmpeg_check = tokio::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .await;
            
        match ffmpeg_check {
            Ok(output) if output.status.success() => {
                info!("✅ ffmpeg disponible");
            }
            _ => {
                error!("❌ ffmpeg no encontrado. Instala con: sudo apt install ffmpeg");
                anyhow::bail!("ffmpeg no disponible");
            }
        }

        Ok(())
    }

    /// Extrae información del video usando yt-dlp
    async fn extract_video_info(&self, url: &str) -> Result<VideoInfo> {
        let cookies_path = self.find_cookies_file().await?;
        
        let mut cmd = tokio::process::Command::new("yt-dlp");
        cmd.args([
            "--print", "%(title)s|%(uploader)s|%(duration)s|%(thumbnail)s",
            "--default-search", "ytsearch",
            "--no-playlist",
            "--socket-timeout", "30",
            "--retries", "3",
        ]);

        // Agregar cookies si están disponibles
        if let Some(cookies) = cookies_path {
            cmd.args(["--cookies", &cookies]);
        }

        cmd.arg(url);

        let output = cmd.output().await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("yt-dlp info failed: {}", error);
        }

        let info_str = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = info_str.trim().split('|').collect();

        Ok(VideoInfo {
            title: parts.get(0).unwrap_or(&"Unknown").to_string(),
            uploader: parts.get(1).map(|s| s.to_string()),
            duration: parts.get(2)
                .and_then(|s| s.parse::<f64>().ok())
                .map(|d| Duration::from_secs_f64(d)),
            thumbnail: parts.get(3).map(|s| s.to_string()),
        })
    }

    /// Busca archivo de cookies disponible
    async fn find_cookies_file(&self) -> Result<Option<String>> {
        let cookies_paths = vec![
            format!("{}/.config/yt-dlp/cookies.txt", std::env::var("HOME").unwrap_or_default()),
            "/home/openmusic/.config/yt-dlp/cookies.txt".to_string(),
            "/app/.config/yt-dlp/cookies.txt".to_string(),
            "./cookies.txt".to_string(),
        ];

        for path in cookies_paths {
            if tokio::fs::metadata(&path).await.is_ok() {
                info!("🍪 Cookies encontradas en: {}", path);
                return Ok(Some(path));
            }
        }

        warn!("🍪 No se encontraron cookies - algunas funcionalidades pueden estar limitadas");
        Ok(None)
    }

    /// Extrae video ID de URL de YouTube
    #[allow(dead_code)]
    pub fn extract_video_id(url: &str) -> Result<String> {
        use url::Url;
        
        let parsed = Url::parse(url)?;
        
        // youtube.com/watch?v=VIDEO_ID
        if let Some(query) = parsed.query() {
            for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
                if key == "v" {
                    return Ok(value.into_owned());
                }
            }
        }
        
        // youtu.be/VIDEO_ID
        if parsed.host_str() == Some("youtu.be") {
            if let Some(segments) = parsed.path_segments() {
                if let Some(video_id) = segments.into_iter().next() {
                    return Ok(video_id.to_string());
                }
            }
        }
        
        anyhow::bail!("No se pudo extraer video ID de: {}", url)
    }

    /// Verifica si la URL es válida para YouTube
    pub fn is_youtube_url(url: &str) -> bool {
        url.contains("youtube.com") || url.contains("youtu.be") || url.contains("music.youtube.com")
    }
}

#[async_trait]
impl MusicSource for YtDlpOptimizedClient {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<TrackSource>> {
        info!("🔍 Iniciando búsqueda yt-dlp optimizada: {}", query);
        
        let search_query = format!("ytsearch{}:{}", limit.min(5), query);
        
        // Buscar cookies para optimización
        let cookies_path = self.find_cookies_file().await.ok().flatten();
        
        // Usar std::process en lugar de tokio::process para evitar problemas de señales
        let output = tokio::task::spawn_blocking(move || {
            let mut cmd = std::process::Command::new("yt-dlp");
            
            // Argumentos optimizados para máxima velocidad
            cmd.args([
                "--print", "%(webpage_url)s|%(title)s|%(uploader)s|%(duration)s",
                "--default-search", "ytsearch",
                "--skip-download", 
                "--no-playlist",
                "--flat-playlist",
                "--quiet",
                "--no-warnings",
                "--socket-timeout", "15", // Reducido de 30 a 15
                "--retries", "2", // Reducido de 3 a 2
                "--fragment-retries", "1", // Nuevo: solo 1 reintento de fragmento
                "--abort-on-unavailable-fragment", // Nuevo: abortar rápidamente si no está disponible
                "--extractor-args", "youtube:player_client=android_embedded", // Cliente más rápido
                "--geo-bypass",
                "--force-ipv4",
            ]);
            
            // Agregar cookies si están disponibles para evitar throttling
            if let Some(cookies) = cookies_path {
                cmd.args(["--cookies", &cookies]);
            }
            
            cmd.arg(&search_query);
            cmd.output()
        }).await;

        let output = match output {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => anyhow::bail!("yt-dlp process error: {}", e),
            Err(_) => anyhow::bail!("yt-dlp task join error"),
        };

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            warn!("yt-dlp search failed: {}", error);
            anyhow::bail!("yt-dlp search failed: {}", error);
        }

        let results = String::from_utf8_lossy(&output.stdout);
        let mut tracks = Vec::new();

        info!("📋 Procesando {} líneas de resultados", results.lines().count());
        for (i, line) in results.lines().take(limit).enumerate() {
            info!("📄 Línea {}: {}", i + 1, line);
            let parts: Vec<&str> = line.split('|').collect();
            info!("🔗 Partes: {:?}", parts);
            
            if parts.len() >= 4 {
                let track = TrackSource::new(
                    parts[1].to_string(), // title
                    parts[0].to_string(), // url
                    SourceType::YouTube,
                    UserId::new(1), // placeholder válido, será asignado después
                )
                .with_artist(parts[2].to_string())
                .with_duration(
                    parts[3].parse::<f64>().ok()
                        .map(|d| Duration::from_secs_f64(d))
                        .unwrap_or(Duration::from_secs(0))
                );

                info!("✅ Track creado: {}", track.title());
                tracks.push(track);
            } else {
                warn!("⚠️ Línea con formato incorrecto: {}", line);
            }
        }

        info!("🔍 Encontrados {} resultados para: {}", tracks.len(), query);
        Ok(tracks)
    }

    async fn get_track(&self, url: &str) -> Result<TrackSource> {
        if !Self::is_youtube_url(url) {
            anyhow::bail!("URL no es de YouTube: {}", url);
        }

        let video_info = self.extract_video_info(url).await?;

        let track = TrackSource::new(
            video_info.title,
            url.to_string(),
            SourceType::YouTube,
            UserId::new(1), // placeholder válido
        );

        let track = if let Some(artist) = video_info.uploader {
            track.with_artist(artist)
        } else {
            track
        };

        let track = if let Some(duration) = video_info.duration {
            track.with_duration(duration)
        } else {
            track
        };

        let track = if let Some(thumbnail) = video_info.thumbnail {
            track.with_thumbnail(thumbnail)
        } else {
            track
        };

        Ok(track)
    }

    async fn get_playlist(&self, url: &str) -> Result<Vec<TrackSource>> {
        let cookies_path = self.find_cookies_file().await?;
        
        let mut cmd = tokio::process::Command::new("yt-dlp");
        cmd.args([
            url,
            "--print", "%(webpage_url)s|%(title)s|%(uploader)s|%(duration)s|%(thumbnail)s",
            "--flat-playlist",
            "--default-search", "ytsearch",
            "--socket-timeout", "30",
            "--quiet"
        ]);

        if let Some(cookies) = cookies_path {
            cmd.args(["--cookies", &cookies]);
        }

        let output = cmd.output().await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("yt-dlp playlist failed: {}", error);
        }

        let results = String::from_utf8_lossy(&output.stdout);
        let mut tracks = Vec::new();

        for line in results.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 5 {
                let track = TrackSource::new(
                    parts[1].to_string(),
                    parts[0].to_string(),
                    SourceType::YouTube,
                    UserId::new(1),
                )
                .with_artist(parts[2].to_string())
                .with_duration(
                    parts[3].parse::<f64>().ok()
                        .map(|d| Duration::from_secs_f64(d))
                        .unwrap_or(Duration::from_secs(0))
                )
                .with_thumbnail(parts[4].to_string());

                tracks.push(track);
            }
        }

        info!("🎵 Playlist extraída con {} tracks", tracks.len());
        Ok(tracks)
    }

    fn is_valid_url(&self, url: &str) -> bool {
        Self::is_youtube_url(url)
    }

    fn source_name(&self) -> &'static str {
        "YtDlpOptimized"
    }
}

/// Información de video extraída
#[derive(Debug)]
struct VideoInfo {
    title: String,
    uploader: Option<String>,
    duration: Option<Duration>,
    thumbnail: Option<String>,
}

impl TrackSource {
    /// Crea input optimizado usando yt-dlp + FFmpeg streaming directo
    pub async fn get_optimized_input(&self) -> Result<Input> {
        info!("🎵 Creando input ultrarrápido para: {}", self.title());
        
        // Verificar que sea URL de YouTube
        if !YtDlpOptimizedClient::is_youtube_url(&self.url()) {
            anyhow::bail!("Solo se soportan URLs de YouTube");
        }

        // Buscar cookies de forma síncrona (más rápido)
        let cookies_option = [
            format!("{}/.config/yt-dlp/cookies.txt", std::env::var("HOME").unwrap_or_default()),
            "/home/openmusic/.config/yt-dlp/cookies.txt".to_string(),
            "/app/.config/yt-dlp/cookies.txt".to_string(),
            "./cookies.txt".to_string(),
        ]
        .iter()
        .find(|path| std::path::Path::new(path).exists())
        .map(|path| format!("--cookies={}", path));

        // Configurar variables de entorno que songbird respeta
        if let Some(cookies) = cookies_option {
            std::env::set_var("YTDL_COOKIES", cookies.replace("--cookies=", ""));
        }
        
        // Configurar opciones básicas via variables de entorno
        std::env::set_var("YTDL_OPTIONS", "--format=bestaudio[ext=m4a]/bestaudio[ext=webm]/bestaudio/best --no-playlist --quiet --no-warnings --geo-bypass --socket-timeout=20 --retries=3");
        std::env::set_var("YTDL_USER_AGENT", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");

        // Crear el cliente HTTP optimizado para songbird
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20)) // Reducido de 30 a 20
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .tcp_keepalive(Duration::from_secs(30)) // Nuevo: keep-alive para reutilizar conexiones
            .pool_idle_timeout(Duration::from_secs(60)) // Nuevo: pool de conexiones
            .pool_max_idle_per_host(4) // Nuevo: máximo idle por host
            .build()?;

        // CAMBIO CRÍTICO: Usar songbird con configuración estándar
        // songbird maneja internamente los argumentos de yt-dlp
        let ytdl = songbird::input::YoutubeDl::new(client, self.url());
        
        // Verificar que el input sea válido antes de proceder
        info!("🔍 Verificando que el input sea válido...");
        let input = Input::from(ytdl);
        
        // Log adicional para debugging
        info!("🎵 Input creado con configuración optimizada");
        info!("🔗 URL procesada: {}", self.url());

        info!("⚡ Input ultrarrápido creado para: {}", self.title());
        Ok(input)
    }

    /// Método de fallback más simple si el optimizado falla
    pub async fn get_simple_input(&self) -> Result<Input> {
        info!("🔄 Usando método simple de fallback para: {}", self.title());
        
        // Verificar que sea URL de YouTube
        if !YtDlpOptimizedClient::is_youtube_url(&self.url()) {
            anyhow::bail!("Solo se soportan URLs de YouTube");
        }

        // Crear cliente HTTP básico
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (compatible; Discord Music Bot)")
            .build()?;

        // Usar configuración mínima y confiable
        let ytdl = songbird::input::YoutubeDl::new(client, self.url());
        let input = Input::from(ytdl);

        info!("✅ Input simple creado para: {}", self.title());
        Ok(input)
    }
}