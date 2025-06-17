// Implementación básica del cliente de Tidal
// TODO: Implementar completamente cuando se compile el proyecto

use super::{MusicSource, TrackSource};
use anyhow::Result;
use async_trait::async_trait;

pub struct TidalClient {
    api_key: String,
}

impl TidalClient {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

#[async_trait]
impl MusicSource for TidalClient {
    async fn search(&self, _query: &str, _limit: usize) -> Result<Vec<TrackSource>> {
        anyhow::bail!("Tidal search not implemented yet")
    }

    async fn get_track(&self, _url: &str) -> Result<TrackSource> {
        anyhow::bail!("Tidal track not implemented yet")
    }

    async fn get_playlist(&self, _url: &str) -> Result<Vec<TrackSource>> {
        anyhow::bail!("Tidal playlist not implemented yet")
    }

    fn is_valid_url(&self, url: &str) -> bool {
        url.contains("tidal.com") || url.contains("listen.tidal.com")
    }

    fn source_name(&self) -> &'static str {
        "tidal"
    }
}
