//! Fuente de audio **perezosa**: `yt-dlp | ffmpeg` que no gasta nada hasta que
//! songbird decide que le toca sonar.
//!
//! ## Por qué esto es el corazón del bot
//!
//! La cola nativa de songbird ([`songbird::tracks::TrackQueue`]) guarda `Track`s
//! ya construidos. Si cada `Track` llevara un [`Input::Live`] —es decir, los dos
//! procesos ya lanzados— encolar una playlist de 50 temas lanzaría 100 procesos
//! de golpe y el bot moriría por OOM. Por eso antes hacía falta una cola propia
//! que guardaba metadatos y construía el audio recién al reproducir.
//!
//! Implementando [`Compose`] el problema desaparece: el `Track` guarda sólo la
//! URL y la receta. songbird llama a [`Compose::create`] cuando la pista pasa a
//! ser la actual (o unos segundos antes, para reproducción sin huecos), y recién
//! ahí se lanzan `yt-dlp` y `ffmpeg`. Así se puede usar la cola nativa —con su
//! avance automático ya probado— sin renunciar al ecualizador.
//!
//! Efecto secundario deseable: el filtro de ffmpeg se resuelve **en el momento
//! de crear el stream**, no al encolar. Cambiar el preset del ecualizador afecta
//! por tanto a todas las pistas que aún no arrancaron.

use songbird::input::{
    core::io::{MediaSource, ReadOnlySource},
    AudioStream, AudioStreamError, AuxMetadata, ChildContainer, Compose, Input,
};
use std::{
    process::{Command, Stdio},
    sync::Arc,
    time::Duration,
};
use tracing::{info, warn};

use super::{ytdlp_optimized::pot_extractor_arg, TrackSource, YtDlpOptimizedClient};

/// Proveedor de la cadena de filtros de ffmpeg (`-af`).
///
/// Es un callback y no un `String` fijo justamente para que el valor se lea al
/// crear el stream: así `/equalizer` se nota en la próxima canción sin tener que
/// tocar nada de la cola.
pub type FilterProvider = Arc<dyn Fn() -> String + Send + Sync>;

/// Receta para producir el audio de una pista, ejecutada bajo demanda.
pub struct LazyFfmpegSource {
    url: String,
    title: String,
    filter: FilterProvider,
    /// Posición inicial de reproducción (usado por `/seek`).
    seek: Option<Duration>,
    metadata: AuxMetadata,
}

impl LazyFfmpegSource {
    pub fn new(track: &TrackSource, filter: FilterProvider) -> Self {
        Self {
            url: track.url(),
            title: track.title(),
            filter,
            seek: None,
            metadata: aux_metadata_from(track),
        }
    }

    /// Igual que [`Self::new`] pero arrancando en `position`.
    pub fn seeking(track: &TrackSource, filter: FilterProvider, position: Duration) -> Self {
        let mut source = Self::new(track, filter);
        source.seek = Some(position);
        source
    }

    /// Lanza `yt-dlp | ffmpeg` y devuelve el contenedor de procesos.
    ///
    /// `ChildContainer` lee del **último** proceso de la lista (ffmpeg) y mata
    /// toda la cadena al hacer drop, así que no quedan `yt-dlp` huérfanos cuando
    /// se salta una canción.
    fn spawn_pipeline(&self) -> anyhow::Result<ChildContainer> {
        if !YtDlpOptimizedClient::is_youtube_url(&self.url) {
            anyhow::bail!("solo se soportan URLs de YouTube: {}", self.url);
        }

        let cookies = YtDlpOptimizedClient::cookies_working_copy();
        let pot_arg = pot_extractor_arg();

        // Se intenta primero por `music.youtube.com` y sólo si falla por el
        // dominio normal. No es un capricho: YouTube aplica su degradación
        // anti-datacenter **por dominio**. Medido en el servidor con la misma
        // IP, las mismas cookies y el mismo vídeo, en un momento en que
        // youtube.com estaba bloqueado:
        //
        //   youtube.com        -> 0 formatos de sólo audio (sólo HLS de vídeo)
        //   music.youtube.com  -> 4 formatos de sólo audio, descarga correcta
        //
        // El encadenado con `||` hace de reserva sin coste cuando el primero
        // funciona: si el vídeo no está en el catálogo de Música —los no
        // musicales no lo están— yt-dlp sale con error sin haber emitido nada y
        // el shell lanza el segundo intento sobre la misma tubería.
        let url_music = a_youtube_music(&self.url);
        let orden = format!(
            "{} || {}",
            comando_ytdlp(&url_music, cookies.as_deref(), &pot_arg),
            comando_ytdlp(&self.url, cookies.as_deref(), &pot_arg),
        );

        let mut ytdlp_cmd = Command::new("sh");
        ytdlp_cmd
            .arg("-c")
            .arg(&orden)
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut ytdlp = ytdlp_cmd
            .spawn()
            .map_err(|e| anyhow::anyhow!("no se pudo lanzar yt-dlp: {e}"))?;
        let ytdlp_stdout = ytdlp
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("yt-dlp sin stdout"))?;

        let filter = (self.filter)();
        let mut ffmpeg_cmd = Command::new("ffmpeg");
        ffmpeg_cmd.args([
            "-hide_banner",
            "-loglevel",
            "error",
            // Arrancar en cuanto haya datos. Por defecto ffmpeg acumula hasta
            // 5 MB (`probesize`) o 5 s (`analyzeduration`) antes de emitir nada:
            // sobre una tubería que se está descargando en directo, eso son
            // varios segundos de silencio antes de la primera nota. El formato
            // ya lo imponemos nosotros con `-f`, así que no hace falta sondearlo.
            //
            // Deliberadamente **no** se usa `-fflags nobuffer`: reduce el búfer
            // interno durante toda la reproducción, no sólo al principio, y ante
            // cualquier irregularidad de la descarga provoca cortes audibles.
            // La latencia baja se gana sondeando menos, no reproduciendo sin red.
            "-analyzeduration",
            "0",
            "-probesize",
            "32768",
            "-i",
            "pipe:0",
        ]);
        // `-ss` va **después** de `-i`: con una tubería el seek rápido no es
        // fiable, y el seek de salida sí es exacto (decodifica y descarta).
        if let Some(pos) = self.seek {
            ffmpeg_cmd.args(["-ss", &format!("{:.3}", pos.as_secs_f64())]);
        }
        ffmpeg_cmd
            .args([
                "-af", &filter, "-ac", "2", "-ar", "48000", "-f", "wav", "pipe:1",
            ])
            .stdin(Stdio::from(ytdlp_stdout))
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let ffmpeg = ffmpeg_cmd
            .spawn()
            .map_err(|e| anyhow::anyhow!("no se pudo lanzar ffmpeg: {e}"))?;

        info!("Stream abierto: {} (filtro: {})", self.title, filter);
        Ok(ChildContainer::new(vec![ytdlp, ffmpeg]))
    }
}

#[async_trait::async_trait]
impl Compose for LazyFfmpegSource {
    fn create(&mut self) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        match self.spawn_pipeline() {
            Ok(container) => Ok(AudioStream {
                input: Box::new(ReadOnlySource::new(container)) as Box<dyn MediaSource>,
            }),
            Err(e) => {
                warn!("No se pudo abrir el stream de «{}»: {e}", self.title);
                Err(AudioStreamError::Fail(e.into()))
            }
        }
    }

    async fn create_async(
        &mut self,
    ) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        // Se usa `create` (síncrono) sobre el pool de hilos bloqueantes de
        // songbird: lanzar procesos es una operación bloqueante corta.
        Err(AudioStreamError::Unsupported)
    }

    fn should_create_async(&self) -> bool {
        false
    }

    async fn aux_metadata(&mut self) -> Result<AuxMetadata, AudioStreamError> {
        Ok(self.metadata.clone())
    }
}

impl From<LazyFfmpegSource> for Input {
    fn from(val: LazyFfmpegSource) -> Self {
        Input::Lazy(Box::new(val))
    }
}

/// Traduce nuestros metadatos a los de songbird.
///
/// La duración importa: la cola nativa la usa para precargar la pista siguiente
/// 5 segundos antes del final y encadenar sin silencio.
fn aux_metadata_from(track: &TrackSource) -> AuxMetadata {
    AuxMetadata {
        title: Some(track.title()),
        artist: track.artist(),
        duration: track.duration(),
        thumbnail: track.thumbnail(),
        source_url: Some(track.url()),
        ..AuxMetadata::default()
    }
}

/// Reescribe una URL de YouTube a su equivalente en YouTube Music.
///
/// Sólo cambia el anfitrión: el identificador del vídeo es el mismo en ambos
/// sitios. Si la URL no es reconocible se devuelve intacta, y el intento por
/// Música simplemente fallará y se usará la de reserva.
fn a_youtube_music(url: &str) -> String {
    url.replace("://www.youtube.com/", "://music.youtube.com/")
        .replace("://youtube.com/", "://music.youtube.com/")
        .replace("://m.youtube.com/", "://music.youtube.com/")
}

/// Escapa un valor para incrustarlo en una orden de shell.
fn entrecomillar(valor: &str) -> String {
    format!("'{}'", valor.replace('\'', "'\\''"))
}

/// Construye la orden de yt-dlp que vuelca el audio por la salida estándar.
fn comando_ytdlp(url: &str, cookies: Option<&str>, pot_arg: &str) -> String {
    let mut orden = String::from("yt-dlp --ignore-config");
    // Sólo audio progresivo, y nunca vídeo. El selector anterior terminaba en
    // `best`, que acepta cualquier formato: cuando YouTube degrada la respuesta
    // y deja sólo HLS, yt-dlp se ponía a bajar decenas de megas de vídeo
    // fragmentado. HLS no se puede transmitir por una tubería —yt-dlp junta los
    // fragmentos antes de emitir—, así que ffmpeg recibía cero bytes y la
    // canción quedaba en silencio. Excluyendo `m3u8` y sin recurso a vídeo, en
    // ese caso yt-dlp falla de inmediato y el fallo se ve.
    orden.push_str(" -f 'bestaudio[acodec=opus][protocol!*=m3u8]/bestaudio[ext=webm][protocol!*=m3u8]/bestaudio[protocol!*=m3u8]'");
    orden.push_str(" -o - --no-playlist --no-check-certificate --geo-bypass --force-ipv4");
    // No verificar formatos: ya elegimos uno concreto con -f y cada
    // verificación es una petición extra que retrasa el arranque.
    orden.push_str(" --no-check-formats --quiet");
    orden.push_str(&format!(" --extractor-args {}", entrecomillar(pot_arg)));
    if let Some(c) = cookies {
        orden.push_str(&format!(" --cookies {}", entrecomillar(c)));
    }
    orden.push_str(&format!(" {}", entrecomillar(url)));
    orden
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reescribe_a_youtube_music() {
        assert_eq!(
            a_youtube_music("https://www.youtube.com/watch?v=abc123"),
            "https://music.youtube.com/watch?v=abc123"
        );
        assert_eq!(
            a_youtube_music("https://youtube.com/watch?v=abc123"),
            "https://music.youtube.com/watch?v=abc123"
        );
        // Una URL ya de Música se queda igual, no se duplica el anfitrión.
        assert_eq!(
            a_youtube_music("https://music.youtube.com/watch?v=abc123"),
            "https://music.youtube.com/watch?v=abc123"
        );
    }

    #[test]
    fn entrecomillado_resiste_comillas() {
        assert_eq!(entrecomillar("simple"), "'simple'");
        // Una comilla simple dentro del valor no debe poder cerrar la cadena.
        assert_eq!(entrecomillar("a'b"), r#"'a'\''b'"#);
    }

    #[test]
    fn la_orden_lleva_reserva_y_no_pide_video() {
        let orden = format!(
            "{} || {}",
            comando_ytdlp("https://music.youtube.com/watch?v=x", None, "pot=1"),
            comando_ytdlp("https://www.youtube.com/watch?v=x", None, "pot=1"),
        );
        assert!(orden.contains("music.youtube.com"));
        assert!(orden.contains("||"));
        // Nunca debe poder caer a vídeo ni aceptar HLS.
        assert!(!orden.contains("/best'"));
        assert!(orden.contains("protocol!*=m3u8"));
    }
}
