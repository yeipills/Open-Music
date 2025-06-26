// Implementación básica del cliente de URL directa
// TODO: Implementar completamente cuando se compile el proyecto

use super::{MusicSource, TrackSource};
use anyhow::Result;
use async_trait::async_trait;

pub struct DirectUrlClient {}

impl DirectUrlClient {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl MusicSource for DirectUrlClient {
    async fn search(&self, _query: &str, _limit: usize) -> Result<Vec<TrackSource>> {
        anyhow::bail!("Direct URL search not implemented yet")
    }

    async fn get_track(&self, _url: &str) -> Result<TrackSource> {
        anyhow::bail!("Direct URL track not implemented yet")
    }

    async fn get_playlist(&self, _url: &str) -> Result<Vec<TrackSource>> {
        anyhow::bail!("Direct URL playlist not implemented yet")
    }

    fn is_valid_url(&self, url: &str) -> bool {
        // Check for HTTP/HTTPS URLs
        if url.starts_with("http://") || url.starts_with("https://") {
            return true;
        }

        // Check for common audio file extensions
        let audio_extensions = [".mp3", ".wav", ".ogg", ".flac", ".m4a"];
        let url_lower = url.to_lowercase();

        audio_extensions.iter().any(|ext| url_lower.ends_with(ext))
    }

    fn source_name(&self) -> &'static str {
        "direct"
    }
}
