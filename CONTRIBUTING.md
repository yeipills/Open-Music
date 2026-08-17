# Cómo contribuir

Gracias por querer aportar. Este documento cubre el trámite: cómo preparar el
entorno, cómo mandar un cambio y qué se espera de él. Cómo funciona el bot por
dentro está en [DEVELOPMENT.md](DEVELOPMENT.md).

## Índice

- [Antes de empezar](#antes-de-empezar)
- [Preparar el entorno](#preparar-el-entorno)
- [Flujo de trabajo](#flujo-de-trabajo)
- [Mensajes de commit](#mensajes-de-commit)
- [Estándares de código](#estándares-de-código)
- [Tests](#tests)
- [Revisión](#revisión)
- [Reportar errores](#reportar-errores)

## Antes de empezar

- Para un cambio grande, abrí primero una incidencia y comentá el enfoque. Es
  mejor discutir el diseño antes que después de escribir quinientas líneas.
- Para un arreglo pequeño o una corrección de documentación, mandá el pull
  request directamente.
- Los cambios en la capa de audio deben respetar las invariantes listadas en
  [DEVELOPMENT.md](DEVELOPMENT.md#invariantes-que-no-hay-que-romper). Están ahí
  porque saltárselas reintrodujo fallos concretos.

## Preparar el entorno

Necesitás Rust 1.82 o superior, o bien Docker si preferís no instalar nada. Las
dependencias de sistema para compilar son `cmake`, `libopus-dev` (`opus-devel` en
Fedora) y `pkg-config`; en tiempo de ejecución hacen falta `ffmpeg` y `yt-dlp`.

Los comandos exactos, con y sin Docker, están en
[DEVELOPMENT.md](DEVELOPMENT.md#entorno-de-desarrollo).

Para probar el bot necesitás tu propia aplicación de Discord y su token. No uses
un token que ya esté en producción.

## Flujo de trabajo

1. Bifurcá el repositorio y clonalo:

   ```bash
   git clone https://github.com/TU-USUARIO/Open-Music.git
   cd Open-Music
   git remote add upstream https://github.com/yeipills/Open-Music.git
   ```

2. Creá una rama con un nombre que diga qué hace:

   ```bash
   git checkout -b fix/reconexion-tras-expulsion
   ```

3. Hacé el cambio y verificá antes de commitear:

   ```bash
   cargo fmt
   cargo clippy --all-targets
   cargo test
   ```

4. Commiteá siguiendo la convención de la sección siguiente.

5. Traé lo último de `main` antes de publicar:

   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

6. Subí la rama y abrí el pull request.

### Antes de abrir el pull request

- `cargo fmt` sin cambios pendientes.
- `cargo clippy --all-targets` sin avisos nuevos.
- `cargo test` en verde.
- `docker compose build` completa, si tocaste dependencias o el Dockerfile.
- Documentación actualizada si el cambio altera el comportamiento visible.
- Tests para la lógica nueva que se pueda probar sin conexión a Discord.

## Mensajes de commit

Formato convencional:

```
tipo(ámbito): descripción corta en minúsculas

Explicación más detallada si hace falta, en uno o varios párrafos.

- Detalle relevante
- Otro detalle

Fixes #123
```

Tipos válidos:

| Tipo | Cuándo |
|---|---|
| `feat` | Funcionalidad nueva. |
| `fix` | Corrección de un fallo. |
| `docs` | Sólo documentación. |
| `style` | Formato, sin cambios de lógica. |
| `refactor` | Reestructuración sin cambiar el comportamiento. |
| `perf` | Mejora de rendimiento. |
| `test` | Añade o mejora tests. |
| `chore` | Mantenimiento, dependencias, configuración. |

El cuerpo del commit debe explicar **por qué**, no repetir lo que ya se ve en el
diff. Si arreglás un fallo, contá qué lo causaba.

## Estándares de código

- El código sigue `rustfmt` con la configuración por defecto.
- Nada de `unwrap()` ni `expect()` en rutas que dependan de datos externos:
  Discord, yt-dlp o el sistema de archivos. Devolvé un error y dejá que suba.
- Los comentarios explican por qué algo está hecho así, no qué hace la línea
  siguiente. Un comentario que repite el código sobra; uno que explica una
  decisión no obvia vale su peso en oro.
- Documentá con `///` los elementos públicos cuyo uso no sea evidente.
- El texto que ve el usuario va en español, con acentuación correcta.
- Sin emojis: ni en la interfaz, ni en los mensajes de registro, ni en la
  documentación.
- Los nombres de identificadores y los comentarios siguen el idioma que ya usa el
  archivo que estás tocando.

## Tests

El proyecto tiene tests unitarios de la configuración, del almacenamiento y de
las funciones puras de la capa de comandos. Todo lo que dependa de Discord o de
la red no se prueba de forma automática.

Al añadir lógica nueva, extraé la parte pura y probala. Por ejemplo, el análisis
de una marca de tiempo o el cálculo de una página de la cola son comprobables sin
levantar nada.

```bash
cargo test
cargo test -- --nocapture   # con la salida de las trazas
```

## Revisión

Al abrir el pull request, contá en la descripción:

- Qué problema resuelve y cómo lo verificaste.
- Si cambia el comportamiento visible del bot, en qué se nota.
- Si tocaste la capa de audio, qué invariantes revisaste.

Un cambio se acepta cuando compila sin avisos nuevos, los tests pasan, el
comportamiento está verificado y el código es coherente con el resto del
proyecto. Si algo no se puede probar sin un servidor de Discord real, decilo de
forma explícita en el pull request en vez de afirmar que funciona.

## Reportar errores

Una incidencia útil trae:

- Qué esperabas que pasara y qué pasó.
- Los pasos para reproducirlo.
- Las trazas relevantes (`docker compose logs open-music`), sin el token ni las
  cookies.
- Versión del bot, del sistema operativo y de yt-dlp.

Antes de abrirla, revisá [TROUBLESHOOTING.md](TROUBLESHOOTING.md): los problemas
con cookies de YouTube y con el bloqueo anti-bot ya están documentados ahí.

## Licencia

Al contribuir aceptás que tu aportación se publique bajo la licencia MIT del
proyecto.
