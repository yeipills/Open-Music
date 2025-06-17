// Implementación básica del cliente de SoundCloud
// TODO: Implementar completamente cuando se compile el proyecto

use super::{MusicSource, TrackSource};
use anyhow::Result;
use async_trait::async_trait;

pub struct SoundCloudClient {
    client_id: String,
}

impl SoundCloudClient {
    pub fn new(client_id: String) -> Self {
        Self { client_id }
    }
}

#[async_trait]
impl MusicSource for SoundCloudClient {
    async fn search(&self, _query: &str, _limit: usize) -> Result<Vec<TrackSource>> {
        anyhow::bail!("SoundCloud search not implemented yet")
    }

    async fn get_track(&self, _url: &str) -> Result<TrackSource> {
        anyhow::bail!("SoundCloud track not implemented yet")
    }

    async fn get_playlist(&self, _url: &str) -> Result<Vec<TrackSource>> {
        anyhow::bail!("SoundCloud playlist not implemented yet")
    }

    fn is_valid_url(&self, url: &str) -> bool {
        url.contains("soundcloud.com")
    }

    fn source_name(&self) -> &'static str {
        "soundcloud"
    }
}
