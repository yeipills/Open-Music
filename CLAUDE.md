# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Este es Open Music Bot - un bot de música para Discord de alto rendimiento construido en Rust con arquitectura moderna. El bot soporta YouTube, URLs directas, procesamiento de audio avanzado (ecualizador), y un sistema de cola robusto.

## Architecture

### Core Components

- **Audio System** (`src/audio/`): Audio player, queue management, effects processing, and equalizer
- **Bot Framework** (`src/bot/`): Discord commands, event handlers, and voice connection management
- **Source Integration** (`src/sources/`): YouTube, Spotify, SoundCloud, and direct URL handlers
- **Cache System** (`src/cache/`): LRU cache with TTL for metadata and audio data
- **Storage** (`src/storage.rs`): JSON-based persistent storage for configurations and settings
- **UI Components** (`src/ui/`): Discord embed and button builders

### Tecnologías Clave

- **Serenity** + **Songbird** para integración con Discord y voz
- **Symphonia** para decodificación de audio (reemplaza dependencia de FFmpeg)
- **yt-dlp** para extracción de audio de YouTube
- **Tokio** runtime async con configuración optimizada
- **DashMap** para estructuras de datos concurrentes

## Comandos de Desarrollo

### Construcción y Ejecución
```bash
cargo build --release    # Production build with optimizations
cargo run                # Development run
```

### Pruebas y Calidad
```bash
cargo test               # Run unit tests
cargo test --test integration  # Run integration tests
cargo clippy            # Linting
cargo fmt               # Code formatting
```

### Desarrollo con Docker
```bash
docker-compose up -d     # Run in container
docker-compose logs -f   # View logs
```

## Configuración

### Configuración del Entorno
Copy `.env.example` to `.env` and configure:
- `DISCORD_TOKEN`: Bot token (required)
- `APPLICATION_ID`: Discord application ID (required)
- `GUILD_ID`: Guild ID for development (optional)

### Configuración de Audio
- `OPUS_BITRATE`: Audio quality (default: 128000)
- `DEFAULT_VOLUME`: Initial volume (default: 0.5)
- `MAX_QUEUE_SIZE`: Queue limit (default: 1000)

## Patrones de Código

### Manejo de Errores
- Use `anyhow::Result<T>` for functions that can fail
- Log errors with appropriate severity levels
- Return descriptive error messages to users

### Operaciones Asíncronas
- All Discord and audio operations are async
- Use `tokio::spawn` for background tasks
- Proper mutex handling for shared state

### Gestión de Conexiones de Voz
- Voice handlers stored in `DashMap<GuildId, Arc<Mutex<Call>>>`
- Auto-disconnect when bot is alone in channel
- Graceful cleanup on disconnection

### Procesamiento de Audio
- Symphonia for decoding (no FFmpeg dependency)
- Opus encoding for Discord voice
- Basic effects applied in real-time pipeline

## Detalles Importantes de Implementación

### Registro de Comandos
- Commands can be registered globally or per-guild
- Guild-specific registration for development (faster updates)
- Global registration for production deployment

### Sistema de Cola
- Supports shuffle, repeat modes, and seeking
- Concurrent-safe operations with proper locking
- Automatic cleanup of expired entries

### Estrategia de Cache
- LRU cache for metadata and audio data
- TTL-based expiration for memory management
- Separate caches for different data types

### Integración de Fuentes
- YouTube: Primary source using yt-dlp
- Direct URLs: Support for various audio formats

## Consideraciones de Rendimiento

- Rust 2024 edition with aggressive optimizations
- Memory usage typically 50-100MB runtime
- Supports 100+ concurrent guilds
- Audio latency < 100ms typical

## Resolución de Problemas

### Problemas Comunes
- Missing system dependencies: `cmake`, `libopus-dev`, `libssl-dev`
- yt-dlp outdated: Update to latest version
- Discord permissions: Bot needs Voice permissions
- Compilation: Requires Rust 1.85+

### Depuración
- Set `RUST_LOG=debug` for verbose logging
- Check Docker health checks for container issues
- Monitor memory usage with built-in metrics