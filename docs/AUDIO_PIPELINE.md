# Pipeline de audio

Cómo viaja el sonido desde una búsqueda hasta el canal de voz. Para el contexto
de calidad (por qué Opus a 128 kbps), ver [AUDIO_QUALITY.md](./AUDIO_QUALITY.md).

## Diagrama

```
/play <búsqueda | url>
        |
        v
  yt-dlp (búsqueda o metadatos)  ->  TrackSource (título, url, duración, autor)
        |
        v
  AudioPlayer::play()
        |
        |  Track::new_with_data(LazyFfmpegSource, Arc<QueueItem>)
        v
  cola nativa de songbird (TrackQueue)
        |
        |  songbird llama a Compose::create cuando la pista pasa a ser la
        |  actual, o unos segundos antes si conoce su duración (precarga)
        v
  yt-dlp -o -  |  ffmpeg -af "loudnorm,<eq>" -ar 48000 -ac 2 -f wav pipe:1
        |
        v
  ChildContainer  ->  symphonia  ->  encoder Opus  ->  canal de voz (DAVE)
```

## Componentes

| Componente | Archivo | Responsabilidad |
|---|---|---|
| `AudioPlayer` | `src/audio/player.rs` | Traduce los comandos a operaciones sobre la cola de songbird. No guarda estado de reproducción. |
| `QueueItem` | `src/audio/queue.rs` | Metadatos adjuntos a cada pista como *user data*, más las vistas que consume la interfaz. |
| Handlers | `src/audio/events.rs` | Reaccionan a los eventos de songbird: inicio de pista, fin, caída del driver e inactividad. |
| `AudioEffects` | `src/audio/effects.rs` | Construye la cadena de filtros de ffmpeg (`loudnorm` más el preset). |
| `LazyFfmpegSource` | `src/sources/lazy.rs` | Implementa `Compose`: lanza `yt-dlp` y `ffmpeg` bajo demanda. |
| `YtDlpOptimizedClient` | `src/sources/ytdlp_optimized.rs` | Búsqueda, metadatos, PO token y cookies. |

## Reglas de diseño

1. **La cola de songbird es la única cola.** No existe una estructura de cola
   propia. El avance de pista lo hace songbird al recibir `TrackEvent::End`, que
   también se emite cuando una pista falla, de modo que una canción rota no
   detiene la reproducción.

2. **Los `Input` son perezosos.** Un `Track` guarda la URL y la receta, no los
   procesos. Encolar doscientos temas no cuesta más que encolar uno; los procesos
   se lanzan cuando a la pista le toca sonar. Construir el audio al encolar
   levantaría dos procesos por canción y agotaría la memoria.

3. **Los efectos viven en la cadena de ffmpeg**, no en Rust. ffmpeg ya está en la
   imagen, así que no se usan bibliotecas de proceso de señal.

   Como consecuencia de la regla 2, el filtro se resuelve al abrir el stream:
   cambiar el preset con `/equalizer` afecta a todas las pistas que aún no han
   arrancado, no sólo a las que se encolen después.

4. **El bitrate se fija por conexión**, en `join_voice_channel`
   (`handler.set_bitrate`), a partir de `config.opus_bitrate`. El techo real lo
   impone el nivel de boost del servidor de Discord.

5. **Saltar de pista tiene un orden obligatorio**, en
   `player::force_skip_top_track`: detener la actual, sacarla de la cola a mano y
   reanudar. Detenerla no la retira de la cola de inmediato, y retirarla no
   arranca la siguiente; en cualquier otro orden quedan dos pistas sonando a la
   vez o la cola parada con canciones dentro.

## Formato

- Origen preferido: Opus en contenedor WebM a 48 kHz, que evita un remuestreo y
  una segunda pérdida de calidad.
- Alternativa: AAC/m4a u otro formato si Opus no está disponible.
- ffmpeg entrega WAV por una tubería. Al no ser un flujo con posiciones, `/seek`
  no rebobina: vuelve a abrir el stream con `-ss`.
- yt-dlp se invoca con `--ignore-config` para no heredar la configuración global
  del sistema, que aplicaría opciones de descarga a cada llamada.
