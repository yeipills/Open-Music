use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tracing::{info, warn, error};

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

/// Manager de almacenamiento basado en archivos JSON
pub struct JsonStorage {
    data_dir: PathBuf,
    servers_cache: HashMap<u64, ServerConfig>,
}

impl JsonStorage {
    pub async fn new(data_dir: PathBuf) -> Result<Self> {
        // Crear directorio de datos si no existe
        fs::create_dir_all(&data_dir).await?;
        
        let servers_dir = data_dir.join("servers");
        fs::create_dir_all(&servers_dir).await?;
        
        info!("📁 Storage inicializado en: {}", data_dir.display());
        
        let mut storage = Self {
            data_dir,
            servers_cache: HashMap::new(),
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
}