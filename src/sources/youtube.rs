use anyhow::{Context, Result};
use async_process::Command;
use regex::Regex;
use serde::Deserialize;
use songbird::input::Input;
use std::time::Duration;
use tracing::{debug, info, warn};

use super::{MusicSource, SourceType, TrackSource};
use serenity::model::id::UserId;

/// Información de metadata de track
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TrackMetadata {
    pub title: String,
    pub artist: Option<String>,
    pub duration: Option<Duration>,
    pub thumbnail: Option<String>,
    pub url: Option<String>,
    pub source_type: SourceType,
    pub is_live: bool,
    pub stream_url: Option<String>,
}

/// Cliente para interactuar con YouTube/yt-dlp
pub struct YouTubeClient {
    #[allow(dead_code)]
    rate_limiter: Option<tokio::sync::Semaphore>,
}

/// Información extraída de yt-dlp
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct YtDlpInfo {
    pub id: String,
    pub title: String,
    pub duration: Option<f64>,
    pub uploader: Option<String>,
    pub thumbnail: Option<String>,
    pub webpage_url: String,
    pub formats: Option<Vec<Format>>,
    pub is_live: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Format {
    format_id: String,
    url: String,
    acodec: Option<String>,
    abr: Option<f64>,
}

impl YouTubeClient {
    pub fn new() -> Self {
        Self {
            // Limitar requests concurrentes para evitar rate limiting
            rate_limiter: Some(tokio::sync::Semaphore::new(10)),
        }
    }

    /// Busca videos en YouTube
    #[allow(dead_code)]
    pub async fn search_metadata(&self, query: &str, limit: usize) -> Result<Vec<TrackMetadata>> {
        let _permit = self.rate_limiter.as_ref().unwrap().acquire().await?;

        info!("🔍 Buscando en YouTube: {}", query);

        let search_query = format!("ytsearch{}:{}", limit, query);
        
        let output = Command::new("yt-dlp")
            .args(&[
                "--no-playlist",
                "--dump-json",
                "--flat-playlist",
                "--skip-download",
                "--no-warnings",
                &search_query,
            ])
            .output()
            .await
            .context("Error al ejecutar yt-dlp")?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("yt-dlp error: {}", error);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        for line in stdout.lines() {
            if let Ok(info) = serde_json::from_str::<YtDlpInfo>(line) {
                results.push(self.info_to_metadata(info));
            }
        }

        Ok(results)
    }

    /// Obtiene información de una URL específica
    #[allow(dead_code)]
    pub async fn get_info(&self, url: &str) -> Result<TrackMetadata> {
        let _permit = self.rate_limiter.as_ref().unwrap().acquire().await?;

        debug!("📊 Obteniendo info de: {}", url);

        let output = Command::new("yt-dlp")
            .args(&[
                "--no-playlist", 
                "--dump-json", 
                "--no-warnings",
                "--user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                "--extractor-args", "youtube:player_client=android,web",
                url
            ])
            .output()
            .await
            .context("Error al ejecutar yt-dlp")?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("yt-dlp error: {}", error);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let info: YtDlpInfo =
            serde_json::from_str(&stdout).context("Error al parsear respuesta de yt-dlp")?;

        Ok(self.info_to_metadata(info))
    }

    /// Obtiene la URL de streaming de audio
    #[allow(dead_code)]
    pub async fn get_stream_url(&self, url: &str) -> Result<String> {
        let _permit = self.rate_limiter.as_ref().unwrap().acquire().await?;

        debug!("🎵 Obteniendo URL de stream para: {}", url);

        let output = Command::new("yt-dlp")
            .args(&[
                "--no-playlist",
                "-f",
                "bestaudio/best",
                "--get-url",
                "--no-warnings",
                "--user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                "--extractor-args", "youtube:player_client=android,web",
                url,
            ])
            .output()
            .await
            .context("Error al ejecutar yt-dlp")?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("yt-dlp error: {}", error);
        }

        let stream_url = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if stream_url.is_empty() {
            anyhow::bail!("No se pudo obtener URL de stream");
        }

        Ok(stream_url)
    }

    /// Crea un Input de Songbird para reproducción (Songbird 0.5.0)
    #[allow(dead_code)]
    pub async fn create_input(&self, url: &str) -> Result<Input> {
        // Songbird 0.5.0: usar lazy input creation
        use songbird::input::YoutubeDl;
        
        let client = reqwest::Client::new();
        // YoutubeDl ahora es lazy por defecto
        let input = YoutubeDl::new(client, url.to_string());
        
        Ok(input.into())
    }

    /// Obtiene información de una playlist
    #[allow(dead_code)]
    pub async fn get_playlist_info(
        &self,
        url: &str,
        max_items: usize,
    ) -> Result<Vec<TrackMetadata>> {
        let _permit = self.rate_limiter.as_ref().unwrap().acquire().await?;

        info!("📋 Obteniendo playlist: {}", url);

        let output = Command::new("yt-dlp")
            .args(&[
                "--flat-playlist",
                "--dump-json",
                "--playlist-end",
                &max_items.to_string(),
                "--no-warnings",
                url,
            ])
            .output()
            .await
            .context("Error al ejecutar yt-dlp")?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("yt-dlp error: {}", error);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut tracks = Vec::new();

        for line in stdout.lines() {
            if let Ok(info) = serde_json::from_str::<YtDlpInfo>(line) {
                tracks.push(self.info_to_metadata(info));
            }
        }

        Ok(tracks)
    }

    /// Extrae metadata de una URL
    #[allow(dead_code)]
    pub async fn extract_metadata(&self, url: &str) -> Result<TrackMetadata> {
        let info = self.get_info(url).await?;
        Ok(info)
    }

    /// Verifica si una URL es válida para YouTube
    pub fn is_youtube_url(url: &str) -> bool {
        let youtube_regex = Regex::new(
            r"^(https?://)?(www\.)?(youtube\.com/(watch\?v=|embed/|v/|playlist\?list=)|youtu\.be/|music\.youtube\.com/)"
        ).unwrap();

        youtube_regex.is_match(url)
    }

    /// Verifica si una URL es una playlist de YouTube
    pub fn is_youtube_playlist(url: &str) -> bool {
        let playlist_regex = Regex::new(
            r"^(https?://)?(www\.)?(youtube\.com/playlist\?list=|music\.youtube\.com/playlist\?list=)"
        ).unwrap();

        playlist_regex.is_match(url) || url.contains("&list=") || url.contains("?list=")
    }

    /// Convierte YtDlpInfo a TrackMetadata
    fn info_to_metadata(&self, info: YtDlpInfo) -> TrackMetadata {
        TrackMetadata {
            title: info.title,
            artist: info.uploader,
            duration: info.duration.map(|d| Duration::from_secs_f64(d)),
            thumbnail: info.thumbnail,
            url: Some(info.webpage_url),
            source_type: SourceType::YouTube,
            is_live: info.is_live.unwrap_or(false),
            stream_url: None,
        }
    }

    /// Convierte YtDlpInfo a TrackSource
    #[allow(dead_code)]
    fn info_to_track_source(&self, info: YtDlpInfo, requested_by: UserId) -> TrackSource {
        let mut track = TrackSource::new(
            info.title,
            info.webpage_url,
            SourceType::YouTube,
            requested_by,
        );

        if let Some(artist) = info.uploader {
            track = track.with_artist(artist);
        }

        if let Some(duration) = info.duration {
            track = track.with_duration(Duration::from_secs_f64(duration));
        }

        if let Some(thumbnail) = info.thumbnail {
            track = track.with_thumbnail(thumbnail);
        }

        track
    }

    /// Búsqueda rápida y optimizada
    pub async fn search_detailed(&self, query: &str, _limit: usize) -> Result<Vec<TrackMetadata>> {
        let _permit = self.rate_limiter.as_ref().unwrap().acquire().await?;

        info!("⚡ Búsqueda rápida en YouTube: {}", query);

        // Limitar a 3 resultados para mayor velocidad
        let search_query = format!("ytsearch3:{}", query);
        
        let output = Command::new("yt-dlp")
            .args(&[
                "--dump-json",
                "--no-warnings", 
                "--no-playlist",
                "--simulate",
                "--socket-timeout", "8",
                "--retries", "1",
                "--no-cache-dir",
                "--user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                "--extractor-args", "youtube:player_client=android,web",
                &search_query,
            ])
            .output()
            .await
            .context("Error al ejecutar yt-dlp")?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("yt-dlp error: {}", error);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        if stdout.trim().is_empty() {
            info!("⚠️ yt-dlp devolvió resultado vacío para: {}", query);
            anyhow::bail!("No se encontraron resultados para: {}", query);
        }

        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }
            
            match serde_json::from_str::<YtDlpInfo>(line) {
                Ok(info) => {
                    results.push(self.info_to_metadata(info));
                    if results.len() >= 3 {
                        break; // Limitar a 3 resultados
                    }
                }
                Err(e) => {
                    tracing::debug!("Error parseando línea JSON: {} - línea: {}", e, line);
                    continue;
                }
            }
        }

        if results.is_empty() {
            anyhow::bail!("No se pudieron parsear resultados válidos");
        }

        info!("✅ Encontrados {} resultados válidos", results.len());
        Ok(results)
    }

    /// Filtrado rápido y simple
    pub fn filter_results(&self, results: Vec<TrackMetadata>, query: &str) -> Vec<TrackMetadata> {
        let mut filtered = results;
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();
        
        info!("⚡ Filtrado rápido de {} resultados", filtered.len());
        
        // Filtro básico y rápido - solo duración válida
        filtered.retain(|track| {
            if let Some(duration) = track.duration {
                let minutes = duration.as_secs() / 60;
                minutes >= 1 && minutes <= 60 // Entre 1 minuto y 1 hora (más restrictivo = más rápido)
            } else {
                !track.is_live // Excluir streams en vivo
            }
        });
        
        // Ordenamiento rápido - solo por palabras coincidentes
        filtered.sort_by(|a, b| {
            let a_title = a.title.to_lowercase();
            let b_title = b.title.to_lowercase();
            
            // Simple scoring: contar palabras que coinciden
            let a_matches = query_words.iter().filter(|&&word| a_title.contains(word)).count();
            let b_matches = query_words.iter().filter(|&&word| b_title.contains(word)).count();
            
            b_matches.cmp(&a_matches)
        });
        
        // Log del mejor resultado
        if let Some(best) = filtered.first() {
            let duration_str = if let Some(dur) = best.duration {
                format!("{:.1}min", dur.as_secs() as f64 / 60.0)
            } else {
                "Live".to_string()
            };
            let artist_str = best.artist.as_ref().map(|a| format!(" by {}", a)).unwrap_or_default();
            info!("  🎯 Mejor resultado: {} {} [{}]", best.title, artist_str, duration_str);
        }
        
        filtered
    }

    /// Actualiza yt-dlp (debe ejecutarse periódicamente)
    #[allow(dead_code)]
    pub async fn update_ytdlp() -> Result<()> {
        info!("🔄 Actualizando yt-dlp...");

        let output = Command::new("yt-dlp").arg("-U").output().await?;

        if output.status.success() {
            info!("✅ yt-dlp actualizado exitosamente");
        } else {
            warn!(
                "⚠️ No se pudo actualizar yt-dlp: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }
}


#[async_trait::async_trait]
impl MusicSource for YouTubeClient {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<TrackSource>> {
        let metadata = self.search_metadata(query, limit).await?;
        let tracks = metadata
            .into_iter()
            .map(|meta| {
                let mut track = TrackSource::new(
                    meta.title,
                    meta.url.unwrap_or_default(),
                    SourceType::YouTube,
                    UserId::default(), // This should be set by the caller
                );

                if let Some(artist) = meta.artist {
                    track = track.with_artist(artist);
                }

                if let Some(duration) = meta.duration {
                    track = track.with_duration(duration);
                }

                if let Some(thumbnail) = meta.thumbnail {
                    track = track.with_thumbnail(thumbnail);
                }

                track
            })
            .collect();

        Ok(tracks)
    }

    async fn get_track(&self, url: &str) -> Result<TrackSource> {
        let metadata = self.get_info(url).await?;
        let mut track = TrackSource::new(
            metadata.title,
            metadata.url.unwrap_or_else(|| url.to_string()),
            SourceType::YouTube,
            UserId::default(), // This should be set by the caller
        );

        if let Some(artist) = metadata.artist {
            track = track.with_artist(artist);
        }

        if let Some(duration) = metadata.duration {
            track = track.with_duration(duration);
        }

        if let Some(thumbnail) = metadata.thumbnail {
            track = track.with_thumbnail(thumbnail);
        }

        Ok(track)
    }

    async fn get_playlist(&self, url: &str) -> Result<Vec<TrackSource>> {
        let metadata_list = self.get_playlist_info(url, 50).await?;
        let tracks = metadata_list
            .into_iter()
            .map(|meta| {
                let mut track = TrackSource::new(
                    meta.title,
                    meta.url.unwrap_or_default(),
                    SourceType::YouTube,
                    UserId::default(), // This should be set by the caller
                );

                if let Some(artist) = meta.artist {
                    track = track.with_artist(artist);
                }

                if let Some(duration) = meta.duration {
                    track = track.with_duration(duration);
                }

                if let Some(thumbnail) = meta.thumbnail {
                    track = track.with_thumbnail(thumbnail);
                }

                track
            })
            .collect();

        Ok(tracks)
    }

    fn is_valid_url(&self, url: &str) -> bool {
        Self::is_youtube_url(url)
    }

    fn source_name(&self) -> &'static str {
        "YouTube"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_youtube_url_detection() {
        assert!(YouTubeClient::is_youtube_url(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        ));
        assert!(YouTubeClient::is_youtube_url(
            "https://youtu.be/dQw4w9WgXcQ"
        ));
        assert!(YouTubeClient::is_youtube_url(
            "https://music.youtube.com/watch?v=test"
        ));
        assert!(YouTubeClient::is_youtube_url(
            "https://www.youtube.com/playlist?list=PLtest"
        ));
        assert!(!YouTubeClient::is_youtube_url("https://example.com/video"));
    }

    #[test]
    fn test_youtube_playlist_detection() {
        // URLs de playlist explícitas
        assert!(YouTubeClient::is_youtube_playlist(
            "https://www.youtube.com/playlist?list=PLtest123"
        ));
        assert!(YouTubeClient::is_youtube_playlist(
            "https://music.youtube.com/playlist?list=PLtest123"
        ));
        
        // URLs con parámetro list
        assert!(YouTubeClient::is_youtube_playlist(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLtest123"
        ));
        assert!(YouTubeClient::is_youtube_playlist(
            "https://www.youtube.com/watch?list=PLtest123&v=dQw4w9WgXcQ"
        ));
        
        // URLs que NO son playlists
        assert!(!YouTubeClient::is_youtube_playlist(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        ));
        assert!(!YouTubeClient::is_youtube_playlist(
            "https://youtu.be/dQw4w9WgXcQ"
        ));
        assert!(!YouTubeClient::is_youtube_playlist(
            "https://example.com/playlist"
        ));
    }
}
