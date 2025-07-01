use anyhow::{Context, Result};
use serde::Deserialize;
use std::time::Duration;
use tracing::{info, warn};

use super::{MusicSource, SourceType, TrackSource};
use serenity::model::id::UserId;

/// Cliente alternativo usando YouTube RSS feeds (no requiere cookies)
pub struct YouTubeRssClient {
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct RssChannel {
    item: Vec<RssItem>,
}

#[derive(Debug, Deserialize)]
struct RssItem {
    title: String,
    link: String,
    #[serde(rename = "pubDate")]
    pub_date: Option<String>,
}

impl YouTubeRssClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// Búsqueda usando canales populares de YouTube RSS
    pub async fn search_rss(&self, query: &str, limit: usize) -> Result<Vec<TrackSource>> {
        info!("🔍 Buscando usando YouTube RSS: {}", query);
        
        // Lista de canales populares que suelen tener música
        let music_channels = vec![
            "UC-9-kyTW8ZkZNDHQJ6FgpwQ", // Music
            "UCEKXhDlL2unOr1s1hfwVeFg", // Popular Music
            "UCoUM-UJ7rirJYP8CQ0EIaHA", // Various Artists
        ];

        let mut all_results = Vec::new();
        
        for channel_id in music_channels {
            match self.search_channel_rss(channel_id, query, limit).await {
                Ok(mut results) => {
                    all_results.append(&mut results);
                    if all_results.len() >= limit {
                        break;
                    }
                }
                Err(e) => {
                    warn!("⚠️ Error buscando en canal {}: {}", channel_id, e);
                    continue;
                }
            }
        }

        all_results.truncate(limit);
        info!("📺 RSS search encontró {} resultados", all_results.len());
        Ok(all_results)
    }

    async fn search_channel_rss(&self, channel_id: &str, query: &str, limit: usize) -> Result<Vec<TrackSource>> {
        let rss_url = format!("https://www.youtube.com/feeds/videos.xml?channel_id={}", channel_id);
        
        let response = self.client
            .get(&rss_url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .context("Error fetching RSS feed")?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP error: {}", response.status());
        }

        let xml_content = response.text().await.context("Error reading RSS content")?;
        
        // Parse RSS XML y filtrar por query
        let mut results = Vec::new();
        let query_lower = query.to_lowercase();
        
        // Extraer video URLs usando regex simple
        use regex::Regex;
        let link_regex = Regex::new(r#"<link[^>]*href="([^"]*watch\?v=([^"&]+))"#)?;
        let title_regex = Regex::new(r#"<title><!\[CDATA\[([^\]]+)\]\]></title>"#)?;
        
        let mut links: Vec<String> = Vec::new();
        let mut titles: Vec<String> = Vec::new();
        
        // Extraer enlaces
        for cap in link_regex.captures_iter(&xml_content) {
            if let Some(url) = cap.get(1) {
                links.push(url.as_str().to_string());
            }
        }
        
        // Extraer títulos
        for cap in title_regex.captures_iter(&xml_content) {
            if let Some(title) = cap.get(1) {
                titles.push(title.as_str().to_string());
            }
        }
        
        // Combinar y filtrar
        for (title, link) in titles.iter().zip(links.iter()) {
            if title.to_lowercase().contains(&query_lower) {
                let track = TrackSource::new(
                    title.clone(),
                    link.clone(),
                    SourceType::YouTube,
                    UserId::default(),
                );
                
                results.push(track);
                
                if results.len() >= limit {
                    break;
                }
            }
        }
        
        Ok(results)
    }
}

#[async_trait::async_trait]
impl MusicSource for YouTubeRssClient {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<TrackSource>> {
        self.search_rss(query, limit).await
    }

    async fn get_track(&self, url: &str) -> Result<TrackSource> {
        // Para RSS, simplemente crear un track básico
        let track = TrackSource::new(
            "RSS Track".to_string(),
            url.to_string(),
            SourceType::YouTube,
            UserId::default(),
        );
        Ok(track)
    }

    async fn get_playlist(&self, _url: &str) -> Result<Vec<TrackSource>> {
        warn!("Playlists no soportadas en cliente RSS");
        Ok(Vec::new())
    }

    fn is_valid_url(&self, url: &str) -> bool {
        url.contains("youtube.com") || url.contains("youtu.be")
    }

    fn source_name(&self) -> &'static str {
        "YouTube RSS"
    }
}