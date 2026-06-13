use anyhow::Result;
use serenity::{model::gateway::GatewayIntents, Client};
use songbird::{SerenityInit, Songbird};
use std::sync::Arc;
use tracing::{error, info};

mod audio;
mod bot;
mod cache;
mod config;
mod monitoring;
mod sources;
mod storage;
mod ui;

use crate::bot::OpenMusicBot;
use crate::cache::MusicCache;
use crate::config::Config;
use crate::monitoring::{MonitoringSystem, MonitoringConfig};
use crate::storage::JsonStorage;

#[tokio::main]
async fn main() -> Result<()> {
    // Inicializar logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("open_music=debug".parse()?)
                .add_directive("serenity=info".parse()?)
                .add_directive("songbird=info".parse()?),
        )
        .init();

    info!("🎵 Iniciando Open Music Bot v{}", env!("CARGO_PKG_VERSION"));

    // Cargar configuración
    let config = Config::load()?;

    // Manejar health check si es necesario
    if std::env::args().any(|arg| arg == "--health-check") {
        return health_check().await;
    }

    // Inicializar almacenamiento JSON
    let storage = Arc::new(tokio::sync::Mutex::new(
        JsonStorage::new(config.data_dir.clone()).await?
    ));

    // Inicializar caché
    let cache = Arc::new(MusicCache::new(config.cache_size));

    // Inicializar sistema de monitoreo
    let monitoring_config = MonitoringConfig::default();
    let monitoring = Arc::new(MonitoringSystem::new(monitoring_config));
    info!("📊 Sistema de monitoreo activado");

    // Configurar intents mínimos necesarios
    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    // Crear handler del bot
    let handler = OpenMusicBot::new(config.clone(), storage, cache, monitoring);

    // Construir cliente con Songbird
    let songbird = Songbird::serenity();
    let mut client = Client::builder(&config.discord_token, intents)
        .event_handler(handler)
        .register_songbird_with(songbird.clone())
        .await?;

    // El sistema de audio (cola, reproducción, efectos) vive en OpenMusicBot.player.
    // Las conexiones de voz se gestionan vía songbird::get(ctx) en los handlers.
    info!("🎵 Sistema de audio listo (AudioPlayer + Songbird + yt-dlp)");

    // Manejar shutdown graceful
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Error al registrar Ctrl+C");
        info!("⚠️ Señal de shutdown recibida, cerrando...");
        std::process::exit(0);
    });

    // Iniciar bot
    info!("🚀 Bot iniciado exitosamente");
    if let Err(why) = client.start().await {
        error!("Error al ejecutar cliente: {:?}", why);
    }

    Ok(())
}

async fn health_check() -> Result<()> {
    // Verificar dependencias críticas
    let yt_dlp = async_process::Command::new("yt-dlp")
        .arg("--version")
        .output()
        .await?;

    let ffmpeg = async_process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .await?;

    if yt_dlp.status.success() && ffmpeg.status.success() {
        println!("OK");
        Ok(())
    } else {
        anyhow::bail!("Dependencias faltantes");
    }
}
