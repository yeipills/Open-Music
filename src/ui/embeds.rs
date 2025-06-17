use serenity::{
    all::{Colour, Timestamp},
    builder::{CreateEmbed, CreateEmbedFooter},
};
use std::time::Duration;

use crate::{
    audio::queue::{LoopMode, QueueInfo, QueueItem},
    sources::TrackSource,
};

/// Crea un embed para mostrar la canción actual
pub fn create_now_playing_embed(track: &QueueItem) -> CreateEmbed {
    let mut embed = CreateEmbed::default()
        .title("🎵 Reproduciendo Ahora")
        .color(Colour::from_rgb(0, 255, 127)) // Verde brillante
        .field("Título", &track.title, true);

    if let Some(artist) = &track.artist {
        embed = embed.field("Artista", artist, true);
    }

    if let Some(duration) = track.duration {
        embed = embed.field("Duración", format_duration(duration), true);
    }

    embed = embed.field("Solicitado por", format!("<@{}>", track.requested_by), true);

    if let Some(thumbnail) = &track.thumbnail {
        embed = embed.thumbnail(thumbnail);
    }

    embed = embed
        .url(&track.url)
        .timestamp(Timestamp::now())
        .footer(CreateEmbedFooter::new("Open Music Bot"));

    embed
}

/// Crea un embed para mostrar que se agregó una canción
pub fn create_track_added_embed(track: &TrackSource) -> CreateEmbed {
    let mut embed = CreateEmbed::default()
        .title("✅ Canción Agregada")
        .color(Colour::from_rgb(0, 255, 0))
        .field("Título", track.title(), true);

    if let Some(artist) = track.artist() {
        embed = embed.field("Artista", &artist, true);
    }

    if let Some(duration) = track.duration() {
        embed = embed.field("Duración", format_duration(duration), true);
    }

    embed = embed.field(
        "Solicitado por",
        format!("<@{}>", track.requested_by()),
        true,
    );

    if let Some(thumbnail) = track.thumbnail() {
        embed = embed.thumbnail(&thumbnail);
    }

    embed = embed
        .url(&track.url())
        .timestamp(Timestamp::now())
        .footer(CreateEmbedFooter::new("Open Music Bot"));

    embed
}

/// Crea un embed para mostrar la cola de reproducción
pub fn create_queue_embed(queue_info: &QueueInfo, page: usize) -> CreateEmbed {
    let items_per_page = 10;
    let queue_page = queue_info.get_page(page, items_per_page);

    let mut embed = CreateEmbed::default()
        .title("📜 Cola de Reproducción")
        .color(Colour::from_rgb(0, 191, 255));

    if queue_info.total_items == 0 {
        return embed
            .description("La cola está vacía")
            .footer(CreateEmbedFooter::new("Open Music Bot"));
    }

    // Canción actual
    if let Some(current) = &queue_info.current {
        let status = match queue_info.loop_mode {
            LoopMode::Track => "🔂",
            LoopMode::Queue => "🔁",
            LoopMode::Off => "▶️",
        };

        embed = embed.field(
            format!("{} Reproduciendo", status),
            format!(
                "**{}**{}",
                current.title,
                if let Some(artist) = &current.artist {
                    format!(" - {}", artist)
                } else {
                    String::new()
                }
            ),
            false,
        );
    }

    // Próximas canciones
    if !queue_page.items.is_empty() {
        let mut description = String::new();

        for (i, item) in queue_page.items.iter().enumerate() {
            let position = (page - 1) * items_per_page + i + 1;
            let duration = if let Some(dur) = item.duration {
                format!(" `[{}]`", format_duration(dur))
            } else {
                String::new()
            };

            description.push_str(&format!(
                "**{}**. {}{}{}\n",
                position,
                item.title,
                if let Some(artist) = &item.artist {
                    format!(" - {}", artist)
                } else {
                    String::new()
                },
                duration
            ));
        }

        embed = embed.field("Próximas canciones", description, false);
    }

    // Información adicional
    let mut info = format!("**Total:** {} canciones", queue_info.total_items);

    if queue_info.total_duration > Duration::ZERO {
        info.push_str(&format!(
            " • **Duración:** {}",
            format_duration(queue_info.total_duration)
        ));
    }

    if queue_info.shuffle {
        info.push_str(" • 🔀 **Aleatorio**");
    }

    embed = embed.field("Información", info, false);

    // Paginación
    if queue_page.total_pages > 1 {
        embed = embed.footer(CreateEmbedFooter::new(format!(
            "Página {} de {} • Open Music Bot",
            queue_page.current_page, queue_page.total_pages
        )));
    } else {
        embed = embed.footer(CreateEmbedFooter::new("Open Music Bot"));
    }

    embed.timestamp(Timestamp::now())
}

/// Crea un embed de ayuda general
pub fn create_help_embed() -> CreateEmbed {
    CreateEmbed::default()
        .title("🎵 Open Music Bot - Ayuda")
        .color(Colour::from_rgb(0, 123, 255))
        .description("Bot de música de alto rendimiento con soporte para múltiples plataformas")
        .field(
            "🎵 Reproducción",
            "• `/play <canción>` - Reproduce una canción\n\
            • `/pause` - Pausa la reproducción\n\
            • `/resume` - Reanuda la reproducción\n\
            • `/skip [cantidad]` - Salta canciones\n\
            • `/stop` - Detiene y limpia la cola",
            false,
        )
        .field(
            "📜 Cola",
            "• `/queue [página]` - Muestra la cola\n\
            • `/shuffle` - Activa/desactiva aleatorio\n\
            • `/loop <modo>` - Configura repetición\n\
            • `/clear [filtro]` - Limpia la cola",
            false,
        )
        .field(
            "🎛️ Audio",
            "• `/volume [nivel]` - Ajusta el volumen\n\
            • `/equalizer <preset>` - Aplica ecualizador\n\
            • `/effect <tipo>` - Activa efectos de audio",
            false,
        )
        .field(
            "🔊 Conexión",
            "• `/join` - Conecta al canal de voz\n\
            • `/leave` - Desconecta del canal\n\
            • `/nowplaying` - Muestra canción actual",
            false,
        )
        .field(
            "🎵 Fuentes Soportadas",
            "• YouTube / YouTube Music\n\
            • Spotify (metadata)\n\
            • SoundCloud\n\
            • Tidal HiFi\n\
            • URLs directas de audio",
            false,
        )
        .footer(CreateEmbedFooter::new(
            "Usa /help <comando> para ayuda específica",
        ))
        .timestamp(Timestamp::now())
}

/// Crea un embed de ayuda para un comando específico
pub fn create_command_help_embed(command: &str) -> CreateEmbed {
    let mut embed = CreateEmbed::default()
        .color(Colour::from_rgb(0, 123, 255))
        .timestamp(Timestamp::now());

    match command {
        "play" => {
            embed = embed
                .title("🎵 Comando /play")
                .description("Reproduce una canción o la agrega a la cola")
                .field("Uso", "`/play <query>`", false)
                .field(
                    "Ejemplos",
                    "• `/play Bohemian Rhapsody`\n\
                    • `/play https://youtube.com/watch?v=...`\n\
                    • `/play Queen - Don't Stop Me Now`",
                    false,
                )
                .field(
                    "Formatos Soportados",
                    "• Búsquedas de texto\n\
                    • URLs de YouTube\n\
                    • URLs de Spotify\n\
                    • URLs de SoundCloud",
                    false,
                );
        }
        "queue" => {
            embed = embed
                .title("📜 Comando /queue")
                .description("Muestra la cola de reproducción actual")
                .field("Uso", "`/queue [página]`", false)
                .field(
                    "Ejemplos",
                    "• `/queue` - Primera página\n\
                    • `/queue 2` - Página 2",
                    false,
                );
        }
        "volume" => {
            embed = embed
                .title("🔊 Comando /volume")
                .description("Ajusta el volumen de reproducción")
                .field("Uso", "`/volume [nivel]`", false)
                .field("Rango", "0-200 (100 = normal)", false)
                .field(
                    "Ejemplos",
                    "• `/volume` - Mostrar volumen actual\n\
                    • `/volume 50` - Volumen al 50%\n\
                    • `/volume 150` - Volumen al 150%",
                    false,
                );
        }
        "equalizer" => {
            embed = embed
                .title("🎛️ Comando /equalizer")
                .description("Configura el ecualizador de 10 bandas")
                .field("Uso", "`/equalizer <preset> [bandas]`", false)
                .field(
                    "Presets Disponibles",
                    "• `normal` - Sin modificaciones\n\
                    • `bass` - Realce de graves\n\
                    • `pop` - Optimizado para pop\n\
                    • `rock` - Optimizado para rock\n\
                    • `jazz` - Optimizado para jazz\n\
                    • `classical` - Optimizado para clásica\n\
                    • `electronic` - Optimizado para electrónica\n\
                    • `vocal` - Realce de vocales",
                    false,
                )
                .field(
                    "Ejemplos",
                    "• `/equalizer bass`\n\
                    • `/equalizer custom 32:2 64:1 125:0 ...`",
                    false,
                );
        }
        _ => {
            embed = embed
                .title("❓ Comando no encontrado")
                .description("Usa `/help` para ver todos los comandos disponibles");
        }
    }

    embed.footer(CreateEmbedFooter::new("Open Music Bot"))
}

/// Crea un embed de error
pub fn create_error_embed(title: &str, description: &str) -> CreateEmbed {
    CreateEmbed::default()
        .title(format!("❌ {}", title))
        .description(description)
        .color(Colour::from_rgb(255, 0, 0))
        .timestamp(Timestamp::now())
        .footer(CreateEmbedFooter::new("Open Music Bot"))
}

/// Crea un embed de éxito
pub fn create_success_embed(title: &str, description: &str) -> CreateEmbed {
    CreateEmbed::default()
        .title(format!("✅ {}", title))
        .description(description)
        .color(Colour::from_rgb(0, 255, 0))
        .timestamp(Timestamp::now())
        .footer(CreateEmbedFooter::new("Open Music Bot"))
}

/// Crea un embed de información
pub fn create_info_embed(title: &str, description: &str) -> CreateEmbed {
    CreateEmbed::default()
        .title(format!("ℹ️ {}", title))
        .description(description)
        .color(Colour::from_rgb(0, 123, 255))
        .timestamp(Timestamp::now())
        .footer(CreateEmbedFooter::new("Open Music Bot"))
}

/// Formatea una duración en formato legible
fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}
