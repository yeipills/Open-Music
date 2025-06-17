use anyhow::Result;
use serenity::{model::gateway::GatewayIntents, Client};
use songbird::{SerenityInit, Songbird};
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use tracing::{error, info};

mod audio;
mod bot;
mod cache;
mod config;
mod sources;
mod ui;

use crate::bot::OpenMusicBot;
use crate::cache::MusicCache;
use crate::config::Config;

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

    // Inicializar base de datos
    let db_pool = initialize_database(&config).await?;

    // Inicializar caché
    let cache = Arc::new(MusicCache::new(config.cache_size));

    // Configurar intents mínimos necesarios
    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    // Crear handler del bot
    let handler = OpenMusicBot::new(config.clone(), db_pool, cache);

    // Construir cliente
    let _songbird = Songbird::serenity();
    let mut client = Client::builder(&config.discord_token, intents)
        .event_handler(handler)
        .register_songbird()
        .await?;

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

async fn initialize_database(config: &Config) -> Result<SqlitePool> {
    let db_path = config.data_dir.join("openmusic.db");
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| format!("sqlite://{}", db_path.display()));

    // Asegurar que el directorio de datos existe
    std::fs::create_dir_all(&config.data_dir)?;

    // Configurar opciones de conexión SQLite
    use sqlx::sqlite::SqliteConnectOptions;
    use std::str::FromStr;
    
    let options = SqliteConnectOptions::from_str(&db_url)?
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(options).await?;

    // Ejecutar migraciones
    sqlx::migrate!("./migrations").run(&pool).await?;

    info!("✅ Base de datos inicializada");
    Ok(pool)
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
