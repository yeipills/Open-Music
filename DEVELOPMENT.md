# Guía de desarrollo

Cómo está construido el bot por dentro y cómo trabajar en él. Si sólo querés
usarlo, el [README](README.md) alcanza. Si venís a mandar un cambio, leé también
[CONTRIBUTING.md](CONTRIBUTING.md).

## Índice

- [Arquitectura](#arquitectura)
- [Invariantes que no hay que romper](#invariantes-que-no-hay-que-romper)
- [Entorno de desarrollo](#entorno-de-desarrollo)
- [Configuración](#configuración)
- [Patrones del código](#patrones-del-código)
- [Depuración](#depuración)

## Arquitectura

| Módulo | Responsabilidad |
|---|---|
| `src/main.rs` | Arranque: configuración, almacenamiento, caché, cliente de serenity y songbird. |
| `src/config.rs` | Configuración desde variables de entorno, con validación. |
| `src/errors.rs` | `BotError`: errores de dominio con su texto en español. |
| `src/storage.rs` | Persistencia en JSON de los ajustes por servidor. |
| `src/bot/mod.rs` | `EventHandler` de serenity: arranque, interacciones y cambios de estado de voz. |
| `src/bot/handlers.rs` | Validación y despacho de los comandos. |
| `src/bot/commands.rs` | Definición y registro de los comandos slash. |
| `src/bot/connection.rs` | Estado de voz: en qué canal está cada quién. |
| `src/bot/search.rs` | Comando de búsqueda con menú de selección. |
| `src/audio/player.rs` | Operaciones sobre la cola de songbird y preferencias por servidor. |
| `src/audio/queue.rs` | Metadatos adjuntos a cada pista y vistas de sólo lectura para la interfaz. |
| `src/audio/events.rs` | Handlers de songbird: inicio de pista, fin, caída del driver e inactividad. |
| `src/audio/effects.rs` | Cadena de filtros de ffmpeg por servidor. |
| `src/sources/lazy.rs` | Fuente perezosa: implementa `Compose` lanzando `yt-dlp` y `ffmpeg` bajo demanda. |
| `src/sources/ytdlp_optimized.rs` | Búsqueda, metadatos, PO token y cookies. |
| `src/ui/embeds.rs`, `src/ui/buttons.rs` | Embeds y controles de Discord. |
| `src/cache/`, `src/monitoring/` | Caché LRU con expiración y métricas. |

El recorrido del audio, con diagrama, está en
[docs/AUDIO_PIPELINE.md](docs/AUDIO_PIPELINE.md).

### El flujo de un comando

1. `OpenMusicBot::interaction_create` recibe la interacción y llama a
   `handlers::dispatch_command`.
2. `run_command` aplica, en orden: límite de frecuencia, rol de DJ si el comando
   lo exige, limpieza de una conexión que Discord ya cerró, y la tabla de
   requisitos de voz.
3. El handler concreto opera sobre `bot.player`.
4. Si algo devuelve `Err`, `dispatch_command` responde con el texto del error.
   Ningún handler responde errores por su cuenta.

## Invariantes que no hay que romper

Estas reglas no son estilo: cada una existe porque romperla reintrodujo un fallo
concreto.

1. **La cola de songbird (`Call::queue()`) es la única fuente de verdad.** No
   crear colas paralelas ni guardar aparte cuál es la pista actual. Mantener dos
   colas sincronizadas a mano fue el origen de las canciones superpuestas y de
   los listados que no coincidían con lo que sonaba.

2. **Las conexiones de voz se consultan siempre a songbird**, con
   `player.call(guild_id)`. No cachear `Arc<Mutex<Call>>` en una estructura
   propia: en cuanto alguien echa al bot del canal, esa copia miente.

3. **`Call::leave()` no saca la conexión del manager.** Para descartar una
   conexión hay que usar `manager.remove()`; si no, `manager.get()` sigue
   devolviendo un `Call` muerto y el bot se cree conectado.

4. **Toda pista se crea en `AudioPlayer::build_track`**, que le adjunta un
   `Arc<QueueItem>` como *user data*. `queue::meta_of` entra en pánico si una
   pista se encoló de otra forma, así que esa función es la única puerta de
   entrada válida.

5. **Los `Input` deben ser perezosos** (`sources::lazy::LazyFfmpegSource`).
   Construir el audio al encolar lanzaría dos procesos por canción.

6. **Para saltar de pista, usar `player::force_skip_top_track`**: detener,
   `dequeue(0)` y reanudar, en ese orden.

7. **`TrackEvent::End` no significa "la canción terminó".** También se emite al
   detener una pista (`PlayMode::Stop`) y al fallar (`PlayMode::Errored`). Filtrar
   por `PlayMode::End` antes de tratarlo como un final natural, o vaciar la cola
   con la repetición activada reencolará lo que se acaba de borrar.

8. **Los comandos devuelven `BotError`**, y la respuesta de error vive sólo en
   `dispatch_command`.

## Entorno de desarrollo

### Requisitos

Para ejecutar el bot: Docker y Docker Compose. Nada más.

Para compilar sin Docker: Rust 1.82 o superior, `cmake`, `libopus-dev` (o
`opus-devel`), `pkg-config`, y en tiempo de ejecución `ffmpeg` y `yt-dlp`.

### Compilar y probar

Con Rust instalado:

```bash
cargo check            # comprobación rápida de tipos
cargo test             # tests unitarios y de integración
cargo clippy           # linter
cargo fmt              # formato
```

Sin Rust instalado, lo mismo dentro de un contenedor:

```bash
docker run --rm -v "$PWD":/app -w /app rust:1-bookworm bash -c \
  "apt-get update -qq && apt-get install -y -qq cmake libopus-dev pkg-config && cargo test"
```

Para iteraciones seguidas conviene montar volúmenes persistentes para el registro
de crates y el directorio `target`, o cada ejecución recompila todo:

```bash
docker run --rm -v "$PWD":/app -w /app \
  -v om-cargo-registry:/usr/local/cargo/registry -v om-target:/app/target \
  rust:1-bookworm cargo check
```

### Ejecutar

```bash
docker compose build
docker compose up -d
docker compose logs -f open-music
docker compose restart open-music   # por ejemplo, tras cambiar las cookies
```

El compose levanta dos contenedores: el bot y el proveedor de PO tokens, este
último accesible sólo desde la red interna.

Cuidado con el token: si el `.env` local apunta al mismo bot que está en
producción, arrancarlo en la máquina de desarrollo desconecta al de producción.
Para probar en local, usá una aplicación de Discord distinta.

## Configuración

Todas las variables se leen del entorno; `Config::load` valida los rangos y
aborta el arranque si algo no cuadra. La lista completa, con valores por defecto,
está en `.env.example`.

| Variable | Obligatoria | Notas |
|---|---|---|
| `DISCORD_TOKEN` | Sí | Token del bot. |
| `APPLICATION_ID` | Sí | Identificador de la aplicación. |
| `GUILD_ID` | No | Si se define, los comandos se registran sólo en ese servidor, lo que propaga en segundos en vez de en una hora. Útil al desarrollar. |
| `DEFAULT_VOLUME` | No | Entre 0.0 y 2.0. |
| `OPUS_BITRATE` | No | El techo real lo fija el nivel de boost del servidor. |
| `MAX_SONG_DURATION` | No | En segundos. |
| `CACHE_SIZE`, `AUDIO_CACHE_SIZE` | No | Entradas de la caché LRU. |
| `MAX_QUEUE_SIZE`, `MAX_PLAYLIST_SIZE` | No | Límites de la cola. |
| `RATE_LIMIT_PER_USER` | No | Comandos por minuto. |
| `POT_PROVIDER_URL` | No | Por defecto apunta al servicio del compose. |
| `RUST_LOG` | No | Filtro de trazas, por ejemplo `info,open_music=debug`. |

## Patrones del código

### Errores

Dentro de la capa de audio y de las fuentes se usa `anyhow::Result` con contexto.
En la capa de comandos se usa `BotError`, que es lo que ve el usuario:

```rust
let track = source_manager
    .get_track_from_url(&query, command.user.id)
    .await
    .map_err(|e| BotError::TrackFail(e.to_string()))?;
```

Para las comprobaciones previas existe `verify`, que sirve tanto con `bool` como
con `Option`:

```rust
verify(bot.player.is_playing(guild_id).await, BotError::NothingPlaying)?;
```

### Bloqueos

`Call` está detrás de un `tokio::sync::Mutex`. Hay que soltarlo antes de
responder a Discord: una llamada HTTP bajo el bloqueo detiene a todos los demás
comandos del servidor.

```rust
let handler = call.lock().await;
let queue = handler.queue().current_queue();
drop(handler);

responder(&queue).await?;
```

Los objetos de la caché de serenity (`ctx.cache.guild(..)`) no se pueden mantener
a través de un `await`. Hay que clonar lo que haga falta antes:

```rust
let guild = ctx.cache.guild(guild_id).ok_or(BotError::NotInGuild)?.clone();
```

### Interacciones que tardan

Discord corta la interacción a los tres segundos. Cualquier comando que lance
`yt-dlp` debe diferir primero y editar después:

```rust
defer(ctx, command).await?;
// ... trabajo lento ...
edit(ctx, command, "Listo").await
```

### Trabajo en segundo plano

Las tareas largas que no deben bloquear la respuesta van en un `tokio::spawn`,
registrando el error en vez de propagarlo. La carga del resto de una playlist en
`play_playlist_stream` es el ejemplo de referencia.

## Depuración

### Trazas

```bash
RUST_LOG=debug cargo run
RUST_LOG=open_music=trace,songbird=debug cargo run   # detalle de voz
docker compose logs -f open-music | grep -i error
```

### Problemas frecuentes al desarrollar

- **Los comandos no aparecen en Discord.** Los globales tardan hasta una hora en
  propagarse. Definí `GUILD_ID` para registrarlos en un servidor concreto.
- **El bot entra al canal pero no suena nada.** Casi siempre es yt-dlp: revisá si
  las cookies siguen siendo válidas. Ver [TROUBLESHOOTING.md](TROUBLESHOOTING.md).
- **Un comando responde "la interacción falló".** Faltó diferir antes de una
  operación lenta, o se respondió dos veces.

### Antes de mandar un cambio

```bash
cargo fmt
cargo clippy --all-targets
cargo test
docker compose build
```

## Recursos

- [Documentación de serenity](https://docs.rs/serenity)
- [Documentación de songbird](https://docs.rs/songbird)
- [Documentación de yt-dlp](https://github.com/yt-dlp/yt-dlp#readme)
- [Filtros de audio de ffmpeg](https://ffmpeg.org/ffmpeg-filters.html#Audio-Filters)
