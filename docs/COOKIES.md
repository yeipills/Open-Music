# Cookies de YouTube — configuración y refresco

## Por qué hacen falta

YouTube bloquea las IPs de datacenter (como la del VPS) con el bot-check
**"Sign in to confirm you're not a bot"** (`playability status: LOGIN_REQUIRED`).
Ese bloqueo ocurre en el *player response* inicial, **antes** de la fase de streaming,
así que el PO Token provider por sí solo **no lo resuelve**: se necesitan cookies de una
sesión real de YouTube.

El bot combina dos mecanismos:
- **Cookies** (`config/cookies.txt`) → pasan el `LOGIN_REQUIRED`.
- **PO Token provider** (servicio `bgutil-provider` del compose) → robustez del streaming.

Ambos son necesarios. Ver el pipeline completo en [AUDIO_PIPELINE.md](./AUDIO_PIPELINE.md).

## ⚠️ Seguridad — leer antes

- Las cookies dan acceso a la cuenta de Google asociada. **Usar SIEMPRE una cuenta
  secundaria/desechable, nunca la cuenta personal.** Si se filtran, se compromete esa cuenta.
- `config/cookies.txt` está en `.gitignore` (`config/cookies.txt`) — **nunca** debe
  commitearse ni subirse a GitHub.
- Mantener la cuenta secundaria sin 2FA molesto y sin datos sensibles.

## Cómo exportar las cookies (MÉTODO INCÓGNITO — importante)

> ⚠️ **No exportes desde una sesión normal de navegador.** YouTube **rota** las
> cookies de sesiones activas como medida de seguridad, invalidando las que
> exportaste a los pocos minutos (síntoma: *"cookies are no longer valid, rotated
> in the browser"*). La solución es exportar desde una ventana de incógnito y
> **cerrarla sin hacer logout**, para que la sesión quede congelada y no se rote.

1. Abrir una **ventana de incógnito / privada**.
2. Iniciar sesión en **YouTube** con la **cuenta secundaria**.
3. Instalar (o tener habilitada en incógnito) la extensión
   **"Get cookies.txt LOCALLY"** (variante *LOCALLY*, no envía datos a terceros).
4. En `https://www.youtube.com`, abrir la extensión y **exportar** →
   descarga un `cookies.txt` en formato **Netscape**.
5. **Cerrar la ventana de incógnito SIN hacer logout.** (Esto congela la sesión:
   YouTube deja de rotar esas cookies y duran mucho más.)

## Cómo instalarlas en el VPS

El archivo va montado como volumen (`./config:/app/config` en `docker-compose.yml`),
así que basta reemplazarlo en el host. En la VPS `yeipi cantabo`:

```bash
# 1) Subir el cookies.txt nuevo a:
/home/yeipi/Open-Music/config/cookies.txt

# 2) Reiniciar SOLO el bot (no tocar otros servicios del host):
cd /home/yeipi/Open-Music
docker compose restart open-music
```

> El contenedor lee el archivo en cada invocación de yt-dlp, pero conviene reiniciar
> para descartar cachés/estado previo.

## Cómo verificar que funcionan

```bash
cd /home/yeipi/Open-Music

# Conteo de cookies reales (debe ser >> 8; el placeholder tenía ~8):
grep -c youtube.com config/cookies.txt

# Prueba de descarga real (cookies + provider), como lo hace el bot.
# Debe imprimir bytes (>0) y NO mostrar "Sign in" ni "LOGIN_REQUIRED":
docker compose exec -T open-music sh -c \
  'yt-dlp --ignore-config --cookies /app/config/cookies.txt \
   --extractor-args "youtubepot-bgutilhttp:base_url=http://bgutil-provider:4416" \
   -f "bestaudio[acodec=opus]/bestaudio/best" -o - --no-playlist --no-warnings \
   "https://www.youtube.com/watch?v=kJQP7kiw5Fk" 2>/tmp/e | head -c 100000 | wc -c; \
   grep -iE "sign in|login_required" /tmp/e || echo "OK sin bloqueo"'
```

(El `Broken pipe` al cortar con `head` es benigno: solo significa que se cerró el stream.)

Prueba final: en Discord, entrar a un canal de voz y usar `/play <tema>`. En los logs
debe aparecer `🎵 Reproduciendo: ...` sin `EOF 0 bytes` ni `no suitable format reader`.

## Cuándo refrescarlas

Las cookies de YouTube **expiran** (semanas a pocos meses) o se invalidan si la cuenta
inicia sesión en otro lado. Síntoma típico en los logs cuando caducan:

```
ERROR: [youtube] ...: Sign in to confirm you're not a bot
web player response playability status: LOGIN_REQUIRED
symphonia probe reach EOF at 0 bytes
```

Cuando eso aparezca: **re-exportar** las cookies de la cuenta secundaria (pasos de arriba),
reemplazar `config/cookies.txt` y `docker compose restart open-music`. Nada más del stack
necesita mantenimiento.
