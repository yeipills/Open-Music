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
            "/app/config/cookies.txt".to_string(),  // Docker container path
            "./config/cookies.txt".to_string(),
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
        
        // Usar URL de YouTube search en lugar de ytsearch extractor (más confiable)
        let search_url = format!(
            "https://www.youtube.com/results?search_query={}",
            urlencoding::encode(query)
        );
        
        // Buscar cookies para optimización
        let cookies_path = self.find_cookies_file().await.ok().flatten();
        let search_limit = limit.min(5);

        // Usar std::process en lugar de tokio::process para evitar problemas de señales
        let output = tokio::task::spawn_blocking(move || {
            let mut cmd = std::process::Command::new("yt-dlp");
            
            // Argumentos optimizados para máxima velocidad
            cmd.args([
                "--flat-playlist",
                "--print", "%(url)s|%(title)s|%(uploader)s|%(duration)s",
                "--skip-download", 
                "--no-warnings",
                "--socket-timeout", "15",
                "--retries", "2",
                "--geo-bypass",
                "--force-ipv4",
                "--playlist-items", &format!("1:{}", search_limit),
            ]);
            
            // Agregar cookies si están disponibles para evitar throttling
            if let Some(cookies) = cookies_path {
                cmd.args(["--cookies", &cookies]);
            }
            
            cmd.arg(&search_url);
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

        // Buscar cookies
        let cookies_path = [
            "/app/config/cookies.txt".to_string(),  // Docker container path
            "./config/cookies.txt".to_string(),
            format!("{}/.config/yt-dlp/cookies.txt", std::env::var("HOME").unwrap_or_default()),
            "/home/openmusic/.config/yt-dlp/cookies.txt".to_string(),
            "/app/.config/yt-dlp/cookies.txt".to_string(),
            "./cookies.txt".to_string(),
        ]
        .iter()
        .find(|path| std::path::Path::new(path).exists())
        .cloned();

        // Extraer URL de audio directamente con yt-dlp
        let url = self.url();
        let cookies = cookies_path.clone();
        
        let audio_url = tokio::task::spawn_blocking(move || {
            let mut cmd = std::process::Command::new("yt-dlp");
            cmd.args([
                "-f", "bestaudio[ext=m4a]/bestaudio[ext=webm]/bestaudio/best",
                "-g",  // Solo obtener URL, no descargar
                "--no-playlist",
                "--no-check-certificate",
                "--geo-bypass",
                "--socket-timeout", "30",
            ]);
            
            if let Some(cookies_file) = cookies {
                cmd.args(["--cookies", &cookies_file]);
            }
            
            cmd.arg(&url);
            
            let output = cmd.output()?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("yt-dlp falló: {}", stderr);
            }
            
            let audio_url = String::from_utf8_lossy(&output.stdout)
                .lines()
                .last()
                .unwrap_or("")
                .trim()
                .to_string();
            
            if audio_url.is_empty() {
                anyhow::bail!("No se pudo obtener URL de audio");
            }
            
            Ok::<String, anyhow::Error>(audio_url)
        }).await??;
        
        info!("🔗 URL de audio extraída: {}...", &audio_url[..audio_url.len().min(80)]);
        
        // Crear cliente HTTP
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .build()?;

        // Usar HttpRequest para streaming directo de la URL de audio
        let request = songbird::input::HttpRequest::new(client, audio_url);
        let input = Input::from(request);
        
        info!("⚡ Input directo creado para: {}", self.title());
        Ok(input)
    }

    /// Método de fallback más simple si el optimizado falla
    pub async fn get_simple_input(&self) -> Result<Input> {
        info!("🔄 Usando método simple de fallback para: {}", self.title());
        
        // Verificar que sea URL de YouTube
        if !YtDlpOptimizedClient::is_youtube_url(&self.url()) {
            anyhow::bail!("Solo se soportan URLs de YouTube");
        }

        // Configurar opciones específicas para fallback sin cookies (2025 anti-bot)
        std::env::set_var("YTDL_OPTIONS", 
            "--format=bestaudio[ext=m4a]/bestaudio[ext=webm]/bestaudio/18/17/36/5/43/34/35/44/45/46 \
             --extractor-args='youtube:player_client=android,android_embedded,android_creator,android_music,ios,ios_embedded,ios_creator,ios_music,mweb,web_embedded,web_creator,web_music,web_safari' \
             --user-agent='com.google.android.youtube/19.09.37 (Linux; U; Android 13; SM-G998B Build/TP1A.220624.014) gzip' \
             --add-header='X-YouTube-Client-Name: 3' \
             --add-header='X-YouTube-Client-Version: 19.09.37' \
             --add-header='Origin: https://www.youtube.com' \
             --add-header='Referer: https://www.youtube.com/' \
             --no-check-certificate \
             --no-playlist \
             --quiet \
             --ignore-errors \
             --no-abort-on-error \
             --geo-bypass \
             --force-ipv4 \
             --socket-timeout=60 \
             --retries=10 \
             --retry-sleep=3"
        );

        // Crear cliente HTTP básico con Android YouTube App User-Agent
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent("com.google.android.youtube/19.09.37 (Linux; U; Android 13; SM-G998B Build/TP1A.220624.014) gzip")
            .build()?;

        // Usar configuración mínima y confiable
        let ytdl = songbird::input::YoutubeDl::new(client, self.url());
        let input = Input::from(ytdl);

        info!("✅ Input simple creado para: {}", self.title());
        Ok(input)
    }
}