# Calidad de audio en Open Music

## La realidad de Discord: no existe "MP3 320kbps"

Discord **no** transmite MP3. Toda la voz va en **Opus**, y la calidad está limitada por el
**bitrate del canal de voz**, que depende del nivel de boost del servidor:

| Nivel de servidor | Bitrate máximo del canal |
|---|---|
| Sin boost | 96 kbps |
| Boost Nivel 1 | **128 kbps** ← nuestro objetivo |
| Boost Nivel 2 | 256 kbps |
| Boost Nivel 3 / Stage | 384 kbps |

Pedir "320 kbps" no aplica a Discord: el valor no existe en la escala. Lo importante es otro:

> **Opus a 128 kbps suena igual o mejor que un MP3 a 320 kbps.**
> Opus es un códec mucho más eficiente; a 128k es percibido como transparente para música.

Nuestro objetivo realista y honesto: **maximizar la calidad dentro del techo de 128 kbps** del canal.

## Cómo se maximiza la calidad (lo que hace este bot)

1. **Fuente Opus, no AAC.** YouTube ofrece audio en Opus (~160 kbps, 48 kHz) y en AAC/m4a
   (~128 kbps, 44.1 kHz). Elegimos **Opus/48 kHz** porque:
   - Mayor bitrate de origen.
   - Mismo sample-rate que Discord (48 kHz) → **sin resample** 44.1→48 (el resample degrada).
   - Mismo códec base → menos pérdida en el transcode final.

2. **Bitrate del canal fijado al máximo.** El encoder Opus de songbird se configura a 128 kbps
   (env `OPUS_BITRATE`, por defecto 128000). Antes este valor era código muerto y no se aplicaba.

3. **Una sola pasada de transcode.** El pipeline decodifica el origen a PCM 48 kHz una vez,
   aplica filtros, y re-codifica a Opus una vez. Sin generaciones extra.

4. **Normalización de loudness (EBU R128 / loudnorm).** Todos los temas suenan a un volumen
   percibido consistente, sin tener que tocar el volumen entre canciones. Esta es la mejora de
   calidad percibida más grande para un bot de música.

5. **Ecualizador real.** Los presets aplican curvas de ecualización reales vía ffmpeg
   (antes solo cambiaban una etiqueta de texto sin efecto).

## Configuración relevante (env)

| Variable | Default | Efecto |
|---|---|---|
| `OPUS_BITRATE` | `128000` | Bitrate del encoder Opus. Subir a 256000/384000 solo si el servidor tiene boost N2/N3. |
| `DEFAULT_VOLUME` | `0.5` | Volumen base (0.0–2.0). |

Si el servidor sube de nivel de boost, basta con cambiar `OPUS_BITRATE` al nuevo techo.
