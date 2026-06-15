# 🎵 Open Music Bot

**Bot de música para Discord de alto rendimiento construido en Rust 🦀**

Motor único de reproducción con cola real, ecualizador y normalización por ffmpeg,
audio Opus nativo y extracción vía yt-dlp con soporte para el bloqueo anti-bot de
YouTube. Footprint mínimo (~100 MB RAM).

> **Estado:** en producción. Audio E2EE (DAVE) soportado vía Songbird 0.6.

## ⚡ Características

**Core**
- **Rust 2021**, Serenity `0.12` + **Songbird `0.6`** (soporte **DAVE/E2EE**, obligatorio
  en Discord desde 2026-03).
- 24 comandos slash con `dm_permission`, embeds ricos y botones nativos.
- Cola real con auto-avance, shuffle, loop e historial.
- Tests unitarios de config y storage.

**Audio**
- **Un solo motor** (`AudioPlayer`): cola, reproducción, efectos y eventos unificados.
- **Calidad Opus configurable** (por defecto **128 kbps**; el techo real lo marca el nivel
  de boost del servidor de Discord). Fuente preferida **Opus/48 kHz** para evitar resample.
  Ver [`docs/AUDIO_QUALITY.md`](docs/AUDIO_QUALITY.md).
- **EQ real + loudness normalization** vía filtros **ffmpeg** (`loudnorm` + 8 presets:
  Bass, Pop, Rock, Jazz, Classical, Electronic, Vocal, Flat).
- Control de volumen 0–200 %.

**YouTube (anti-bot)**
- Extracción con **yt-dlp** en streaming directo (sin descargas intermedias).
- **PO Token provider** (`bgutil`) como servicio del compose + **cookies** de cuenta.
  Ver [`docs/COOKIES.md`](docs/COOKIES.md).
- **Playlists en streaming**: la música arranca apenas se extrae el primer tema y el
  resto se encola en segundo plano (rápido incluso en listas largas). Soporta
  `playlist?list=`, `watch?v=...&list=` y radios/mixes (`list=RD`, con tope de 50).

**Operación**
- Monitoreo y métricas en tiempo real, health check integrado.
- Docker multi-stage (build Debian/glibc, runtime con ffmpeg + yt-dlp + deno).

## 🏗️ Arquitectura

```
Usuario: /play <query|url|playlist>
        │
        ▼
yt-dlp (búsqueda / extracción de playlist en streaming lazy)
        │
        ▼
AudioPlayer  ──►  MusicQueue (cola, auto-avance)
        │
        ▼ (por track)
yt-dlp -o -  │  ffmpeg -af "loudnorm,<eq>"  ──►  ChildContainer
        │                                              │
        ▼                                              ▼
   PO Token (bgutil) + cookies            songbird → Opus 128k → Discord (DAVE/E2EE)
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
│   ├── player.rs    # Motor único: cola, reproducción, auto-avance, eventos
│   ├── queue.rs     # Cola (shuffle, loop, historial)
│   └── effects.rs   # Construye la cadena de filtros ffmpeg (loudnorm + EQ)
├── bot/
│   ├── handlers.rs  # Dispatch de comandos y lógica de /play (incl. playlist streaming)
│   ├── commands.rs  # Registro de comandos slash
│   └── events.rs    # Eventos de Discord
├── sources/
│   └── ytdlp_optimized.rs  # Búsqueda, extracción, cadena yt-dlp|ffmpeg, PO token, cookies
├── ui/{embeds,buttons}.rs  # Embeds y controles
├── cache/, monitoring/     # Caché LRU y métricas
└── config.rs               # Configuración por entorno
docs/
├── AUDIO_PIPELINE.md   # Pipeline de audio
├── AUDIO_QUALITY.md    # Por qué Opus 128k (realidad de Discord)
└── COOKIES.md          # Configuración y refresco de cookies de YouTube
```

## 🚀 Inicio rápido (Docker)

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

## 🎛️ Comandos

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

## ⚙️ Configuración (.env)

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

## 🍪 YouTube: cookies y PO token

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

## 🐳 Docker

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

## 🚨 Solución de problemas

| Síntoma (en logs) | Causa | Solución |
|---|---|---|
| `close code 4017 / DAVE protocol required` | Songbird sin soporte DAVE | Usar Songbird ≥ 0.6 (ya incluido) |
| `Sign in to confirm you're not a bot` / `LOGIN_REQUIRED` | Cookies ausentes/quemadas | Re-exportar cookies en incógnito → `config/cookies.txt` |
| `cookies are no longer valid, rotated in the browser` | Cookies exportadas de sesión activa | Exportar en incógnito y cerrar sin logout |
| `symphonia probe reach EOF at 0 bytes` | yt-dlp devolvió 0 bytes (bloqueo) | Mismo que arriba (cookies) |
| `DISCORD_TOKEN not found` | Falta el token | Configurar `.env` |

## 🧪 Desarrollo

Sin toolchain Rust local, todo se valida vía Docker:
```bash
docker build --target builder -t openmusic-check .   # valida compilación
cargo test    # (si tenés Rust local) tests de config y storage
```

## 📄 Licencia

MIT — ver [LICENSE](LICENSE).

---

<div align="center">

**🦀 Rust · Serenity & Songbird · ffmpeg · yt-dlp**

</div>
