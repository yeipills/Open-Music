# Pipeline de audio — arquitectura

> Documento vivo. Refleja el estado **objetivo** tras el refactor de unificación + calidad.
> Para el contexto de calidad (por qué Opus 128k), ver [AUDIO_QUALITY.md](./AUDIO_QUALITY.md).

## Diagrama del pipeline (por track)

```
Usuario: /play <query>
        │
        ▼
yt-dlp (búsqueda)  ──►  TrackSource (título, url, duración, autor)
        │
        ▼
AudioPlayer.play()  ──►  MusicQueue (encola)
        │
        ▼ (al iniciar / auto-avance)
yt-dlp -g  (formato: bestaudio[acodec=opus] / webm / best)  ──►  URL directa googlevideo
        │
        ▼
ffmpeg -i <url> -af "loudnorm,<eq del preset>" -ar 48000 -ac 2 -f s16le -
        │  (PCM 48 kHz estéreo, 1 sola pasada de transcode)
        ▼
songbird Input  ──►  encoder Opus @128 kbps  ──►  canal de voz Discord
```

## Componentes

| Componente | Archivo | Responsabilidad |
|---|---|---|
| `AudioPlayer` | `src/audio/player.rs` | **Único motor.** Cola, reproducción, auto-avance, volumen, eventos. |
| `MusicQueue` | `src/audio/queue.rs` | Estructura de cola (shuffle, loop, historial). |
| `AudioEffects` | `src/audio/effects.rs` | Construye la cadena de filtros ffmpeg (loudnorm + EQ por preset). |
| `TrackSource` | `src/sources/mod.rs` | Metadatos del track y creación del `Input` de audio. |
| `YtDlpOptimizedClient` | `src/sources/ytdlp_optimized.rs` | Búsqueda y extracción de URL directa con yt-dlp. |

## Reglas del diseño

1. **Un solo motor.** Todos los comandos (`/play /pause /skip /queue /add …`) operan sobre
   `bot.player` (`AudioPlayer`). No hay un segundo motor paralelo.
2. **El auto-avance es responsabilidad del `TrackEndHandler`**, que llama a `play_next` al
   terminar cada track. Sin esto, la cola no avanza.
3. **Los efectos viven en la cadena ffmpeg**, no en Rust. ffmpeg ya está en la imagen Docker;
   no se usan crates de DSP (`fundsp`/`rubato` fueron eliminados).
4. **El bitrate se fija una vez** al construir el cliente songbird
   (`Songbird::serenity_from_config`), desde `config.opus_bitrate`.

## Decodificación y formato

- Origen preferido: **Opus en contenedor WebM, 48 kHz** (evita resample y doble pérdida).
- Fallback: AAC/m4a u otros si Opus no está disponible.
- yt-dlp se invoca con `--ignore-config` para no heredar el `~/.config/yt-dlp/config` global
  (que aplicaba flags de descarga a toda invocación).

## Historial

- **Lavalink eliminado** (`refactor: eliminar capa Lavalink`): la capa "híbrida" Lavalink nunca
  llegó a usarse de verdad; se quitó junto con ~2.9k líneas de código muerto.
- **Unificación de motor**: antes coexistían `AudioPlayer` y `AudioManager` con estado divergido
  (la cola salía vacía y no había auto-avance). Se unificó en `AudioPlayer`.
