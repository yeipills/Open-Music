use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;
use tracing::{info, debug, error};
use serenity::model::id::UserId;
use super::{MusicSource, TrackSource, SourceType};

#[derive(Debug, Deserialize)]
struct YouTubeAPIResponse {
    items: Vec<YouTubeVideo>,
}

#[derive(Debug, Deserialize)]
struct YouTubeVideo {
    id: VideoId,
    snippet: VideoSnippet,
}

#[derive(Debug, Deserialize)]
struct VideoId {
    videoId: String,
}

#[derive(Debug, Deserialize)]
struct VideoSnippet {
    title: String,
    channelTitle: String,
    thumbnails: Thumbnails,
    #[allow(dead_code)]
    publishedAt: String,
}

#[derive(Debug, Deserialize)]
struct Thumbnails {
    medium: Option<Thumbnail>,
    high: Option<Thumbnail>,
}

#[derive(Debug, Deserialize, Clone)]
struct Thumbnail {
    url: String,
}

#[derive(Debug, Deserialize)]
struct VideoDetailsResponse {
    items: Vec<VideoDetails>,
}

#[derive(Debug, Deserialize)]
struct VideoDetails {
    snippet: VideoSnippet,
    contentDetails: ContentDetails,
}

#[derive(Debug, Deserialize)]
struct ContentDetails {
    duration: String,
}

pub struct YouTubeAPIv3Client {
    api_key: String,
    client: reqwest::Client,
}

impl YouTubeAPIv3Client {
    pub fn new(api_key: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to create HTTP client");

        Self { api_key, client }
    }

    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<TrackSource>> {
        debug!("🔍 Búsqueda YouTube API v3: {}", query);

        let url = "https://www.googleapis.com/youtube/v3/search";
        
        let response = self.client
            .get(url)
            .query(&[
                ("part", "snippet"),
                ("q", query),
                ("type", "video"),
                ("maxResults", &limit.to_string()),
                ("key", &self.api_key),
                ("videoEmbeddable", "true"),
                ("videoSyndicated", "true"),
                ("order", "relevance"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!("❌ YouTube API error: {} - {}", status, error_text);
            anyhow::bail!("YouTube API error: {} - {}", status, error_text);
        }

        let api_response: YouTubeAPIResponse = response.json().await?;
        
        let tracks: Vec<TrackSource> = api_response.items
            .into_iter()
            .map(|video| {
                let url = format!("https://www.youtube.com/watch?v={}", video.id.videoId);
                let thumbnail = video.snippet.thumbnails.high.clone()
                    .or(video.snippet.thumbnails.medium.clone())
                    .map(|t| t.url);

                let mut track = TrackSource::new(
                    video.snippet.title,
                    url,
                    SourceType::YouTube,
                    UserId::default(),
                );

                track = track.with_artist(video.snippet.channelTitle);
                
                if let Some(thumb) = thumbnail {
                    track = track.with_thumbnail(thumb);
                }

                track
            })
            .collect();

        info!("✅ YouTube API v3: {} resultados", tracks.len());
        Ok(tracks)
    }

    pub async fn get_track(&self, url: &str) -> Result<TrackSource> {
        let video_id = Self::extract_video_id(url)?;
        
        let url = "https://www.googleapis.com/youtube/v3/videos";
        
        let response = self.client
            .get(url)
            .query(&[
                ("part", "snippet,contentDetails"),
                ("id", &video_id),
                ("key", &self.api_key),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("YouTube API error: {}", response.status());
        }

        let api_response: VideoDetailsResponse = response.json().await?;
        
        if api_response.items.is_empty() {
            anyhow::bail!("Video no encontrado");
        }

        let video = &api_response.items[0];
        let snippet = &video.snippet;
        
        let url = format!("https://www.youtube.com/watch?v={}", video_id);
        let thumbnail = snippet.thumbnails.high.clone()
            .or(snippet.thumbnails.medium.clone())
            .map(|t| t.url);

        let mut track = TrackSource::new(
            snippet.title.clone(),
            url,
            SourceType::YouTube,
            UserId::default(),
        );

        track = track.with_artist(snippet.channelTitle.clone());
        
        if let Some(thumb) = thumbnail {
            track = track.with_thumbnail(thumb);
        }

        // Parsear duración ISO 8601
        if let Ok(duration) = Self::parse_duration(&video.contentDetails.duration) {
            track = track.with_duration(duration);
        }

        Ok(track)
    }

    pub async fn get_playlist(&self, url: &str) -> Result<Vec<TrackSource>> {
        let playlist_id = Self::extract_playlist_id(url)?;
        
        let url = "https://www.googleapis.com/youtube/v3/playlistItems";
        
        let response = self.client
            .get(url)
            .query(&[
                ("part", "snippet"),
                ("playlistId", &playlist_id),
                ("maxResults", "50"),
                ("key", &self.api_key),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("YouTube API error: {}", response.status());
        }

        #[derive(Deserialize)]
        struct PlaylistResponse {
            items: Vec<PlaylistItem>,
        }

        #[derive(Deserialize)]
        struct PlaylistItem {
            snippet: PlaylistSnippet,
        }

        #[derive(Deserialize)]
        struct PlaylistSnippet {
            title: String,
            videoOwnerChannelTitle: String,
            thumbnails: Thumbnails,
            resourceId: ResourceId,
        }

        #[derive(Deserialize)]
        struct ResourceId {
            videoId: String,
        }

        let api_response: PlaylistResponse = response.json().await?;
        
        let tracks: Vec<TrackSource> = api_response.items
            .into_iter()
            .map(|item| {
                let url = format!("https://www.youtube.com/watch?v={}", item.snippet.resourceId.videoId);
                let thumbnail = item.snippet.thumbnails.high.clone()
                    .or(item.snippet.thumbnails.medium.clone())
                    .map(|t| t.url);

                let mut track = TrackSource::new(
                    item.snippet.title,
                    url,
                    SourceType::YouTube,
                    UserId::default(),
                );

                track = track.with_artist(item.snippet.videoOwnerChannelTitle);
                
                if let Some(thumb) = thumbnail {
                    track = track.with_thumbnail(thumb);
                }

                track
            })
            .collect();

        Ok(tracks)
    }

    fn extract_video_id(url: &str) -> Result<String> {
        if url.contains("youtube.com/watch") {
            if let Some(v) = url.split("v=").nth(1) {
                if let Some(video_id) = v.split('&').next() {
                    return Ok(video_id.to_string());
                }
            }
        } else if url.contains("youtu.be/") {
            if let Some(video_id) = url.split("youtu.be/").nth(1) {
                if let Some(video_id) = video_id.split('?').next() {
                    return Ok(video_id.to_string());
                }
            }
        }
        
        anyhow::bail!("No se pudo extraer video ID de la URL: {}", url)
    }

    fn extract_playlist_id(url: &str) -> Result<String> {
        if url.contains("youtube.com/playlist") {
            if let Some(list) = url.split("list=").nth(1) {
                if let Some(playlist_id) = list.split('&').next() {
                    return Ok(playlist_id.to_string());
                }
            }
        }
        
        anyhow::bail!("No se pudo extraer playlist ID de la URL: {}", url)
    }

    fn parse_duration(duration: &str) -> Result<Duration> {
        // Parsear duración ISO 8601 (PT1H2M3S)
        let mut hours = 0;
        let mut minutes = 0;
        let mut seconds = 0;
        
        let mut current_num = String::new();
        
        for ch in duration.chars() {
            match ch {
                'P' | 'T' => continue,
                'H' => {
                    hours = current_num.parse().unwrap_or(0);
                    current_num.clear();
                }
                'M' => {
                    minutes = current_num.parse().unwrap_or(0);
                    current_num.clear();
                }
                'S' => {
                    seconds = current_num.parse().unwrap_or(0);
                    current_num.clear();
                }
                _ if ch.is_ascii_digit() => {
                    current_num.push(ch);
                }
                _ => continue,
            }
        }
        
        Ok(Duration::from_secs(hours * 3600 + minutes * 60 + seconds))
    }
}

#[async_trait]
impl MusicSource for YouTubeAPIv3Client {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<TrackSource>> {
        self.search(query, limit).await
    }

    async fn get_track(&self, url: &str) -> Result<TrackSource> {
        self.get_track(url).await
    }

    async fn get_playlist(&self, url: &str) -> Result<Vec<TrackSource>> {
        self.get_playlist(url).await
    }

    fn is_valid_url(&self, url: &str) -> bool {
        url.contains("youtube.com") || url.contains("youtu.be")
    }

    fn source_name(&self) -> &'static str {
        "YouTube API v3"
    }
} 