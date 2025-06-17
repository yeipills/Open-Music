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
pub struct TrackMetadata {
    pub title: String,
    pub artist: Option<String>,
    pub duration: Option<Duration>,
    pub thumbnail: Option<String>,
    pub url: Option<String>,
    pub source_type: SourceType,
    pub is_live: bool,
}

/// Cliente para interactuar con YouTube/yt-dlp
pub struct YouTubeClient {
    rate_limiter: Option<tokio::sync::Semaphore>,
}

/// Información extraída de yt-dlp
#[derive(Debug, Deserialize)]
struct YtDlpInfo {
    id: String,
    title: String,
    duration: Option<f64>,
    uploader: Option<String>,
    thumbnail: Option<String>,
    webpage_url: String,
    formats: Option<Vec<Format>>,
    is_live: Option<bool>,
}

#[derive(Debug, Deserialize)]
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
            rate_limiter: Some(tokio::sync::Semaphore::new(3)),
        }
    }

    /// Busca videos en YouTube
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
    pub async fn get_info(&self, url: &str) -> Result<TrackMetadata> {
        let _permit = self.rate_limiter.as_ref().unwrap().acquire().await?;

        debug!("📊 Obteniendo info de: {}", url);

        let output = Command::new("yt-dlp")
            .args(&["--no-playlist", "--dump-json", "--no-warnings", url])
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

    /// Crea un Input de Songbird para reproducción
    pub async fn create_input(&self, url: &str) -> Result<Input> {
        // Usar YoutubeDl de songbird para manejo optimizado
        let client = reqwest::Client::new();
        let source = songbird::input::YoutubeDl::new(client, url.to_string());

        Ok(source.into())
    }

    /// Obtiene información de una playlist
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
    pub async fn extract_metadata(&self, url: &str) -> Result<TrackMetadata> {
        let info = self.get_info(url).await?;
        Ok(info)
    }

    /// Verifica si una URL es válida para YouTube
    pub fn is_youtube_url(url: &str) -> bool {
        let youtube_regex = Regex::new(
            r"^(https?://)?(www\.)?(youtube\.com/(watch\?v=|embed/|v/)|youtu\.be/|music\.youtube\.com/)"
        ).unwrap();

        youtube_regex.is_match(url)
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
        }
    }

    /// Convierte YtDlpInfo a TrackSource
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

    /// Busca videos con información detallada para selección
    pub async fn search_detailed(&self, query: &str, limit: usize) -> Result<Vec<TrackMetadata>> {
        let _permit = self.rate_limiter.as_ref().unwrap().acquire().await?;

        info!("🔍 Búsqueda detallada en YouTube: {}", query);

        let search_query = format!("ytsearch{}:{}", limit, query);
        
        let output = Command::new("yt-dlp")
            .args(&[
                "--no-playlist",
                "--dump-json",
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

    /// Filtra resultados por relevancia y calidad
    pub fn filter_results(&self, results: Vec<TrackMetadata>, query: &str) -> Vec<TrackMetadata> {
        let mut filtered = results;
        let query_lower = query.to_lowercase();
        
        // Ordenar por relevancia
        filtered.sort_by(|a, b| {
            let a_title = a.title.to_lowercase();
            let b_title = b.title.to_lowercase();
            
            // Priorizar coincidencias exactas
            let a_exact = a_title.contains(&query_lower);
            let b_exact = b_title.contains(&query_lower);
            
            match (a_exact, b_exact) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => {
                    // Si ambos tienen o no tienen coincidencia exacta, ordenar por duración
                    match (a.duration, b.duration) {
                        (Some(dur_a), Some(dur_b)) => {
                            // Preferir duraciones entre 1-10 minutos
                            let score_a = duration_score(dur_a);
                            let score_b = duration_score(dur_b);
                            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
                        }
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        _ => std::cmp::Ordering::Equal,
                    }
                }
            }
        });
        
        // Filtrar videos muy largos o muy cortos
        filtered.retain(|track| {
            if let Some(duration) = track.duration {
                let minutes = duration.as_secs() / 60;
                minutes >= 1 && minutes <= 600 // Entre 1 minuto y 10 horas
            } else {
                !track.is_live // Excluir streams en vivo por defecto
            }
        });
        
        filtered
    }

    /// Actualiza yt-dlp (debe ejecutarse periódicamente)
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

/// Calcula score de relevancia basado en duración
fn duration_score(duration: Duration) -> f64 {
    let minutes = duration.as_secs() as f64 / 60.0;
    
    if minutes < 1.0 {
        0.1 // Muy corto
    } else if minutes <= 3.0 {
        0.9 // Muy bueno para música
    } else if minutes <= 6.0 {
        1.0 // Perfecto para música
    } else if minutes <= 10.0 {
        0.8 // Aceptable
    } else if minutes <= 20.0 {
        0.6 // Posiblemente mix o podcast
    } else {
        0.3 // Muy largo, probablemente no es música
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
        assert!(!YouTubeClient::is_youtube_url("https://example.com/video"));
    }
}
