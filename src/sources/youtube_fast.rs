use anyhow::{Context, Result};
use serde::Deserialize;
use std::time::Duration;
use tracing::{info, warn};
use regex::Regex;

use super::{MusicSource, SourceType, TrackSource};
use super::youtube::TrackMetadata;
use serenity::model::id::UserId;

/// Cliente rápido usando búsqueda web de YouTube
pub struct YouTubeFastClient {
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct YouTubeSearchResult {
    #[serde(rename = "videoId")]
    video_id: String,
    title: String,
    #[serde(rename = "lengthText")]
    length_text: Option<LengthText>,
    #[serde(rename = "ownerText")]
    owner_text: Option<OwnerText>,
    #[serde(rename = "thumbnails")]
    thumbnails: Option<Vec<Thumbnail>>,
}

#[derive(Debug, Deserialize)]
struct LengthText {
    #[serde(rename = "simpleText")]
    simple_text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OwnerText {
    runs: Option<Vec<Run>>,
}

#[derive(Debug, Deserialize)]
struct Run {
    text: String,
}

#[derive(Debug, Deserialize)]
struct Thumbnail {
    url: String,
    width: Option<u32>,
    height: Option<u32>,
}

impl YouTubeFastClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// Búsqueda rápida usando scraping web de YouTube
    pub async fn search_fast(&self, query: &str, limit: usize) -> Result<Vec<TrackMetadata>> {
        info!("🚀 Búsqueda ultrarrápida en YouTube: {}", query);
        
        let search_url = format!(
            "https://www.youtube.com/results?search_query={}",
            urlencoding::encode(query)
        );

        let response = self.client
            .get(&search_url)
            .timeout(Duration::from_secs(8))
            .send()
            .await
            .context("Error en request a YouTube")?;

        let html = response.text().await.context("Error leyendo respuesta")?;
        
        // Extraer JSON de datos usando regex
        let video_data = self.extract_video_data(&html)?;
        let results = self.parse_video_results(video_data, limit).await?;
        
        info!("⚡ Búsqueda completada en <5s: {} resultados", results.len());
        Ok(results)
    }

    /// Extrae datos de video del HTML usando regex
    fn extract_video_data(&self, html: &str) -> Result<Vec<String>> {
        let mut video_ids = Vec::new();
        
        // Regex para extraer IDs de video
        let video_id_regex = Regex::new(r#""videoId":"([a-zA-Z0-9_-]{11})""#)?;
        let _title_regex = Regex::new(r#""title":\{"runs":\[\{"text":"([^"]+)""#)?;
        
        // Extraer hasta 5 video IDs únicos
        let mut seen_ids = std::collections::HashSet::new();
        for cap in video_id_regex.captures_iter(html) {
            if let Some(video_id) = cap.get(1) {
                let id = video_id.as_str().to_string();
                if seen_ids.insert(id.clone()) {
                    video_ids.push(id);
                    if video_ids.len() >= 3 {
                        break;
                    }
                }
            }
        }
        
        Ok(video_ids)
    }

    /// Convierte IDs de video a metadata
    async fn parse_video_results(&self, video_ids: Vec<String>, _limit: usize) -> Result<Vec<TrackMetadata>> {
        let mut results = Vec::new();
        
        for video_id in video_ids.into_iter().take(3) {
            if let Ok(metadata) = self.get_video_metadata(&video_id).await {
                results.push(metadata);
            }
        }
        
        Ok(results)
    }

    /// Obtiene metadata básica de un video por ID
    async fn get_video_metadata(&self, video_id: &str) -> Result<TrackMetadata> {
        let url = format!("https://www.youtube.com/watch?v={}", video_id);
        
        // Usar yt-dlp solo para obtener metadata básica (más rápido)
        let output = tokio::process::Command::new("yt-dlp")
            .args(&[
                "--dump-json",
                "--no-warnings",
                "--no-playlist", 
                "--simulate",
                "--socket-timeout", "3",
                "--user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                "--extractor-args", "youtube:player_client=android,web",
                &url,
            ])
            .output()
            .await
            .context("Error ejecutando yt-dlp para metadata")?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            warn!("yt-dlp falló para {}: {}", video_id, error);
            anyhow::bail!("No se pudo obtener información del video {}", video_id);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = stdout.lines().next() {
            if let Ok(info) = serde_json::from_str::<super::youtube::YtDlpInfo>(line) {
                return Ok(TrackMetadata {
                    title: info.title,
                    artist: info.uploader,
                    duration: info.duration.map(|d| Duration::from_secs_f64(d)),
                    thumbnail: info.thumbnail,
                    url: Some(info.webpage_url),
                    source_type: SourceType::YouTube,
                    is_live: info.is_live.unwrap_or(false),
                });
            }
        }

        // Si falla el parsing JSON, intentar con el cliente estándar
        warn!("No se pudo parsear JSON para video {}, intentando con método alternativo", video_id);
        anyhow::bail!("No se pudo parsear información del video {}", video_id)
    }
}

#[async_trait::async_trait]
impl MusicSource for YouTubeFastClient {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<TrackSource>> {
        let metadata = self.search_fast(query, limit).await?;
        let tracks = metadata
            .into_iter()
            .map(|meta| {
                let mut track = TrackSource::new(
                    meta.title,
                    meta.url.unwrap_or_default(),
                    SourceType::YouTube,
                    UserId::default(),
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
        // Extraer video ID de la URL
        let video_id = if let Some(captures) = Regex::new(r"(?:youtube\.com/watch\?v=|youtu\.be/)([a-zA-Z0-9_-]{11})")?.captures(url) {
            captures.get(1).unwrap().as_str()
        } else {
            anyhow::bail!("URL de YouTube inválida");
        };

        let metadata = self.get_video_metadata(video_id).await?;
        let mut track = TrackSource::new(
            metadata.title,
            metadata.url.unwrap_or_else(|| url.to_string()),
            SourceType::YouTube,
            UserId::default(),
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

    async fn get_playlist(&self, _url: &str) -> Result<Vec<TrackSource>> {
        warn!("Playlists no soportadas en cliente rápido");
        Ok(Vec::new())
    }

    fn is_valid_url(&self, url: &str) -> bool {
        url.contains("youtube.com") || url.contains("youtu.be")
    }

    fn source_name(&self) -> &'static str {
        "YouTube Fast"
    }
}