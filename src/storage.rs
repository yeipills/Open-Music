use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use tracing::{info, warn, error};
use chrono::{DateTime, Utc};

/// Configuración de servidor almacenada en JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub guild_id: u64,
    pub default_volume: f32,
    pub max_queue_size: usize,
    pub auto_leave_timeout: u64, // seconds
    pub dj_role_id: Option<u64>,
    pub announcement_channel_id: Option<u64>,
    pub auto_leave_empty: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            guild_id: 0,
            default_volume: 0.5,
            max_queue_size: 100,
            auto_leave_timeout: 300,
            dj_role_id: None,
            announcement_channel_id: None,
            auto_leave_empty: true,
        }
    }
}

/// Playlist personal del usuario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPlaylist {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: u64,
    pub guild_id: u64,
    pub tracks: Vec<PlaylistTrack>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_public: bool,
    pub is_favorite: bool,
    pub play_count: u32,
    pub tags: Vec<String>,
}

/// Canción dentro de una playlist personal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistTrack {
    pub title: String,
    pub artist: Option<String>,
    pub url: String,
    pub duration: Option<Duration>,
    pub thumbnail: Option<String>,
    pub added_by: u64,
    pub added_at: DateTime<Utc>,
    pub source_type: String, // "YouTube", "Spotify", etc.
}

/// Historial de playlists cargadas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistHistory {
    pub user_id: u64,
    pub guild_id: u64,
    pub recent_playlists: Vec<PlaylistHistoryEntry>,
    pub favorite_playlists: Vec<String>, // IDs de playlists favoritas
    pub total_playlists_loaded: u32,
    pub last_updated: DateTime<Utc>,
}

/// Entrada del historial de playlists
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistHistoryEntry {
    pub playlist_id: Option<String>, // Si es playlist personal
    pub playlist_url: Option<String>, // Si es playlist externa
    pub playlist_name: String,
    pub track_count: usize,
    pub loaded_at: DateTime<Utc>,
    pub source: String, // "YouTube", "Personal", etc.
}

impl UserPlaylist {
    #[allow(dead_code)]
    pub fn new(name: String, owner_id: u64, guild_id: u64) -> Self {
        let now = Utc::now();
        let id = format!("{}_{}_{}_{}", guild_id, owner_id, now.timestamp(), fastrand::u32(..));
        
        Self {
            id,
            name,
            description: None,
            owner_id,
            guild_id,
            tracks: Vec::new(),
            created_at: now,
            updated_at: now,
            is_public: false,
            is_favorite: false,
            play_count: 0,
            tags: Vec::new(),
        }
    }
    
    #[allow(dead_code)]
    pub fn add_track(&mut self, track: PlaylistTrack) {
        self.tracks.push(track);
        self.updated_at = Utc::now();
    }
    
    #[allow(dead_code)]
    pub fn remove_track(&mut self, index: usize) -> Option<PlaylistTrack> {
        if index < self.tracks.len() {
            self.updated_at = Utc::now();
            Some(self.tracks.remove(index))
        } else {
            None
        }
    }
    
    #[allow(dead_code)]
    pub fn total_duration(&self) -> Duration {
        self.tracks
            .iter()
            .filter_map(|track| track.duration)
            .sum()
    }
    
    #[allow(dead_code)]
    pub fn increment_play_count(&mut self) {
        self.play_count += 1;
        self.updated_at = Utc::now();
    }
}

impl PlaylistTrack {
    #[allow(dead_code)]
    pub fn from_track_source(track: &crate::sources::TrackSource, added_by: u64) -> Self {
        Self {
            title: track.title(),
            artist: track.artist(),
            url: track.url(),
            duration: track.duration(),
            thumbnail: track.thumbnail(),
            added_by,
            added_at: chrono::Utc::now(),
            source_type: format!("{:?}", track.source_type()),
        }
    }
}

/// Manager de almacenamiento basado en archivos JSON
pub struct JsonStorage {
    data_dir: PathBuf,
    servers_cache: HashMap<u64, ServerConfig>,
    #[allow(dead_code)]
    playlists_cache: HashMap<String, UserPlaylist>,
    #[allow(dead_code)]
    history_cache: HashMap<(u64, u64), PlaylistHistory>, // (user_id, guild_id)
}

impl JsonStorage {
    pub async fn new(data_dir: PathBuf) -> Result<Self> {
        // Crear directorio de datos si no existe
        fs::create_dir_all(&data_dir).await?;
        
        let servers_dir = data_dir.join("servers");
        fs::create_dir_all(&servers_dir).await?;
        
        let playlists_dir = data_dir.join("playlists");
        fs::create_dir_all(&playlists_dir).await?;
        
        let history_dir = data_dir.join("history");
        fs::create_dir_all(&history_dir).await?;
        
        info!("📁 Storage inicializado en: {}", data_dir.display());
        
        let mut storage = Self {
            data_dir,
            servers_cache: HashMap::new(),
            playlists_cache: HashMap::new(),
            history_cache: HashMap::new(),
        };
        
        // Cargar configuraciones existentes
        storage.load_all_servers().await?;
        
        Ok(storage)
    }
    
    /// Obtiene la configuración de un servidor
    #[allow(dead_code)]
    pub async fn get_server_config(&mut self, guild_id: u64) -> Result<ServerConfig> {
        // Verificar cache primero
        if let Some(config) = self.servers_cache.get(&guild_id) {
            return Ok(config.clone());
        }
        
        // Cargar desde archivo
        match self.load_server_config(guild_id).await {
            Ok(config) => {
                self.servers_cache.insert(guild_id, config.clone());
                Ok(config)
            }
            Err(_) => {
                // Crear configuración por defecto
                let mut config = ServerConfig::default();
                config.guild_id = guild_id;
                
                self.save_server_config(&config).await?;
                self.servers_cache.insert(guild_id, config.clone());
                
                info!("📝 Configuración por defecto creada para guild {}", guild_id);
                Ok(config)
            }
        }
    }
    
    /// Actualiza la configuración de un servidor
    #[allow(dead_code)]
    pub async fn update_server_config(&mut self, config: ServerConfig) -> Result<()> {
        let guild_id = config.guild_id;
        
        // Actualizar cache
        self.servers_cache.insert(guild_id, config.clone());
        
        // Guardar en archivo
        self.save_server_config(&config).await?;
        
        info!("💾 Configuración actualizada para guild {}", guild_id);
        Ok(())
    }
    
    /// Actualiza el volumen por defecto de un servidor
    #[allow(dead_code)]
    pub async fn set_default_volume(&mut self, guild_id: u64, volume: f32) -> Result<()> {
        let mut config = self.get_server_config(guild_id).await?;
        config.default_volume = volume.clamp(0.0, 2.0);
        self.update_server_config(config).await
    }
    
    /// Actualiza el tamaño máximo de cola de un servidor
    #[allow(dead_code)]
    pub async fn set_max_queue_size(&mut self, guild_id: u64, size: usize) -> Result<()> {
        let mut config = self.get_server_config(guild_id).await?;
        config.max_queue_size = size.min(1000); // Límite máximo de 1000
        self.update_server_config(config).await
    }
    
    /// Actualiza el rol de DJ de un servidor
    #[allow(dead_code)]
    pub async fn set_dj_role(&mut self, guild_id: u64, role_id: Option<u64>) -> Result<()> {
        let mut config = self.get_server_config(guild_id).await?;
        config.dj_role_id = role_id;
        self.update_server_config(config).await
    }
    
    /// Actualiza el canal de anuncios de un servidor
    #[allow(dead_code)]
    pub async fn set_announcement_channel(&mut self, guild_id: u64, channel_id: Option<u64>) -> Result<()> {
        let mut config = self.get_server_config(guild_id).await?;
        config.announcement_channel_id = channel_id;
        self.update_server_config(config).await
    }
    
    /// Lista todas las configuraciones de servidores
    #[allow(dead_code)]
    pub fn list_servers(&self) -> Vec<u64> {
        self.servers_cache.keys().copied().collect()
    }
    
    /// Obtiene estadísticas de almacenamiento
    #[allow(dead_code)]
    pub async fn get_storage_stats(&self) -> Result<StorageStats> {
        let servers_dir = self.data_dir.join("servers");
        let mut files = fs::read_dir(&servers_dir).await?;
        let mut file_count = 0;
        let mut total_size = 0;
        
        while let Some(entry) = files.next_entry().await? {
            if entry.path().extension().map_or(false, |ext| ext == "json") {
                file_count += 1;
                if let Ok(metadata) = entry.metadata().await {
                    total_size += metadata.len();
                }
            }
        }
        
        Ok(StorageStats {
            server_configs: file_count,
            cached_configs: self.servers_cache.len(),
            total_size_bytes: total_size,
            data_dir: self.data_dir.clone(),
        })
    }
    
    // Métodos privados
    
    async fn load_server_config(&self, guild_id: u64) -> Result<ServerConfig> {
        let file_path = self.get_server_file_path(guild_id);
        let content = fs::read_to_string(&file_path).await?;
        let config: ServerConfig = serde_json::from_str(&content)?;
        Ok(config)
    }
    
    #[allow(dead_code)]
    async fn save_server_config(&self, config: &ServerConfig) -> Result<()> {
        let file_path = self.get_server_file_path(config.guild_id);
        let content = serde_json::to_string_pretty(config)?;
        fs::write(&file_path, content).await?;
        Ok(())
    }
    
    async fn load_all_servers(&mut self) -> Result<()> {
        let servers_dir = self.data_dir.join("servers");
        
        if !servers_dir.exists() {
            return Ok(());
        }
        
        let mut files = fs::read_dir(&servers_dir).await?;
        let mut loaded_count = 0;
        
        while let Some(entry) = files.next_entry().await? {
            let path = entry.path();
            
            if path.extension().map_or(false, |ext| ext == "json") {
                if let Some(file_name) = path.file_stem().and_then(|n| n.to_str()) {
                    if let Some(guild_id_str) = file_name.strip_prefix("guild_") {
                        if let Ok(guild_id) = guild_id_str.parse::<u64>() {
                            match self.load_server_config(guild_id).await {
                                Ok(config) => {
                                    self.servers_cache.insert(guild_id, config);
                                    loaded_count += 1;
                                }
                                Err(e) => {
                                    warn!("Error cargando configuración para guild {}: {}", guild_id, e);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        if loaded_count > 0 {
            info!("📂 Cargadas {} configuraciones de servidor", loaded_count);
        }
        
        Ok(())
    }
    
    fn get_server_file_path(&self, guild_id: u64) -> PathBuf {
        self.data_dir.join("servers").join(format!("guild_{}.json", guild_id))
    }
}

/// Estadísticas de almacenamiento
#[derive(Debug)]
pub struct StorageStats {
    pub server_configs: usize,
    pub cached_configs: usize,
    pub total_size_bytes: u64,
    pub data_dir: PathBuf,
}

impl std::fmt::Display for StorageStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "📊 Storage Stats:\n\
             📁 Data Directory: {}\n\
             📝 Server Configs: {} files\n\
             💾 Cached Configs: {} in memory\n\
             📦 Total Size: {} bytes ({:.2} KB)",
            self.data_dir.display(),
            self.server_configs,
            self.cached_configs,
            self.total_size_bytes,
            self.total_size_bytes as f64 / 1024.0
        )
    }
}

/// Funciones de utilidad para migrar desde base de datos (si existiera)
impl JsonStorage {
    /// Crea una configuración de ejemplo para testing
    #[allow(dead_code)]
    pub async fn create_example_config(&mut self, guild_id: u64) -> Result<()> {
        let config = ServerConfig {
            guild_id,
            default_volume: 0.7,
            max_queue_size: 50,
            auto_leave_timeout: 600,
            dj_role_id: None,
            announcement_channel_id: None,
            auto_leave_empty: true,
        };
        
        self.update_server_config(config).await?;
        info!("📝 Configuración de ejemplo creada para guild {}", guild_id);
        Ok(())
    }
    
    /// Limpia configuraciones de servidores que ya no existen
    #[allow(dead_code)]
    pub async fn cleanup_old_configs(&mut self, active_guilds: &[u64]) -> Result<usize> {
        let servers_dir = self.data_dir.join("servers");
        let mut files = fs::read_dir(&servers_dir).await?;
        let mut removed_count = 0;
        
        while let Some(entry) = files.next_entry().await? {
            let path = entry.path();
            
            if let Some(file_name) = path.file_stem().and_then(|n| n.to_str()) {
                if let Some(guild_id_str) = file_name.strip_prefix("guild_") {
                    if let Ok(guild_id) = guild_id_str.parse::<u64>() {
                        if !active_guilds.contains(&guild_id) {
                            match fs::remove_file(&path).await {
                                Ok(_) => {
                                    self.servers_cache.remove(&guild_id);
                                    removed_count += 1;
                                    info!("🗑️ Configuración eliminada para guild inactiva: {}", guild_id);
                                }
                                Err(e) => {
                                    error!("Error eliminando configuración para guild {}: {}", guild_id, e);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(removed_count)
    }
    
    // === MÉTODOS PARA PLAYLISTS PERSONALES ===
    
    #[allow(dead_code)]
    pub async fn create_playlist(&mut self, name: String, owner_id: u64, guild_id: u64) -> Result<String> {
        let playlist = UserPlaylist::new(name, owner_id, guild_id);
        let playlist_id = playlist.id.clone();
        
        self.save_playlist(&playlist).await?;
        self.playlists_cache.insert(playlist_id.clone(), playlist);
        
        Ok(playlist_id)
    }

    #[allow(dead_code)]
    pub async fn get_playlist(&mut self, playlist_id: &str) -> Result<Option<UserPlaylist>> {
        // Intentar obtener del caché primero
        if let Some(playlist) = self.playlists_cache.get(playlist_id) {
            return Ok(Some(playlist.clone()));
        }
        
        // Cargar desde archivo
        match self.load_playlist(playlist_id).await {
            Ok(playlist) => {
                self.playlists_cache.insert(playlist_id.to_string(), playlist.clone());
                Ok(Some(playlist))
            }
            Err(_) => Ok(None)
        }
    }

    #[allow(dead_code)]
    pub async fn get_user_playlists(&mut self, user_id: u64, guild_id: u64) -> Result<Vec<UserPlaylist>> {
        let mut playlists = Vec::new();
        
        for playlist in self.playlists_cache.values() {
            if playlist.owner_id == user_id && playlist.guild_id == guild_id {
                playlists.push(playlist.clone());
            }
        }
        
        Ok(playlists)
    }

    #[allow(dead_code)]
    pub async fn get_public_playlists(&mut self, guild_id: u64) -> Result<Vec<UserPlaylist>> {
        let mut playlists = Vec::new();
        
        for playlist in self.playlists_cache.values() {
            if playlist.guild_id == guild_id && playlist.is_public {
                playlists.push(playlist.clone());
            }
        }
        
        Ok(playlists)
    }

    #[allow(dead_code)]
    pub async fn update_playlist(&mut self, playlist: UserPlaylist) -> Result<()> {
        self.save_playlist(&playlist).await?;
        self.playlists_cache.insert(playlist.id.clone(), playlist);
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn delete_playlist(&mut self, playlist_id: &str, user_id: u64) -> Result<bool> {
        if let Some(playlist) = self.playlists_cache.get(playlist_id) {
            if playlist.owner_id == user_id {
                let file_path = self.get_playlist_file_path(playlist_id);
                if file_path.exists() {
                    std::fs::remove_file(file_path)?;
                }
                self.playlists_cache.remove(playlist_id);
                return Ok(true);
            }
        }
        Ok(false)
    }
    
    #[allow(dead_code)]
    /// Añade una canción a una playlist
    pub async fn add_track_to_playlist(&mut self, playlist_id: &str, track: PlaylistTrack, user_id: u64) -> Result<bool> {
        if let Some(mut playlist) = self.get_playlist(playlist_id).await? {
            if playlist.owner_id != user_id {
                return Ok(false); // No autorizado
            }
            
            playlist.add_track(track);
            self.update_playlist(playlist).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    
    #[allow(dead_code)]
    /// Remueve una canción de una playlist
    pub async fn remove_track_from_playlist(&mut self, playlist_id: &str, track_index: usize, user_id: u64) -> Result<Option<PlaylistTrack>> {
        if let Some(mut playlist) = self.get_playlist(playlist_id).await? {
            if playlist.owner_id != user_id {
                return Ok(None); // No autorizado
            }
            
            if let Some(removed_track) = playlist.remove_track(track_index) {
                self.update_playlist(playlist).await?;
                Ok(Some(removed_track))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
    
    // === MÉTODOS PARA HISTORIAL DE PLAYLISTS ===
    
    #[allow(dead_code)]
    /// Añade una entrada al historial de playlists del usuario
    pub async fn add_to_playlist_history(&mut self, user_id: u64, guild_id: u64, entry: PlaylistHistoryEntry) -> Result<()> {
        let key = (user_id, guild_id);
        
        let mut history = match self.history_cache.get(&key) {
            Some(h) => h.clone(),
            None => match self.load_playlist_history(user_id, guild_id).await {
                Ok(h) => h,
                Err(_) => PlaylistHistory {
                    user_id,
                    guild_id,
                    recent_playlists: Vec::new(),
                    favorite_playlists: Vec::new(),
                    total_playlists_loaded: 0,
                    last_updated: Utc::now(),
                }
            }
        };
        
        // Añadir nueva entrada al principio
        history.recent_playlists.insert(0, entry);
        history.total_playlists_loaded += 1;
        history.last_updated = Utc::now();
        
        // Limitar a las últimas 50 playlists
        if history.recent_playlists.len() > 50 {
            history.recent_playlists.truncate(50);
        }
        
        // Actualizar cache y guardar
        self.history_cache.insert(key, history.clone());
        self.save_playlist_history(&history).await?;
        
        Ok(())
    }
    
    #[allow(dead_code)]
    /// Obtiene el historial de playlists de un usuario
    pub async fn get_playlist_history(&mut self, user_id: u64, guild_id: u64) -> Result<PlaylistHistory> {
        let key = (user_id, guild_id);
        
        if let Some(history) = self.history_cache.get(&key) {
            return Ok(history.clone());
        }
        
        match self.load_playlist_history(user_id, guild_id).await {
            Ok(history) => {
                self.history_cache.insert(key, history.clone());
                Ok(history)
            }
            Err(_) => {
                let history = PlaylistHistory {
                    user_id,
                    guild_id,
                    recent_playlists: Vec::new(),
                    favorite_playlists: Vec::new(),
                    total_playlists_loaded: 0,
                    last_updated: Utc::now(),
                };
                Ok(history)
            }
        }
    }
    
    #[allow(dead_code)]
    /// Marca/desmarca una playlist como favorita
    pub async fn toggle_favorite_playlist(&mut self, user_id: u64, guild_id: u64, playlist_id: String) -> Result<bool> {
        let key = (user_id, guild_id);
        let mut history = self.get_playlist_history(user_id, guild_id).await?;
        
        let is_favorited = if let Some(pos) = history.favorite_playlists.iter().position(|id| id == &playlist_id) {
            history.favorite_playlists.remove(pos);
            false
        } else {
            history.favorite_playlists.push(playlist_id);
            true
        };
        
        history.last_updated = Utc::now();
        self.history_cache.insert(key, history.clone());
        self.save_playlist_history(&history).await?;
        
        Ok(is_favorited)
    }
    
    // === MÉTODOS PRIVADOS PARA PLAYLISTS ===
    
    #[allow(dead_code)]
    async fn load_playlist(&self, playlist_id: &str) -> Result<UserPlaylist> {
        let file_path = self.get_playlist_file_path(playlist_id);
        let content = fs::read_to_string(&file_path).await?;
        let playlist: UserPlaylist = serde_json::from_str(&content)?;
        Ok(playlist)
    }
    
    #[allow(dead_code)]
    async fn save_playlist(&self, playlist: &UserPlaylist) -> Result<()> {
        let file_path = self.get_playlist_file_path(&playlist.id);
        let content = serde_json::to_string_pretty(playlist)?;
        fs::write(&file_path, content).await?;
        Ok(())
    }
    
    #[allow(dead_code)]
    async fn load_all_playlists(&mut self) -> Result<()> {
        let playlists_dir = self.data_dir.join("playlists");
        
        if !playlists_dir.exists() {
            return Ok(());
        }
        
        let mut files = fs::read_dir(&playlists_dir).await?;
        let mut loaded_count = 0;
        
        while let Some(entry) = files.next_entry().await? {
            let path = entry.path();
            
            if path.extension().map_or(false, |ext| ext == "json") {
                if let Some(file_name) = path.file_stem().and_then(|n| n.to_str()) {
                    if let Some(playlist_id) = file_name.strip_prefix("playlist_") {
                        if !self.playlists_cache.contains_key(playlist_id) {
                            match self.load_playlist(playlist_id).await {
                                Ok(playlist) => {
                                    self.playlists_cache.insert(playlist_id.to_string(), playlist);
                                    loaded_count += 1;
                                }
                                Err(e) => {
                                    warn!("Error cargando playlist {}: {}", playlist_id, e);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        if loaded_count > 0 {
            info!("📂 Cargadas {} playlists personales", loaded_count);
        }
        
        Ok(())
    }
    
    #[allow(dead_code)]
    async fn load_playlist_history(&self, user_id: u64, guild_id: u64) -> Result<PlaylistHistory> {
        let file_path = self.get_history_file_path(user_id, guild_id);
        let content = fs::read_to_string(&file_path).await?;
        let history: PlaylistHistory = serde_json::from_str(&content)?;
        Ok(history)
    }
    
    #[allow(dead_code)]
    async fn save_playlist_history(&self, history: &PlaylistHistory) -> Result<()> {
        let file_path = self.get_history_file_path(history.user_id, history.guild_id);
        let content = serde_json::to_string_pretty(history)?;
        fs::write(&file_path, content).await?;
        Ok(())
    }
    
    #[allow(dead_code)]
    fn get_playlist_file_path(&self, playlist_id: &str) -> PathBuf {
        self.data_dir.join("playlists").join(format!("playlist_{}.json", playlist_id))
    }
    
    #[allow(dead_code)]
    fn get_history_file_path(&self, user_id: u64, guild_id: u64) -> PathBuf {
        self.data_dir.join("history").join(format!("history_{}_{}.json", guild_id, user_id))
    }
}