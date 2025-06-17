// Implementación básica del cliente de Spotify
// TODO: Implementar completamente cuando se compile el proyecto

use super::{MusicSource, TrackSource};
use anyhow::Result;
use async_trait::async_trait;

pub struct SpotifyClient {
    client_id: String,
    client_secret: String,
}

impl SpotifyClient {
    pub fn new(client_id: String, client_secret: String) -> Self {
        Self {
            client_id,
            client_secret,
        }
    }
}

#[async_trait]
impl MusicSource for SpotifyClient {
    async fn search(&self, _query: &str, _limit: usize) -> Result<Vec<TrackSource>> {
        anyhow::bail!("Spotify search not implemented yet")
    }

    async fn get_track(&self, _url: &str) -> Result<TrackSource> {
        anyhow::bail!("Spotify track not implemented yet")
    }

    async fn get_playlist(&self, _url: &str) -> Result<Vec<TrackSource>> {
        anyhow::bail!("Spotify playlist not implemented yet")
    }

    fn is_valid_url(&self, url: &str) -> bool {
        url.contains("open.spotify.com")
    }

    fn source_name(&self) -> &'static str {
        "spotify"
    }
}
