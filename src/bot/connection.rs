//! Estado de conexión de voz: quién está dónde.
//!
//! Toda la validación de "¿puede este usuario ejecutar este comando?" se reduce
//! a comparar dos canales: el del usuario y el del bot. Modelarlo como un enum
//! cerrado evita la maraña de `if` sueltos repartidos por cada handler y hace
//! imposible olvidarse de un caso.

use serenity::model::{
    guild::Guild,
    id::{ChannelId, UserId},
};

/// Relación entre el canal de voz del usuario y el del bot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connection {
    /// Sólo el usuario está en un canal de voz.
    User(ChannelId),
    /// Sólo el bot está en un canal de voz.
    Bot(ChannelId),
    /// Ambos, en el **mismo** canal. `(canal_bot, canal_usuario)`.
    Mutual(ChannelId, ChannelId),
    /// Ambos, en canales **distintos**. `(canal_bot, canal_usuario)`.
    Separate(ChannelId, ChannelId),
    /// Ninguno de los dos está en un canal de voz.
    Neither,
}

/// Clasifica la situación de voz de una guild para un usuario dado.
pub fn check_voice_connections(guild: &Guild, user_id: &UserId, bot_id: &UserId) -> Connection {
    let user_channel = get_voice_channel_for_user(guild, user_id);
    let bot_channel = get_voice_channel_for_user(guild, bot_id);

    match (bot_channel, user_channel) {
        (Some(bot), Some(user)) if bot == user => Connection::Mutual(bot, user),
        (Some(bot), Some(user)) => Connection::Separate(bot, user),
        (Some(bot), None) => Connection::Bot(bot),
        (None, Some(user)) => Connection::User(user),
        (None, None) => Connection::Neither,
    }
}

/// Canal de voz en el que está un usuario, si está en alguno.
///
/// Se lee de `guild.voice_states`, que serenity mantiene actualizado con los
/// eventos `VOICE_STATE_UPDATE`; es la fuente de verdad más barata y no
/// requiere ninguna llamada HTTP.
pub fn get_voice_channel_for_user(guild: &Guild, user_id: &UserId) -> Option<ChannelId> {
    guild
        .voice_states
        .get(user_id)
        .and_then(|voice_state| voice_state.channel_id)
}
