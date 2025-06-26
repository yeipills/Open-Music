use anyhow::Result;
use serenity::{
    all::{
        CommandInteraction, Context, CreateInteractionResponse, CreateInteractionResponseMessage,
        ComponentInteraction,
    },
    builder::CreateEmbed,
};
use std::{sync::Arc, time::Instant};
use tracing::{debug, info, warn};
use tokio::time::timeout;
use std::time::Duration;

use crate::{bot::OpenMusicBot, ui::embeds::MusicEmbeds};

/// Optimizador de comandos para respuestas ultra-rápidas
pub struct FastCommandProcessor {
    /// Caché de respuestas pre-generadas
    response_cache: dashmap::DashMap<String, CachedResponse>,
    
    /// Estadísticas de rendimiento
    stats: Arc<CommandStats>,
    
    /// Configuración de optimizaciones
    config: FastCommandConfig,
}

#[derive(Debug, Clone)]
struct CachedResponse {
    embed: CreateEmbed,
    content: Option<String>,
    ephemeral: bool,
    created_at: Instant,
}

#[derive(Debug, Clone)]
pub struct FastCommandConfig {
    /// Timeout máximo para comandos
    pub command_timeout: Duration,
    
    /// Usar respuestas en caché
    pub use_cache: bool,
    
    /// TTL para respuestas cacheadas
    pub cache_ttl: Duration,
    
    /// Diferir respuestas automáticamente para comandos pesados
    pub auto_defer: bool,
    
    /// Umbral para auto-defer (ms)
    pub defer_threshold: Duration,
}

#[derive(Debug)]
struct CommandStats {
    total_commands: std::sync::atomic::AtomicU64,
    fast_responses: std::sync::atomic::AtomicU64,
    deferred_responses: std::sync::atomic::AtomicU64,
    cache_hits: std::sync::atomic::AtomicU64,
    timeouts: std::sync::atomic::AtomicU64,
    avg_response_time: std::sync::atomic::AtomicU64, // microseconds
}

impl Default for FastCommandConfig {
    fn default() -> Self {
        Self {
            command_timeout: Duration::from_secs(25), // Antes del timeout de Discord (30s)
            use_cache: true,
            cache_ttl: Duration::from_secs(60),
            auto_defer: true,
            defer_threshold: Duration::from_millis(2000), // 2 segundos
        }
    }
}

impl FastCommandProcessor {
    pub fn new(config: FastCommandConfig) -> Self {
        let processor = Self {
            response_cache: dashmap::DashMap::new(),
            stats: Arc::new(CommandStats {
                total_commands: std::sync::atomic::AtomicU64::new(0),
                fast_responses: std::sync::atomic::AtomicU64::new(0),
                deferred_responses: std::sync::atomic::AtomicU64::new(0),
                cache_hits: std::sync::atomic::AtomicU64::new(0),
                timeouts: std::sync::atomic::AtomicU64::new(0),
                avg_response_time: std::sync::atomic::AtomicU64::new(0),
            }),
            config,
        };

        // Iniciar tarea de limpieza de caché
        processor.start_cache_cleanup();
        
        info!("⚡ Procesador de comandos rápidos iniciado");
        processor
    }

    /// Procesa comando con optimizaciones de velocidad
    pub async fn process_command(
        &self,
        ctx: &Context,
        interaction: CommandInteraction,
        bot: &OpenMusicBot,
    ) -> Result<()> {
        let start_time = Instant::now();
        let command_name = interaction.data.name.clone();
        
        self.stats.total_commands.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        debug!("⚡ Procesando comando rápido: {}", command_name);

        // Verificar caché primero para comandos estáticos
        if self.config.use_cache {
            if let Some(cached) = self.get_cached_response(&command_name, &interaction) {
                return self.send_cached_response(ctx, &interaction, cached, start_time).await;
            }
        }

        // Determinar si se debe diferir automáticamente
        let should_defer = self.should_auto_defer(&command_name);
        
        if should_defer {
            // Diferir inmediatamente para comandos pesados
            interaction.defer(ctx).await?;
            self.stats.deferred_responses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            
            // Procesar con timeout
            match timeout(self.config.command_timeout, self.execute_command(ctx, &interaction, bot)).await {
                Ok(Ok(())) => {
                    self.record_response_time(start_time);
                    Ok(())
                }
                Ok(Err(e)) => {
                    self.send_error_followup(ctx, &interaction, &e.to_string()).await?;
                    Err(e)
                }
                Err(_) => {
                    self.stats.timeouts.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    self.send_timeout_followup(ctx, &interaction).await?;
                    Err(anyhow::anyhow!("Comando timeout"))
                }
            }
        } else {
            // Respuesta inmediata para comandos ligeros
            match timeout(self.config.defer_threshold, self.execute_command_fast(ctx, &interaction, bot)).await {
                Ok(Ok(())) => {
                    self.stats.fast_responses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    self.record_response_time(start_time);
                    Ok(())
                }
                Ok(Err(e)) => Err(e),
                Err(_) => {
                    // Si se pasa del umbral, diferir y continuar
                    warn!("⏰ Comando {} excedió umbral, diferir", command_name);
                    interaction.defer(ctx).await?;
                    self.stats.deferred_responses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    
                    match self.execute_command(ctx, &interaction, bot).await {
                        Ok(()) => {
                            self.record_response_time(start_time);
                            Ok(())
                        }
                        Err(e) => {
                            self.send_error_followup(ctx, &interaction, &e.to_string()).await?;
                            Err(e)
                        }
                    }
                }
            }
        }
    }

    /// Ejecuta comando de forma optimizada para respuesta rápida
    async fn execute_command_fast(
        &self,
        ctx: &Context,
        interaction: &CommandInteraction,
        bot: &OpenMusicBot,
    ) -> Result<()> {
        match interaction.data.name.as_str() {
            "pause" => self.fast_pause(ctx, interaction, bot).await,
            "resume" => self.fast_resume(ctx, interaction, bot).await,
            "skip" => self.fast_skip(ctx, interaction, bot).await,
            "stop" => self.fast_stop(ctx, interaction, bot).await,
            "nowplaying" => self.fast_nowplaying(ctx, interaction, bot).await,
            "queue" => self.fast_queue(ctx, interaction, bot).await,
            "volume" => self.fast_volume(ctx, interaction, bot).await,
            _ => {
                // Para comandos no optimizados, usar el handler normal
                crate::bot::handlers::handle_command(ctx, interaction.clone(), bot).await
            }
        }
    }

    /// Ejecuta comando diferido (con más tiempo)
    async fn execute_command(
        &self,
        ctx: &Context,
        interaction: &CommandInteraction,
        bot: &OpenMusicBot,
    ) -> Result<()> {
        // Usar handler normal para comandos diferidos
        match interaction.data.name.as_str() {
            "play" => crate::bot::handlers::handle_play(ctx, interaction.clone(), bot).await,
            "search" => crate::bot::search::handle_search_command(ctx, interaction.clone(), bot).await,
            "playlist" => crate::bot::handlers::handle_playlist(ctx, interaction.clone(), bot).await,
            _ => crate::bot::handlers::handle_command(ctx, interaction.clone(), bot).await,
        }
    }

    // Comandos optimizados para respuesta ultra-rápida

    async fn fast_pause(&self, ctx: &Context, interaction: &CommandInteraction, bot: &OpenMusicBot) -> Result<()> {
        let guild_id = interaction.guild_id.ok_or_else(|| anyhow::anyhow!("Comando solo en servidores"))?;
        
        match bot.player.pause(guild_id).await {
            Ok(()) => {
                let embed = MusicEmbeds::create_success_embed("⏸️ Pausado", "Reproducción pausada");
                self.send_fast_response(ctx, interaction, embed, false).await
            }
            Err(e) => {
                let embed = MusicEmbeds::create_error_embed("Error", &format!("No se pudo pausar: {}", e));
                self.send_fast_response(ctx, interaction, embed, true).await
            }
        }
    }

    async fn fast_resume(&self, ctx: &Context, interaction: &CommandInteraction, bot: &OpenMusicBot) -> Result<()> {
        let guild_id = interaction.guild_id.ok_or_else(|| anyhow::anyhow!("Comando solo en servidores"))?;
        
        match bot.player.resume(guild_id).await {
            Ok(()) => {
                let embed = MusicEmbeds::create_success_embed("▶️ Reanudado", "Reproducción reanudada");
                self.send_fast_response(ctx, interaction, embed, false).await
            }
            Err(e) => {
                let embed = MusicEmbeds::create_error_embed("Error", &format!("No se pudo reanudar: {}", e));
                self.send_fast_response(ctx, interaction, embed, true).await
            }
        }
    }

    async fn fast_skip(&self, ctx: &Context, interaction: &CommandInteraction, bot: &OpenMusicBot) -> Result<()> {
        let guild_id = interaction.guild_id.ok_or_else(|| anyhow::anyhow!("Comando solo en servidores"))?;
        
        // Obtener amount si se especificó
        let amount = interaction.data.options.iter()
            .find(|opt| opt.name == "amount")
            .and_then(|opt| opt.value.as_i64())
            .unwrap_or(1) as usize;

        match bot.player.skip(guild_id, amount).await {
            Ok(skipped) => {
                let message = if skipped == 1 {
                    "⏭️ Canción saltada".to_string()
                } else {
                    format!("⏭️ {} canciones saltadas", skipped)
                };
                let embed = MusicEmbeds::create_success_embed("Saltado", &message);
                self.send_fast_response(ctx, interaction, embed, false).await
            }
            Err(e) => {
                let embed = MusicEmbeds::create_error_embed("Error", &format!("No se pudo saltar: {}", e));
                self.send_fast_response(ctx, interaction, embed, true).await
            }
        }
    }

    async fn fast_stop(&self, ctx: &Context, interaction: &CommandInteraction, bot: &OpenMusicBot) -> Result<()> {
        let guild_id = interaction.guild_id.ok_or_else(|| anyhow::anyhow!("Comando solo en servidores"))?;
        
        match bot.player.stop(guild_id).await {
            Ok(()) => {
                let embed = MusicEmbeds::create_success_embed("⏹️ Detenido", "Reproducción detenida y cola limpiada");
                self.send_fast_response(ctx, interaction, embed, false).await
            }
            Err(e) => {
                let embed = MusicEmbeds::create_error_embed("Error", &format!("No se pudo detener: {}", e));
                self.send_fast_response(ctx, interaction, embed, true).await
            }
        }
    }

    async fn fast_nowplaying(&self, ctx: &Context, interaction: &CommandInteraction, bot: &OpenMusicBot) -> Result<()> {
        let guild_id = interaction.guild_id.ok_or_else(|| anyhow::anyhow!("Comando solo en servidores"))?;
        
        match bot.player.get_current_track(guild_id).await {
            Some(track) => {
                let embed = MusicEmbeds::create_now_playing_embed(&track, None);
                self.send_fast_response(ctx, interaction, embed, false).await
            }
            None => {
                let embed = MusicEmbeds::create_info_embed("Sin Reproducción", "No hay ninguna canción reproduciéndose");
                self.send_fast_response(ctx, interaction, embed, true).await
            }
        }
    }

    async fn fast_queue(&self, ctx: &Context, interaction: &CommandInteraction, bot: &OpenMusicBot) -> Result<()> {
        let guild_id = interaction.guild_id.ok_or_else(|| anyhow::anyhow!("Comando solo en servidores"))?;
        
        let page = interaction.data.options.iter()
            .find(|opt| opt.name == "page")
            .and_then(|opt| opt.value.as_i64())
            .unwrap_or(1) as usize;

        let queue = bot.player.get_queue(guild_id);
        
        if queue.is_empty() {
            let embed = MusicEmbeds::create_info_embed("Cola Vacía", "No hay canciones en la cola");
            self.send_fast_response(ctx, interaction, embed, true).await
        } else {
            let embed = MusicEmbeds::create_queue_embed(&queue, page, 10);
            self.send_fast_response(ctx, interaction, embed, false).await
        }
    }

    async fn fast_volume(&self, ctx: &Context, interaction: &CommandInteraction, bot: &OpenMusicBot) -> Result<()> {
        let guild_id = interaction.guild_id.ok_or_else(|| anyhow::anyhow!("Comando solo en servidores"))?;
        
        if let Some(level) = interaction.data.options.iter()
            .find(|opt| opt.name == "level")
            .and_then(|opt| opt.value.as_i64()) {
            
            match bot.player.set_volume(guild_id, level as u8).await {
                Ok(()) => {
                    let embed = MusicEmbeds::create_success_embed(
                        "🔊 Volumen Ajustado", 
                        &format!("Volumen establecido a {}%", level)
                    );
                    self.send_fast_response(ctx, interaction, embed, false).await
                }
                Err(e) => {
                    let embed = MusicEmbeds::create_error_embed("Error", &format!("No se pudo ajustar volumen: {}", e));
                    self.send_fast_response(ctx, interaction, embed, true).await
                }
            }
        } else {
            // Mostrar volumen actual
            let current_volume = bot.player.get_volume(guild_id).await.unwrap_or(50);
            let embed = MusicEmbeds::create_info_embed(
                "🔊 Volumen Actual", 
                &format!("El volumen está en {}%", current_volume)
            );
            self.send_fast_response(ctx, interaction, embed, true).await
        }
    }

    // Métodos de utilidad

    async fn send_fast_response(
        &self,
        ctx: &Context,
        interaction: &CommandInteraction,
        embed: CreateEmbed,
        ephemeral: bool,
    ) -> Result<()> {
        let response = CreateInteractionResponseMessage::new()
            .embed(embed)
            .ephemeral(ephemeral);

        interaction
            .create_response(ctx, CreateInteractionResponse::Message(response))
            .await?;

        Ok(())
    }

    async fn send_error_followup(&self, ctx: &Context, interaction: &CommandInteraction, error: &str) -> Result<()> {
        let embed = MusicEmbeds::create_error_embed("Error", error);
        
        interaction
            .create_followup(
                ctx,
                serenity::builder::CreateInteractionResponseFollowup::new()
                    .embed(embed)
                    .ephemeral(true),
            )
            .await?;

        Ok(())
    }

    async fn send_timeout_followup(&self, ctx: &Context, interaction: &CommandInteraction) -> Result<()> {
        let embed = MusicEmbeds::create_error_embed(
            "Timeout", 
            "El comando tardó demasiado en procesarse. Inténtalo de nuevo."
        );
        
        interaction
            .create_followup(
                ctx,
                serenity::builder::CreateInteractionResponseFollowup::new()
                    .embed(embed)
                    .ephemeral(true),
            )
            .await?;

        Ok(())
    }

    fn should_auto_defer(&self, command_name: &str) -> bool {
        if !self.config.auto_defer {
            return false;
        }

        // Comandos que siempre deben diferirse por ser pesados
        matches!(command_name, 
            "play" | "search" | "playlist" | "lyrics" | 
            "playlist-play" | "server-stats" | "server-export"
        )
    }

    fn get_cached_response(&self, command_name: &str, interaction: &CommandInteraction) -> Option<CachedResponse> {
        if !self.config.use_cache {
            return None;
        }

        // Solo cachear comandos que no cambian frecuentemente
        if !matches!(command_name, "help" | "ping" | "info") {
            return None;
        }

        let cache_key = format!("{}_{}", command_name, interaction.guild_id.unwrap_or_default());
        
        if let Some(cached) = self.response_cache.get(&cache_key) {
            if cached.created_at.elapsed() < self.config.cache_ttl {
                self.stats.cache_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Some(cached.clone());
            }
        }

        None
    }

    async fn send_cached_response(
        &self,
        ctx: &Context,
        interaction: &CommandInteraction,
        cached: CachedResponse,
        start_time: Instant,
    ) -> Result<()> {
        let mut response = CreateInteractionResponseMessage::new()
            .embed(cached.embed)
            .ephemeral(cached.ephemeral);

        if let Some(content) = cached.content {
            response = response.content(content);
        }

        interaction
            .create_response(ctx, CreateInteractionResponse::Message(response))
            .await?;

        self.record_response_time(start_time);
        Ok(())
    }

    fn record_response_time(&self, start_time: Instant) {
        let elapsed_micros = start_time.elapsed().as_micros() as u64;
        
        // Promedio móvil simple
        let current_avg = self.stats.avg_response_time.load(std::sync::atomic::Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            elapsed_micros
        } else {
            (current_avg * 3 + elapsed_micros) / 4 // Factor de suavizado
        };
        
        self.stats.avg_response_time.store(new_avg, std::sync::atomic::Ordering::Relaxed);
    }

    fn start_cache_cleanup(&self) {
        let cache = self.response_cache.clone();
        let ttl = self.config.cache_ttl;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // Cada 5 minutos
            
            loop {
                interval.tick().await;
                
                cache.retain(|_, cached| cached.created_at.elapsed() < ttl);
            }
        });
    }

    pub fn get_performance_stats(&self) -> CommandPerformanceStats {
        CommandPerformanceStats {
            total_commands: self.stats.total_commands.load(std::sync::atomic::Ordering::Relaxed),
            fast_responses: self.stats.fast_responses.load(std::sync::atomic::Ordering::Relaxed),
            deferred_responses: self.stats.deferred_responses.load(std::sync::atomic::Ordering::Relaxed),
            cache_hits: self.stats.cache_hits.load(std::sync::atomic::Ordering::Relaxed),
            timeouts: self.stats.timeouts.load(std::sync::atomic::Ordering::Relaxed),
            avg_response_time_ms: self.stats.avg_response_time.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1000.0,
            cache_size: self.response_cache.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandPerformanceStats {
    pub total_commands: u64,
    pub fast_responses: u64,
    pub deferred_responses: u64,
    pub cache_hits: u64,
    pub timeouts: u64,
    pub avg_response_time_ms: f64,
    pub cache_size: usize,
}