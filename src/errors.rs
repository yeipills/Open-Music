//! Errores de dominio del bot.
//!
//! Todo comando devuelve `Result<(), BotError>`. El despachador
//! ([`crate::bot::handlers::handle_command`]) es el **único** sitio que responde
//! al usuario en caso de fallo: cada variante sabe cómo se cuenta en español vía
//! [`Display`]. Así no hay que repetir el mismo `create_response` de error en
//! cada handler, ni se pierde un error por olvidarse de contestar.

use serenity::{model::mention::Mention, prelude::SerenityError};
use std::fmt::{self, Debug, Display};

#[derive(Debug)]
pub enum BotError {
    /// Mensaje fijo, conocido en tiempo de compilación.
    Other(&'static str),
    /// Mensaje construido en runtime.
    Dynamic(String),

    // ---- Estado de conexión de voz ----
    /// El comando se usó por DM o fuera de un servidor.
    NotInGuild,
    /// El bot no está en ningún canal de voz.
    NotConnected,
    /// Quien invoca no está en ningún canal de voz.
    AuthorNotFound,
    /// El bot está en un canal y quien invoca en ninguno.
    AuthorDisconnected(Mention),
    /// Ambos están en canales distintos.
    WrongVoiceChannel,
    /// El bot ya está ocupado en otro canal.
    AlreadyConnected(Mention),

    // ---- Estado de reproducción ----
    NothingPlaying,
    QueueEmpty,
    NotInRange(&'static str, isize, isize, isize),
    NoHistory,
    TrackFail(String),

    // ---- Permisos y límites ----
    RateLimited,
    DjRequired,

    // ---- Errores ajenos ----
    Serenity(Box<SerenityError>),
    Anyhow(anyhow::Error),
}

impl std::error::Error for BotError {}

impl Display for BotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Other(msg) => f.write_str(msg),
            Self::Dynamic(msg) => f.write_str(msg),

            Self::NotInGuild => f.write_str("❌ Este comando solo funciona dentro de un servidor"),
            Self::NotConnected => f.write_str("❌ No estoy conectado a ningún canal de voz"),
            Self::AuthorNotFound => {
                f.write_str("❌ Tenés que estar en un canal de voz para usar este comando")
            }
            Self::AuthorDisconnected(mention) => f.write_fmt(format_args!(
                "❌ Estoy reproduciendo en {mention} — unite a ese canal para controlarme"
            )),
            Self::WrongVoiceChannel => {
                f.write_str("❌ Tenés que estar en el mismo canal de voz que yo")
            }
            Self::AlreadyConnected(mention) => {
                f.write_fmt(format_args!("❌ Ya estoy conectado en {mention}"))
            }

            Self::NothingPlaying => f.write_str("❌ No hay nada reproduciéndose"),
            Self::QueueEmpty => f.write_str("❌ La cola está vacía"),
            Self::NotInRange(param, value, lower, upper) => f.write_fmt(format_args!(
                "❌ `{param}` debe estar entre {lower} y {upper}, pero fue {value}"
            )),
            Self::NoHistory => f.write_str("❌ No hay canciones anteriores en el historial"),
            Self::TrackFail(err) => {
                if err.is_empty() {
                    f.write_str("❌ No se encontró la canción")
                } else {
                    f.write_fmt(format_args!("❌ {err}"))
                }
            }

            Self::RateLimited => f.write_str(
                "⏳ Estás enviando comandos muy rápido. Esperá unos segundos.",
            ),
            Self::DjRequired => f.write_str("🎧 Este comando requiere el rol de DJ"),

            Self::Serenity(err) => f.write_fmt(format_args!("❌ Error de Discord: {err}")),
            Self::Anyhow(err) => f.write_fmt(format_args!("❌ {err}")),
        }
    }
}

impl From<SerenityError> for BotError {
    fn from(err: SerenityError) -> Self {
        Self::Serenity(Box::new(err))
    }
}

impl From<anyhow::Error> for BotError {
    fn from(err: anyhow::Error) -> Self {
        Self::Anyhow(err)
    }
}

impl From<std::io::Error> for BotError {
    fn from(err: std::io::Error) -> Self {
        Self::Dynamic(err.to_string())
    }
}

/// Tipos que se pueden evaluar como verdadero/falso y desempaquetar.
///
/// Permite escribir `verify(queue.current(), BotError::NothingPlaying)?` tanto
/// para `bool` como para `Option<T>`, obteniendo el valor interno cuando aplica.
pub trait Verifiable<T> {
    fn to_bool(&self) -> bool;
    fn unpack(self) -> T;
}

impl Verifiable<bool> for bool {
    fn to_bool(&self) -> bool {
        *self
    }
    fn unpack(self) -> bool {
        self
    }
}

impl<T> Verifiable<T> for Option<T> {
    fn to_bool(&self) -> bool {
        self.is_some()
    }
    fn unpack(self) -> T {
        // Sólo se llama tras `to_bool()`, así que nunca es `None`.
        self.expect("verify: unpack sobre None")
    }
}

/// Devuelve el valor si la condición se cumple, o `err` si no.
pub fn verify<K, T: Verifiable<K>>(verifiable: T, err: BotError) -> Result<K, BotError> {
    if verifiable.to_bool() {
        Ok(verifiable.unpack())
    } else {
        Err(err)
    }
}

pub type BotResult<T> = Result<T, BotError>;
