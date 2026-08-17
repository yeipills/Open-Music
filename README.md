# Open Music Bot

**Bot de música para Discord escrito en Rust.**

Reproduce audio de YouTube en canales de voz de Discord. Usa la cola nativa de
songbird, ecualización y normalización de volumen con ffmpeg, y extracción con
yt-dlp preparada para el bloqueo anti-bot de YouTube.

> **Estado:** en producción. Audio E2EE (DAVE) soportado vía songbird 0.6.

## Características

**Núcleo**
- Rust 2021, serenity `0.12` y songbird `0.6` (soporte DAVE/E2EE, obligatorio en
  Discord desde marzo de 2026).
- 25 comandos slash con embeds y botones nativos.
- La cola es la de songbird (`builtin-queue`): avance automático, aleatorio,
  repetición e historial se apoyan en ella, sin estado duplicado.

**Audio**
- Bitrate de Opus configurable (128 kbps por defecto); el techo real lo fija el
  nivel de boost del servidor. Se prefiere la fuente Opus a 48 kHz para evitar un
  remuestreo. Ver [`docs/AUDIO_QUALITY.md`](docs/AUDIO_QUALITY.md).
- Ecualización y normalización de sonoridad con filtros de ffmpeg (`loudnorm` más
  ocho presets: Bass, Pop, Rock, Jazz, Classical, Electronic, Vocal y Flat).
- Control de volumen del 0 al 200 %.

**YouTube**
- Extracción con yt-dlp en streaming, sin descargas intermedias.
- Proveedor de PO token (`bgutil`) como servicio del compose, más cookies de
  cuenta. Ver [`docs/COOKIES.md`](docs/COOKIES.md).
- Las playlists se cargan en streaming: la música arranca en cuanto se extrae el
  primer tema y el resto se encola por detrás. `/play` con un enlace de lista
  carga hasta 15 temas; para la lista completa está `/playlist`.

**Operación**
- Métricas, comprobación de salud y registro estructurado.
- Imagen Docker multi-etapa (compilación en Debian, runtime con ffmpeg, yt-dlp y
  deno).

## Arquitectura

```
/play <búsqueda | url | playlist>
        |
        v
  yt-dlp  (búsqueda, o extracción de la lista en streaming)
        |
        v
  AudioPlayer.play()  ->  cola nativa de songbird (TrackQueue)
        |
        |  songbird pide el audio sólo cuando la pista va a sonar
        v
  LazyFfmpegSource::create()
        |
        v
  yt-dlp -o -  |  ffmpeg -af "loudnorm,<eq>"  ->  ChildContainer
        |                                              |
        v                                              v
  PO token (bgutil) + cookies              songbird -> Opus -> Discord (DAVE)
```

Detalle completo en [`docs/AUDIO_PIPELINE.md`](docs/AUDIO_PIPELINE.md).

### Stack
| Componente | Tecnología |
|---|---|
| Framework / Voz | Serenity 0.12 + **Songbird 0.6** (DAVE) |
| Decodificación / Encoding | Symphonia + opus2 (vía songbird) |
| Audio (EQ/normalización) | **ffmpeg** (`loudnorm`, `equalizer`) |
| Extracción | **yt-dlp** + **bgutil PO Token provider** |
| Runtime async | Tokio |
| Contenedor | Docker (builder `rust:1`-bookworm, runtime `debian:bookworm-slim`) |

### Estructura
```
src/
├── audio/
│   ├── player.rs    # Operaciones sobre la cola nativa de songbird
│   ├── queue.rs     # Metadatos pegados a cada pista + vistas para la UI
│   ├── events.rs    # Handlers de songbird (Play, End, inactividad)
│   └── effects.rs   # Construye la cadena de filtros ffmpeg (loudnorm + EQ)
├── bot/
│   ├── handlers.rs  # Validación y dispatch de comandos (incl. playlist streaming)
│   ├── connection.rs# Estado de voz: usuario vs bot
│   └── commands.rs  # Registro de comandos slash
├── sources/
│   ├── lazy.rs      # Fuente perezosa: yt-dlp | ffmpeg sólo al reproducir
│   └── ytdlp_optimized.rs  # Búsqueda, extracción, PO token, cookies
├── ui/{embeds,buttons}.rs  # Embeds y controles
├── errors.rs               # Errores de dominio con mensajes en español
├── cache/, monitoring/     # Caché LRU y métricas
└── config.rs               # Configuración por entorno
docs/
├── AUDIO_PIPELINE.md   # Pipeline de audio
├── AUDIO_QUALITY.md    # Por qué Opus 128k (realidad de Discord)
└── COOKIES.md          # Configuración y refresco de cookies de YouTube
```

## Inicio rápido (Docker)

```bash
cp .env.example .env
# Configurar DISCORD_TOKEN y APPLICATION_ID en .env
docker compose up -d
docker compose logs -f open-music
```

Esto levanta dos contenedores: `open-music-bot` y `open-music-potprovider` (el
proveedor de PO tokens, accesible solo en la red interna del compose).

> **Cookies de YouTube:** para reproducir desde una IP de datacenter (VPS) hay que
> proveer cookies de una cuenta secundaria en `config/cookies.txt`. El método correcto
> (exportar en incógnito para que no caduquen) está en [`docs/COOKIES.md`](docs/COOKIES.md).

## Comandos

**Reproducción**
```
/play <búsqueda|url|playlist>   /pause   /resume   /stop
/skip [cantidad]   /previous   /seek <tiempo>   /nowplaying
/join [canal]   /leave
```

**Cola**
```
/queue [página]   /add <búsqueda>   /remove <pos>   /jump <pos>
/clear [all|duplicates|user]   /shuffle   /loop <off|track|queue>   /playlist   /search
```

**Audio**
```
/volume [0-200]   /equalizer <Bass|Pop|Rock|Jazz|Classical|Electronic|Vocal|Flat>
```

**Sistema**
```
/help   /health   /metrics
```

## Configuración (.env)

```env
# === DISCORD (requerido) ===
DISCORD_TOKEN=tu_bot_token
APPLICATION_ID=tu_application_id
GUILD_ID=                  # opcional: comandos solo en un servidor (testing)

# === AUDIO ===
DEFAULT_VOLUME=0.5         # 0.0–2.0
OPUS_BITRATE=128000        # techo = bitrate del canal (boost del servidor)
MAX_SONG_DURATION=7200

# === PERFORMANCE / LÍMITES ===
CACHE_SIZE=100
AUDIO_CACHE_SIZE=50
MAX_QUEUE_SIZE=1000
MAX_PLAYLIST_SIZE=100
RATE_LIMIT_PER_USER=20
WORKER_THREADS=            # vacío = auto (nº de CPUs)

# === FEATURES ===
ENABLE_EQUALIZER=true
ENABLE_AUTOPLAY=false

# === PO TOKEN (opcional; default apunta al servicio del compose) ===
# POT_PROVIDER_URL=http://bgutil-provider:4416

# === PATHS / LOGGING ===
DATA_DIR=/app/data
CACHE_DIR=/app/cache
RUST_LOG=info,open_music=debug
RUST_BACKTRACE=1
```

## YouTube: cookies y PO token

YouTube bloquea las IPs de datacenter con *"Sign in to confirm you're not a bot"*
(`LOGIN_REQUIRED`). Para reproducir hacen falta **las dos cosas**:

1. **PO Token provider** (`bgutil-provider`, ya incluido en el compose) — robustez del streaming.
2. **Cookies** de una cuenta secundaria en `config/cookies.txt`.

Puntos clave (detalle en [`docs/COOKIES.md`](docs/COOKIES.md)):
- Exportar las cookies **en ventana de incógnito** y cerrarla **sin logout**, o YouTube
  las rota e invalida en minutos.
- El bot pasa a yt-dlp una **copia descartable** de las cookies por invocación, para no
  degradar el `config/cookies.txt` original (yt-dlp lo reescribiría).
- `config/cookies.txt` está en `.gitignore` — nunca commitearlo.

## Docker

```bash
docker compose build              # construir (build largo: compila Rust + DAVE/MLS)
docker compose up -d              # levantar bot + PO token provider
docker compose logs -f open-music # logs
docker compose restart open-music # reiniciar solo el bot (ej. tras cambiar cookies)
```

Notas:
- El builder usa `rust:1`-bookworm (la cadena DAVE arrastra `openmls`, que requiere
  Rust ≥ 1.87). El runtime es `debian:bookworm-slim` con ffmpeg, yt-dlp y deno.
- El bot no necesita puertos entrantes; el `8080` interno (métricas) se mapea a
  `127.0.0.1:8095` para no chocar con otros servicios del host.

## Solución de problemas

| Síntoma (en logs) | Causa | Solución |
|---|---|---|
| `close code 4017 / DAVE protocol required` | Songbird sin soporte DAVE | Usar Songbird ≥ 0.6 (ya incluido) |
| `Sign in to confirm you're not a bot` / `LOGIN_REQUIRED` | Cookies ausentes/quemadas | Re-exportar cookies en incógnito → `config/cookies.txt` |
| `cookies are no longer valid, rotated in the browser` | Cookies exportadas de sesión activa | Exportar en incógnito y cerrar sin logout |
| `symphonia probe reach EOF at 0 bytes` | yt-dlp devolvió 0 bytes (bloqueo) | Mismo que arriba (cookies) |
| `DISCORD_TOKEN not found` | Falta el token | Configurar `.env` |

## Desarrollo

No hace falta tener Rust instalado: todo se valida en un contenedor.

```bash
# Comprobar que compila
docker run --rm -v "$PWD":/app -w /app rust:1-bookworm \
  bash -c "apt-get update -qq && apt-get install -y -qq cmake libopus-dev pkg-config && cargo check"

# Tests
docker run --rm -v "$PWD":/app -w /app rust:1-bookworm \
  bash -c "apt-get update -qq && apt-get install -y -qq cmake libopus-dev pkg-config && cargo test"
```

Con Rust local basta con `cargo check`, `cargo test` y `cargo clippy`.

La arquitectura interna, las invariantes que no hay que romper y la guía para
trabajar en el código están en [DEVELOPMENT.md](DEVELOPMENT.md). Para contribuir,
[CONTRIBUTING.md](CONTRIBUTING.md).

## Licencia

MIT — ver [LICENSE](LICENSE).

---

Construido con Rust, serenity, songbird, ffmpeg y yt-dlp.
