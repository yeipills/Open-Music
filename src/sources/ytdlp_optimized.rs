use anyhow::Result;
use async_trait::async_trait;
use serenity::model::id::UserId;
use songbird::input::Input;
use std::time::Duration;
use tracing::{debug, info, warn, error};

use super::{MusicSource, TrackSource, SourceType};

/// Extractor-arg que apunta yt-dlp al proveedor de PO Tokens (servicio
/// `bgutil-provider` en la red del compose). Evita el bloqueo anti-bot de
/// YouTube ("Sign in to confirm you're not a bot") desde IPs de datacenter.
/// El base_url es overridable por env `POT_PROVIDER_URL`.
fn pot_extractor_arg() -> String {
    let base = std::env::var("POT_PROVIDER_URL")
        .unwrap_or_else(|_| "http://bgutil-provider:4416".to_string());
    format!("youtubepot-bgutilhttp:base_url={base}")
}

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
        let cookies_path = Self::cookies_working_copy();

        let pot_arg = pot_extractor_arg();
        let mut cmd = tokio::process::Command::new("yt-dlp");
        cmd.args([
            "--ignore-config",
            "--print", "%(title)s|%(uploader)s|%(duration)s|%(thumbnail)s",
            "--no-playlist",
            "--socket-timeout", "30",
            "--retries", "3",
            "--extractor-args", &pot_arg,
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

    /// Devuelve una **copia temporal descartable** del cookies.txt para pasarle a
    /// yt-dlp. yt-dlp reescribe el archivo de cookies al terminar; pasarle el
    /// original hace que invocaciones concurrentes (búsqueda + descarga) se pisen
    /// y **degraden/quemen** las cookies en horas. Con una copia por invocación,
    /// el `config/cookies.txt` original queda intacto y las cookies duran hasta su
    /// expiración natural. Las copias viejas (>10 min) se limpian best-effort.
    /// Si no hay cookies, devuelve None (yt-dlp lo intenta sin ellas).
    pub fn cookies_working_copy() -> Option<String> {
        // Limpieza best-effort de copias huérfanas en tmpfs.
        if let Ok(entries) = std::fs::read_dir("/tmp") {
            let now = std::time::SystemTime::now();
            for e in entries.flatten() {
                let name = e.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("om_cookies_") && name.ends_with(".txt") {
                    if let Ok(modified) = e.metadata().and_then(|m| m.modified()) {
                        if now.duration_since(modified).map(|d| d.as_secs() > 600).unwrap_or(false) {
                            let _ = std::fs::remove_file(e.path());
                        }
                    }
                }
            }
        }

        let original = Self::find_cookies_path()?;
        let dst = format!("/tmp/om_cookies_{}.txt", fastrand::u64(..));
        match std::fs::copy(&original, &dst) {
            Ok(_) => Some(dst),
            Err(e) => {
                warn!("🍪 No se pudo copiar cookies a tmp ({e}); usando original");
                Some(original)
            }
        }
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

    /// Busca un archivo de cookies en las rutas conocidas (versión síncrona).
    pub fn find_cookies_path() -> Option<String> {
        [
            "/app/config/cookies.txt".to_string(),
            "./config/cookies.txt".to_string(),
            format!("{}/.config/yt-dlp/cookies.txt", std::env::var("HOME").unwrap_or_default()),
            "/home/openmusic/.config/yt-dlp/cookies.txt".to_string(),
            "/app/.config/yt-dlp/cookies.txt".to_string(),
            "./cookies.txt".to_string(),
        ]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())
    }

    /// Lanza yt-dlp en streaming *lazy* para una playlist: emite una línea por
    /// track a medida que los procesa, sin esperar a listar toda la lista. No pide
    /// thumbnail (se resuelve al reproducir cada track) para acelerar la aparición.
    /// Formato por línea: `url|title|uploader|duration`.
    pub fn spawn_playlist_stream(
        url: &str,
        cookies: Option<&str>,
    ) -> std::io::Result<tokio::process::Child> {
        let pot_arg = pot_extractor_arg();
        let mut cmd = tokio::process::Command::new("yt-dlp");
        cmd.args([
            url,
            "--ignore-config",
            "--flat-playlist",
            "--lazy-playlist",
            "--print", "%(url)s|%(title)s|%(uploader)s|%(duration)s",
            "--no-warnings",
            "--socket-timeout", "30",
            "--extractor-args", &pot_arg,
        ]);
        if let Some(c) = cookies {
            cmd.args(["--cookies", c]);
        }
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        cmd.spawn()
    }

    /// Parsea una línea del stream de playlist (`url|title|uploader|duration`).
    pub fn parse_playlist_line(line: &str, requested_by: UserId) -> Option<TrackSource> {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 2 || parts[0].is_empty() || parts[0] == "NA" {
            return None;
        }
        let mut track = TrackSource::new(
            parts[1].to_string(),
            parts[0].to_string(),
            SourceType::YouTube,
            requested_by,
        );
        if let Some(artist) = parts.get(2) {
            if !artist.is_empty() && *artist != "NA" {
                track = track.with_artist(artist.to_string());
            }
        }
        if let Some(dur) = parts.get(3).and_then(|s| s.parse::<f64>().ok()) {
            track = track.with_duration(Duration::from_secs_f64(dur));
        }
        Some(track)
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
        
        // Copia descartable de cookies (yt-dlp reescribe el archivo)
        let cookies_path = Self::cookies_working_copy();
        let search_limit = limit.min(5);
        let pot_arg = pot_extractor_arg();

        // Usar std::process en lugar de tokio::process para evitar problemas de señales
        let output = tokio::task::spawn_blocking(move || {
            let mut cmd = std::process::Command::new("yt-dlp");

            // Argumentos optimizados para máxima velocidad
            cmd.args([
                "--ignore-config",
                "--flat-playlist",
                "--print", "%(url)s|%(title)s|%(uploader)s|%(duration)s",
                "--skip-download",
                "--no-warnings",
                "--socket-timeout", "15",
                "--retries", "2",
                "--geo-bypass",
                "--force-ipv4",
                "--extractor-args", &pot_arg,
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

        debug!("📋 Procesando {} líneas de resultados", results.lines().count());
        for (i, line) in results.lines().take(limit).enumerate() {
            debug!("📄 Línea {}: {}", i + 1, line);
            let parts: Vec<&str> = line.split('|').collect();
            debug!("🔗 Partes: {:?}", parts);

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

                debug!("✅ Track creado: {}", track.title());
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
        let cookies_path = Self::cookies_working_copy();

        let pot_arg = pot_extractor_arg();
        let mut cmd = tokio::process::Command::new("yt-dlp");
        // Sin thumbnail (se resuelve al reproducir) + lazy para emitir antes.
        cmd.args([
            url,
            "--ignore-config",
            "--print", "%(url)s|%(title)s|%(uploader)s|%(duration)s",
            "--flat-playlist",
            "--lazy-playlist",
            "--socket-timeout", "30",
            "--extractor-args", &pot_arg,
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
        let tracks: Vec<TrackSource> = results
            .lines()
            .filter_map(|line| Self::parse_playlist_line(line, UserId::new(1)))
            .collect();

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
    /// Input principal con efectos: cadena `yt-dlp -o - | ffmpeg -af <filter>`.
    ///
    /// yt-dlp streamea el mejor audio Opus al stdout; ffmpeg aplica los filtros
    /// (`loudnorm` + EQ) y entrega WAV/PCM que songbird decodifica. Una sola pasada
    /// de transcode. Encadenar los dos procesos evita el problema de URLs `-g` que
    /// expiran. Devuelve un `Input` vía `ChildContainer`.
    pub async fn get_ffmpeg_input(&self, filter: &str) -> Result<Input> {
        use std::process::{Command, Stdio};

        if !YtDlpOptimizedClient::is_youtube_url(&self.url()) {
            anyhow::bail!("Solo se soportan URLs de YouTube");
        }

        let cookies_path = YtDlpOptimizedClient::cookies_working_copy();

        let url = self.url();
        let filter = filter.to_string();
        let title = self.title();
        let pot_arg = pot_extractor_arg();

        let input = tokio::task::spawn_blocking(move || -> Result<Input> {
            // 1) yt-dlp streamea el contenedor de mejor audio a stdout
            let mut ytdlp_cmd = Command::new("yt-dlp");
            ytdlp_cmd.args([
                "--ignore-config",
                "-f", "bestaudio[acodec=opus]/bestaudio[ext=webm]/bestaudio/best",
                "-o", "-",
                "--no-playlist",
                "--no-check-certificate",
                "--geo-bypass",
                "--force-ipv4",
                // Acelerar el arranque: no hacer HEAD requests para verificar
                // formatos (ya elegimos uno concreto con -f).
                "--no-check-formats",
                "--extractor-args", &pot_arg,
                "--quiet",
            ]);
            if let Some(ref c) = cookies_path {
                ytdlp_cmd.args(["--cookies", c]);
            }
            ytdlp_cmd.arg(&url);
            ytdlp_cmd.stdout(Stdio::piped()).stderr(Stdio::null());

            let mut ytdlp = ytdlp_cmd.spawn()
                .map_err(|e| anyhow::anyhow!("no se pudo lanzar yt-dlp: {}", e))?;
            let ytdlp_stdout = ytdlp.stdout.take()
                .ok_or_else(|| anyhow::anyhow!("yt-dlp sin stdout"))?;

            // 2) ffmpeg aplica filtros (loudnorm + EQ) y produce WAV a stdout
            let mut ffmpeg_cmd = Command::new("ffmpeg");
            ffmpeg_cmd.args([
                "-i", "pipe:0",
                "-af", &filter,
                "-ac", "2",
                "-ar", "48000",
                "-f", "wav",
                "pipe:1",
            ]);
            ffmpeg_cmd
                .stdin(Stdio::from(ytdlp_stdout))
                .stdout(Stdio::piped())
                .stderr(Stdio::null());

            let ffmpeg = ffmpeg_cmd.spawn()
                .map_err(|e| anyhow::anyhow!("no se pudo lanzar ffmpeg: {}", e))?;

            // ChildContainer lee del último proceso (ffmpeg); mantiene yt-dlp vivo
            // y lo limpia al hacer drop.
            let container = songbird::input::ChildContainer::new(vec![ytdlp, ffmpeg]);
            Ok(Input::from(container))
        })
        .await??;

        info!("🎚️ Input con efectos (ffmpeg) creado para: {}", title);
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