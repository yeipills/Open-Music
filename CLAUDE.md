
  📋 INFORMACIÓN DEL PROYECTO OPEN MUSIC

  🎯 Descripción del Proyecto

  Open Music Bot - Bot de música para Discord de alto rendimiento construido en Rust 🦀. Bot moderno con arquitectura optimizada, soporte completo para comandos slash, e
  interfaz interactiva avanzada. Estado: COMPLETAMENTE FUNCIONAL y listo para producción.

  🏗️ Arquitectura y Stack Tecnológico

  Lenguaje Principal: Rust 2021 Edition (v1.82+)

  Dependencias Principales:
  # Framework Discord
  serenity = "0.12.4" (features: voice, gateway, rustls_backend, cache)
  songbird = "0.5.0" (features: builtin-queue, serenity, driver)

  # Audio Processing
  symphonia = "0.5.5" (mp3, aac, flac, wav, ogg, isomp4)  # decodificación (vía songbird)
  audiopus = "0.3.0-rc.0"
  # EQ y loudness normalization se hacen con filtros ffmpeg (-af), no con crates DSP.
  # Pipeline y calidad documentados en docs/AUDIO_PIPELINE.md y docs/AUDIO_QUALITY.md.

  # Async Runtime
  tokio = "1.45" (features: full, rt-multi-thread)
  async-trait = "0.1.88"

  # Utilidades
  serde = "1.0" (derive)
  anyhow = "1.0"
  tracing = "0.1"
  reqwest = "0.12.20"
  dashmap = "6.1"

  📁 Estructura del Proyecto

  src/
  ├── audio/              # Sistema de audio
  │   ├── player.rs      # Motor de reproducción principal
  │   ├── queue.rs       # Gestión de cola avanzada
  │   ├── effects.rs     # Ecualizador y procesamiento
  │   └── robust_queue.rs # Cola robusta con recovery
  ├── bot/               # Lógica del bot Discord
  │   ├── commands.rs    # Comandos slash implementados
  │   ├── fast_commands.rs # Comandos optimizados
  │   ├── handlers.rs    # Manejadores de eventos
  │   ├── events.rs      # Eventos de Discord
  │   └── search.rs      # Sistema de búsqueda
  ├── sources/           # Fuentes de audio
  │   ├── ytdlp_optimized.rs # yt-dlp ultra-optimizado
  │   └── search_optimizer.rs # Optimizador de búsqueda
  ├── ui/                # Interfaz de usuario
  │   ├── embeds.rs      # Embeds ricos
  │   └── buttons.rs     # Controles interactivos
  ├── cache/             # Sistema de caché
  │   ├── lru_cache.rs   # Cache LRU con TTL
  │   └── optimized_cache.rs # Cache optimizado
  ├── monitoring/        # Monitoreo y métricas
  │   ├── performance_monitor.rs
  │   ├── health_checker.rs
  │   └── error_tracker.rs
  ├── config.rs          # Configuración centralizada
  ├── storage.rs         # Almacenamiento JSON persistente
  └── main.rs           # Punto de entrada principal

  ⚙️ Configuración y Variables de Entorno

  Variables Requeridas:
  DISCORD_TOKEN=tu_bot_token_discord
  APPLICATION_ID=tu_application_id

  Variables de Audio:
  DEFAULT_VOLUME=0.5 (0.0-2.0)
  OPUS_BITRATE=128000 (64000-510000)
  FRAME_SIZE=960 (120-2880)
  MAX_SONG_DURATION=7200 # 2 horas

  Variables de Performance:
  CACHE_SIZE=100
  AUDIO_CACHE_SIZE=50
  MAX_QUEUE_SIZE=1000
  WORKER_THREADS= # Auto-detecta CPUs
  MAX_PLAYLIST_SIZE=100
  RATE_LIMIT_PER_USER=20

  🛠️ Scripts y Comandos de Desarrollo

  Comandos de Cargo:
  # Desarrollo
  cargo build              # Debug build
  cargo run                 # Ejecutar con debug
  cargo test                # Tests unitarios
  cargo clippy              # Linter
  cargo fmt                 # Formatear código

  # Producción
  cargo build --release     # Build optimizado
  STRIP=true cargo build --release # Binary ~15MB

  Docker:
  docker-compose up -d      # Iniciar servicios
  docker-compose logs -f    # Ver logs
  docker-compose restart    # Reiniciar
  docker-compose down       # Parar y remover

  Scripts de Testing:
  - scripts/simple_test.sh - Prueba directa de yt-dlp
  - scripts/test_youtube_fix.sh - Test de corrección YouTube
  - scripts/test_hierarchical_system.sh - Test sistema jerárquico

  🎵 Funcionalidades Implementadas

  Control de Reproducción:
  - ▶️ Play/Pause/Stop con controles básicos
  - ⏭️ Skip/Previous para navegación
  - 🔀 Shuffle con reproducción aleatoria
  - 🔁 Repeat con modos Off/Track/Queue
  - ⏰ Seek para saltar a posición específica
  - 📴 Auto-disconnect por inactividad

  Audio Avanzado:
  - 🎚️ Ecualizador con 8 presets (Bass, Pop, Rock, Jazz, Classical, Electronic, Vocal, Flat)
  - 🔊 Control de volumen 0-200% con normalización
  - 🎵 Alta calidad Opus 96-384kbps, 48kHz
  - 🔄 Procesamiento con filtros FIR/IIR en tiempo real

  Gestión de Cola:
  - 📄 Visualización con paginación automática (10 tracks/página)
  - ➕ Agregar/Remover por posición o patrón
  - 🔄 Reordenar tracks dinámicamente
  - 🗑️ Limpiar total, duplicados, por usuario
  - 🎯 Jump a posición específica
  - 📈 Historial de últimas 50 reproducciones

  🚀 Comandos Disponibles

  Reproducción:
  /play <búsqueda>     # Reproduce canción o playlist
  /pause               # Pausar reproducción
  /resume              # Reanudar reproducción
  /stop                # Detener y limpiar cola
  /skip [cantidad]     # Saltar canciones
  /previous            # Canción anterior
  /seek <tiempo>       # Saltar a posición (ej: 1:30)

  Gestión de Cola:
  /queue [página]      # Ver cola paginada
  /add <búsqueda>      # Agregar sin reproducir
  /remove <posición>   # Remover canción específica
  /clear [filtro]      # Limpiar (all/duplicates/user)
  /shuffle             # Toggle aleatorio
  /loop <modo>         # off/track/queue
  /jump <posición>     # Saltar a posición

  Audio:
  /volume [0-200]      # Ajustar volumen
  /equalizer <preset>  # Presets de ecualizador
  /bassboost [nivel]   # 0-100 intensidad
  /normalize           # Normalizar niveles

  🐳 Configuración Docker

  Especificaciones del Contenedor:
  - Imagen base: Alpine 3.21
  - Tamaño final: ~50MB (multi-stage optimizado)
  - RAM reservada: 256MB / límite: 512MB
  - CPU reservada: 0.5 cores / límite: 2.0 cores
  - Health check integrado cada 30s
  - Logging optimizado (JSON, max 10MB, 3 archivos)

  📊 Métricas de Rendimiento

  Objetivos de Performance:
  - Memoria RAM: <100MB (típico 50-80MB)
  - CPU idle: <5% (típico 1-2%)
  - CPU playing: <20% (típico 10-15%)
  - Latencia audio: <100ms (típico 50-80ms)
  - Respuesta comandos: <500ms (típico 100-200ms)
  - Servidores concurrentes: 100+ (testado 50+)

  🔧 Patrones de Código y Mejores Prácticas

  Error Handling:
  // Usar anyhow::Result para operaciones fallibles
  pub async fn play_song(url: &str) -> anyhow::Result<()> {
      let source = extract_audio(url).await
          .with_context(|| format!("Failed to extract: {}", url))?;
      Ok(())
  }

  Async Operations:
  // Spawn background tasks para operaciones no-bloqueantes
  tokio::spawn(async move {
      if let Err(e) = download_audio(url).await {
          tracing::warn!("Background download failed: {:?}", e);
      }
  });

  📝 Información de Testing

  Testing Commands:
  cargo test                # Unit tests
  cargo test --test integration # Integration tests
  cargo test -- --nocapture    # Tests con output
  cargo test --release         # Optimized test run

  Herramientas de Quality:
  cargo audit               # Security audit
  cargo outdated            # Check updates
  cargo tree               # Dependency tree
  cargo clippy -- -D warnings # Linter estricto

  🔍 Troubleshooting Común

  Errores Frecuentes:
  - DISCORD_TOKEN not found: Configurar token en .env
  - opus link error: Instalar libopus-dev
  - cmake not found: Instalar cmake build-essential
  - yt-dlp not found: pip3 install --upgrade yt-dlp

  📚 Archivos de Documentación

  - README.md - Información general completa
  - DEVELOPMENT.md - Guía de desarrollo detallada
  - TROUBLESHOOTING.md - Solución de problemas
