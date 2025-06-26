use serenity::{
    all::{Colour, Timestamp},
    builder::{CreateEmbed, CreateEmbedFooter, CreateActionRow},
};
use std::time::Duration;

use crate::{
    audio::queue::{LoopMode, QueueInfo, QueueItem},
    sources::TrackSource,
    bot::OpenMusicBot,
};

/// Crea un embed para mostrar la canción actual desde TrackSource
pub fn create_now_playing_embed_from_source(track: &TrackSource) -> CreateEmbed {
    let mut embed = CreateEmbed::default()
        .title("🎵 Reproduciendo Ahora")
        .description(format!("**{}**", track.title()))
        .color(colors::SUCCESS_GREEN)
        .field("🎤 Artista", track.artist().as_ref().unwrap_or(&"Desconocido".to_string()), true);

    if let Some(duration) = track.duration() {
        embed = embed.field("⏱️ Duración", format_duration(duration), true);
    } else {
        embed = embed.field("⏱️ Duración", "🔴 En vivo", true);
    }

    embed = embed
        .field("👤 Solicitado por", format!("<@{}>", track.requested_by()), true)
        .field("🔗 Fuente", "YouTube", true);

    if let Some(thumbnail) = track.thumbnail() {
        embed = embed.thumbnail(&thumbnail);
    }

    embed = embed
        .url(&track.url())
        .timestamp(Timestamp::now())
        .footer(CreateEmbedFooter::new(STANDARD_FOOTER));

    embed
}

/// Paleta de colores estandarizada para el bot
pub mod colors {
    use serenity::all::Colour;
    
    pub const SUCCESS_GREEN: Colour = Colour::from_rgb(67, 181, 129);
    pub const ERROR_RED: Colour = Colour::from_rgb(220, 53, 69);
    pub const WARNING_ORANGE: Colour = Colour::from_rgb(255, 193, 7);
    pub const INFO_BLUE: Colour = Colour::from_rgb(52, 144, 220);
    pub const MUSIC_PURPLE: Colour = Colour::from_rgb(138, 43, 226);
    pub const NEUTRAL_GRAY: Colour = Colour::from_rgb(108, 117, 125);
    #[allow(dead_code)]
    pub const ACCENT_CYAN: Colour = Colour::from_rgb(23, 162, 184);
}

/// Footer estandarizado para todos los embeds
const STANDARD_FOOTER: &str = "🎵 Open Music Bot";

/// Crea un embed para mostrar la canción actual desde QueueItem
pub fn create_now_playing_embed(track: &QueueItem) -> CreateEmbed {
    let mut embed = CreateEmbed::default()
        .title("🎵 Reproduciendo Ahora")
        .description(format!("**{}**", track.title))
        .color(colors::SUCCESS_GREEN)
        .field("🎤 Artista", track.artist.as_ref().unwrap_or(&"Desconocido".to_string()), true);

    if let Some(duration) = track.duration {
        embed = embed.field("⏱️ Duración", format_duration(duration), true);
    } else {
        embed = embed.field("⏱️ Duración", "🔴 En vivo", true);
    }

    embed = embed
        .field("👤 Solicitado por", format!("<@{}>", track.requested_by), true)
        .field("🔗 Fuente", "YouTube", true);

    if let Some(thumbnail) = &track.thumbnail {
        embed = embed.thumbnail(thumbnail);
    }

    embed = embed
        .url(&track.url)
        .timestamp(Timestamp::now())
        .footer(CreateEmbedFooter::new(STANDARD_FOOTER));

    embed
}

/// Crea un embed para mostrar que se agregó una canción
pub fn create_track_added_embed(track: &TrackSource) -> CreateEmbed {
    let description = format!(
        "**{}** se ha agregado a la cola de reproducción",
        track.title()
    );
    
    let mut embed = CreateEmbed::default()
        .title("✅ Canción Agregada Exitosamente")
        .description(&description)
        .color(colors::SUCCESS_GREEN)
        .field("🎤 Artista", track.artist().as_ref().unwrap_or(&"Desconocido".to_string()), true);

    if let Some(duration) = track.duration() {
        embed = embed.field("⏱️ Duración", format_duration(duration), true);
    } else {
        embed = embed.field("⏱️ Duración", "🔴 En vivo", true);
    }

    embed = embed
        .field("👤 Solicitado por", format!("<@{}>", track.requested_by()), true)
        .field("🔗 Fuente", "YouTube", true);

    if let Some(thumbnail) = track.thumbnail() {
        embed = embed.thumbnail(&thumbnail);
    }

    embed = embed
        .url(&track.url())
        .timestamp(Timestamp::now())
        .footer(CreateEmbedFooter::new("🎵 Se reproducirá automáticamente si no hay música sonando"));

    embed
}

/// Crea un embed para mostrar que una playlist fue agregada
pub fn create_playlist_added_embed(track_count: usize, playlist_url: &str) -> CreateEmbed {
    let (title, emoji) = if track_count == 1 {
        ("1 Canción Agregada de Playlist", "✅")
    } else {
        ("Playlist Agregada Exitosamente", "📋")
    };
    
    let description = if track_count == 1 {
        "Se agregó **1 canción** de la playlist a la cola de reproducción".to_string()
    } else {
        format!("Se agregaron **{} canciones** de la playlist a la cola de reproducción", track_count)
    };

    let mut embed = CreateEmbed::default()
        .title(format!("{} {}", emoji, title))
        .description(&description)
        .color(colors::MUSIC_PURPLE)
        .field("📊 Canciones agregadas", track_count.to_string(), true);

    // Extraer el ID de la playlist para mostrar
    if let Some(list_start) = playlist_url.find("list=") {
        let list_id = &playlist_url[list_start + 5..];
        let clean_list_id = list_id.split('&').next().unwrap_or(list_id);
        embed = embed.field("🆔 Playlist ID", format!("`{}`", clean_list_id), true);
    }

    embed = embed.field("🔗 Fuente", "YouTube Playlist", true);

    // Agregar información útil en footer
    let footer_text = if track_count > 1 {
        "🎵 La reproducción comenzará automáticamente • Usa /queue para ver todas las canciones"
    } else {
        "🎵 La canción se reproducirá automáticamente"
    };
    
    embed = embed
        .footer(CreateEmbedFooter::new(footer_text))
        .timestamp(Timestamp::now());

    embed
}

/// Crea un embed para mostrar la cola de reproducción
pub fn create_queue_embed(queue_info: &QueueInfo, page: usize) -> CreateEmbed {
    let items_per_page = 10;
    let queue_page = queue_info.get_page(page, items_per_page);

    let mut embed = CreateEmbed::default()
        .title("📋 Cola de Reproducción")
        .color(colors::INFO_BLUE);

    if queue_info.total_items == 0 {
        return embed
            .description("😴 **La cola está vacía**\n\n💡 Usa `/play <canción>` para agregar música")
            .color(colors::NEUTRAL_GRAY)
            .footer(CreateEmbedFooter::new(STANDARD_FOOTER))
            .timestamp(Timestamp::now());
    }

    // Canción actual
    if let Some(current) = &queue_info.current {
        let status = match queue_info.loop_mode {
            LoopMode::Track => "🔂",
            LoopMode::Queue => "🔁",
            LoopMode::Off => "▶️",
        };

        let current_display = format!(
            "**{}**{}{}",
            current.title,
            if let Some(artist) = &current.artist {
                format!(" - {}", artist)
            } else {
                String::new()
            },
            if let Some(dur) = current.duration {
                format!(" `[{}]`", format_duration(dur))
            } else {
                String::new()
            }
        );

        embed = embed.field(format!("{} Reproduciendo", status), current_display, false);
    }

    // Próximas canciones con agrupación mejorada
    if !queue_page.items.is_empty() {
        let mut description = String::new();

        for (i, item) in queue_page.items.iter().enumerate() {
            let position = page.saturating_sub(1) * items_per_page + i + 1;
            let duration = if let Some(dur) = item.duration {
                format!(" `[{}]`", format_duration(dur))
            } else {
                String::new()
            };

            // Determinar el emoji basado en el solicitante y si es parte de una playlist
            let emoji = if position <= 5 {
                "🎵" // Próximas 5 canciones
            } else if position <= 15 {
                "🎶" // Siguientes canciones
            } else {
                "🎧" // Canciones más lejanas
            };

            description.push_str(&format!(
                "{} **{}**. {}{}{}\n",
                emoji,
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

        embed = embed.field("🎼 Próximas canciones", description, false);
    }

    // Información adicional mejorada
    let mut info = format!("**📊 Total:** {} canciones", queue_info.total_items);

    if queue_info.total_duration > Duration::ZERO {
        info.push_str(&format!(
            "\n**⏱️ Duración:** {}",
            format_duration(queue_info.total_duration)
        ));
    }

    // Posición actual en la cola
    info.push_str(&format!("\n**📍 Posición:** {}/{}", 
        1, // Posición simplificada
        queue_info.total_items + 1 // +1 para incluir la canción actual
    ));

    if queue_info.shuffle {
        info.push_str("\n**🔀 Modo:** Aleatorio");
    } else {
        info.push_str("\n**➡️ Modo:** Secuencial");
    }

    // Información de loop
    let loop_text = match queue_info.loop_mode {
        LoopMode::Track => "🔂 Repetir canción",
        LoopMode::Queue => "🔁 Repetir cola",
        LoopMode::Off => "➡️ Sin repetición",
    };
    info.push_str(&format!("\n**{}**", loop_text));

    embed = embed.field("📈 Estado de la Cola", info, false);

    // Paginación mejorada
    if queue_page.total_pages > 1 {
        let progress_bar = create_pagination_bar(queue_page.current_page, queue_page.total_pages);
        embed = embed.footer(CreateEmbedFooter::new(format!(
            "{} • Página {} de {} • Open Music Bot",
            progress_bar, queue_page.current_page, queue_page.total_pages
        )));
    } else {
        embed = embed.footer(CreateEmbedFooter::new(format!(
            "🎵 {} canciones en total • Open Music Bot", 
            queue_info.total_items
        )));
    }

    embed.timestamp(Timestamp::now())
}

/// Crea un embed mejorado para mostrar la cola con agrupación por playlists
pub fn create_enhanced_queue_embed(queue_info: &QueueInfo, page: usize, show_playlist_groups: bool) -> CreateEmbed {
    if !show_playlist_groups {
        return create_queue_embed(queue_info, page);
    }

    let items_per_page = 8; // Menos items por página para mostrar más detalles
    let queue_page = queue_info.get_page(page, items_per_page);

    let mut embed = CreateEmbed::default()
        .title("📋 Cola de Reproducción (Vista Agrupada)")
        .color(colors::MUSIC_PURPLE);

    if queue_info.total_items == 0 {
        return embed
            .description("😴 **La cola está vacía**\n\n💡 Usa `/play <canción>` para agregar música")
            .color(colors::NEUTRAL_GRAY)
            .footer(CreateEmbedFooter::new(STANDARD_FOOTER))
            .timestamp(Timestamp::now());
    }

    // Canción actual con más detalles
    if let Some(current) = &queue_info.current {
        let status_emoji = match queue_info.loop_mode {
            LoopMode::Track => "🔂",
            LoopMode::Queue => "🔁", 
            LoopMode::Off => "▶️",
        };

        let current_info = format!(
            "**{}**{}\n👤 Solicitado por: <@{}>\n⏱️ Duración: {}",
            current.title,
            if let Some(artist) = &current.artist {
                format!("\n🎤 Artista: {}", artist)
            } else {
                String::new()
            },
            current.requested_by,
            if let Some(dur) = current.duration {
                format_duration(dur)
            } else {
                "🔴 En vivo".to_string()
            }
        );

        embed = embed.field(format!("{} Reproduciendo Ahora", status_emoji), current_info, false);
    }

    // Próximas canciones con información detallada
    if !queue_page.items.is_empty() {
        let mut description = String::new();
        
        // Agrupar canciones por usuario solicitante
        let mut current_user: Option<serenity::model::id::UserId> = None;
        let mut user_count = 0;

        for (i, item) in queue_page.items.iter().enumerate() {
            let position = page.saturating_sub(1) * items_per_page + i + 1;
            
            // Detectar cambio de usuario para agrupación
            if current_user != Some(item.requested_by) {
                if current_user.is_some() {
                    description.push_str("\n");
                }
                current_user = Some(item.requested_by);
                user_count += 1;
                description.push_str(&format!("**👤 Grupo {} - <@{}>:**\n", user_count, item.requested_by));
            }

            let duration_text = if let Some(dur) = item.duration {
                format_duration(dur)
            } else {
                "🔴 Live".to_string()
            };

            description.push_str(&format!(
                "  **{}**. {} `[{}]`\n",
                position,
                item.title,
                duration_text
            ));
        }

        embed = embed.field("🎼 Cola Agrupada", description, false);
    }

    // Estadísticas detalladas
    let mut stats = String::new();
    
    // Calcular estadísticas por usuario
    let mut user_stats: std::collections::HashMap<serenity::model::id::UserId, (usize, Duration)> = std::collections::HashMap::new();
    
    // Analizar canción actual
    if let Some(current) = &queue_info.current {
        let entry = user_stats.entry(current.requested_by).or_insert((0, Duration::ZERO));
        entry.0 += 1;
        if let Some(dur) = current.duration {
            entry.1 += dur;
        }
    }
    
    // Analizar items de la cola (simplificado para este ejemplo)
    for item in &queue_page.items {
        let entry = user_stats.entry(item.requested_by).or_insert((0, Duration::ZERO));
        entry.0 += 1;
        if let Some(dur) = item.duration {
            entry.1 += dur;
        }
    }

    stats.push_str(&format!("📊 **Total:** {} canciones\n", queue_info.total_items));
    stats.push_str(&format!("⏱️ **Duración total:** {}\n", 
        if queue_info.total_duration > Duration::ZERO {
            format_duration(queue_info.total_duration)
        } else {
            "Calculando...".to_string()
        }
    ));
    stats.push_str(&format!("👥 **Contribuyentes:** {} usuarios\n", user_stats.len()));
    
    // Modo y estado
    if queue_info.shuffle {
        stats.push_str("🔀 **Aleatorio:** Activado\n");
    }
    
    let loop_text = match queue_info.loop_mode {
        LoopMode::Track => "🔂 Repetir canción",
        LoopMode::Queue => "🔁 Repetir cola", 
        LoopMode::Off => "➡️ Sin repetición",
    };
    stats.push_str(&format!("🔁 **Loop:** {}", loop_text));

    embed = embed.field("📈 Estadísticas Detalladas", stats, false);

    // Paginación con información adicional
    if queue_page.total_pages > 1 {
        let progress_bar = create_pagination_bar(queue_page.current_page, queue_page.total_pages);
        embed = embed.footer(CreateEmbedFooter::new(format!(
            "{} • Página {} de {} • Vista Agrupada • Open Music Bot",
            progress_bar, queue_page.current_page, queue_page.total_pages
        )));
    } else {
        embed = embed.footer(CreateEmbedFooter::new("🎵 Vista Agrupada • Open Music Bot"));
    }

    embed.timestamp(Timestamp::now())
}

/// Crea una barra de progreso para la paginación
fn create_pagination_bar(current: usize, total: usize) -> String {
    if total <= 1 {
        return "▰".to_string();
    }
    
    let bar_length: usize = 8;
    let filled = ((current as f64 / total as f64) * bar_length as f64) as usize;
    let empty = bar_length.saturating_sub(filled);
    
    format!("[{}{}]", "▰".repeat(filled), "▱".repeat(empty))
}

/// Crea un embed de ayuda general
#[allow(dead_code)]
pub fn create_help_embed() -> CreateEmbed {
    CreateEmbed::default()
        .title("🎵 Open Music Bot - Guía Completa")
        .color(colors::INFO_BLUE)
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
            • `/equalizer <preset>` - Aplica ecualizador",
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
#[allow(dead_code)]
pub fn create_command_help_embed(command: &str) -> CreateEmbed {
    let mut embed = CreateEmbed::default()
        .color(colors::INFO_BLUE)
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
        _ => {
            embed = embed
                .title("❓ Comando no encontrado")
                .description("Usa `/help` para ver todos los comandos disponibles");
        }
    }

    embed.footer(CreateEmbedFooter::new("Open Music Bot"))
}

/// Crea un embed de error
#[allow(dead_code)]
pub fn create_error_embed(title: &str, description: &str) -> CreateEmbed {
    CreateEmbed::default()
        .title(format!("❌ {}", title))
        .description(description)
        .color(colors::ERROR_RED)
        .timestamp(Timestamp::now())
        .footer(CreateEmbedFooter::new("Open Music Bot"))
}

/// Crea un embed de éxito
#[allow(dead_code)]
pub fn create_success_embed(title: &str, description: &str) -> CreateEmbed {
    CreateEmbed::default()
        .title(format!("✅ {}", title))
        .description(description)
        .color(colors::SUCCESS_GREEN)
        .timestamp(Timestamp::now())
        .footer(CreateEmbedFooter::new("Open Music Bot"))
}

/// Crea un embed de información
#[allow(dead_code)]
pub fn create_info_embed(title: &str, description: &str) -> CreateEmbed {
    CreateEmbed::default()
        .title(format!("ℹ️ {}", title))
        .description(description)
        .color(colors::INFO_BLUE)
        .timestamp(Timestamp::now())
        .footer(CreateEmbedFooter::new("Open Music Bot"))
}

/// Crea un embed para mostrar el estado del ecualizador
pub async fn create_equalizer_status_embed(guild_id: serenity::model::id::GuildId, bot: &OpenMusicBot) -> anyhow::Result<CreateEmbed> {
    let _ = guild_id; // Para evitar warning de parámetro no usado
    let _ = bot; // Para evitar warning de parámetro no usado
    
    let embed = CreateEmbed::default()
        .title("🎛️ Estado del Ecualizador AVANZADO")
        .description("Sistema de ecualizador de 10 bandas con procesamiento híbrido FFmpeg + DSP en tiempo real.")
        .color(colors::MUSIC_PURPLE)
        .field("Estado", "✅ OPERATIVO CON EFECTOS REALES", true)
        .field("Procesamiento", "🔥 Híbrido: FFmpeg + Real-time DSP", true)
        .field("Presets Disponibles", 
               "• **Flat** - Sin modificaciones\n\
                • **Bass** - Realce intenso de graves (+6dB)\n\
                • **Pop** - Optimizado para música pop\n\
                • **Rock** - Potencia rock con graves y agudos\n\
                • **Jazz** - Suave con realce de medios\n\
                • **Classical** - Refinado y balanceado\n\
                • **Electronic** - Intenso para electrónica\n\
                • **Vocal** - Claridad para voces", false)
        .field("Bandas de Frecuencia", 
               "32Hz • 64Hz • 125Hz • 250Hz • 500Hz\n\
                1kHz • 2kHz • 4kHz • 8kHz • 16kHz", true)
        .field("Capacidades", 
               "✅ Ajuste en tiempo real\n\
                ✅ Presets profesionales\n\
                ✅ Bandas personalizables\n\
                ✅ Aplicación instantánea", true)
        .timestamp(Timestamp::now())
        .footer(CreateEmbedFooter::new("Open Music Bot • Sistema de Audio Profesional"));
    
    Ok(embed)
}

/// Crea un embed para mostrar el estado del ecualizador
pub async fn create_effects_status_embed(_guild_id: serenity::model::id::GuildId, bot: &OpenMusicBot) -> anyhow::Result<CreateEmbed> {
    let eq_details = bot.player.get_equalizer_details();
    
    let description = format!("**Estado Actual:** 🎛️ {}\n\n**Presets de Ecualizador Disponibles:**\n🎵 **Bass** - Enfatiza graves\n🎤 **Pop** - Equilibrado moderno\n🎸 **Rock** - Graves y agudos\n🎺 **Jazz** - Claridad vocal\n🎼 **Classical** - Dinámico natural\n🔊 **Electronic** - Sintético\n🗣️ **Vocal** - Enfatiza voces\n📏 **Flat** - Sin modificaciones", eq_details);
    
    let embed = CreateEmbed::default()
        .title("🎛️ ECUALIZADOR DE AUDIO")
        .description(description)
        .color(Colour::from_rgb(100, 149, 237))
        .field("Comandos", 
               "• `/equalizer <preset>` - Aplicar preset de ecualizador", false)
        .timestamp(Timestamp::now())
        .footer(CreateEmbedFooter::new("Open Music Bot • Audio Engine v2.0"));
    
    Ok(embed)
}

/// Crea un embed para mostrar información detallada de una playlist antes de agregar
#[allow(dead_code)]
pub fn create_playlist_preview_embed(playlist_title: &str, track_count: usize, playlist_url: &str) -> CreateEmbed {
    let description = format!(
        "**Playlist detectada**: {}\n\n📊 **Canciones encontradas**: {}\n🎵 **Fuente**: YouTube\n\n⏳ Procesando canciones...",
        playlist_title,
        track_count
    );

    let mut embed = CreateEmbed::default()
        .title("📋 Cargando Playlist")
        .description(&description)
        .color(colors::WARNING_ORANGE) // Naranja para carga
        .thumbnail("https://img.youtube.com/vi/thumbnail_placeholder/maxresdefault.jpg");

    // Extraer el ID de la playlist
    if let Some(list_start) = playlist_url.find("list=") {
        let list_id = &playlist_url[list_start + 5..];
        let clean_list_id = list_id.split('&').next().unwrap_or(list_id);
        embed = embed.field("🆔 ID de Playlist", format!("`{}`", clean_list_id), true);
    }

    embed = embed
        .footer(CreateEmbedFooter::new("🔄 Las canciones se están agregando a la cola..."))
        .timestamp(Timestamp::now());

    embed
}

/// Crea un embed para mostrar cuando una playlist está vacía o hay error
#[allow(dead_code)]
pub fn create_playlist_error_embed(error_message: &str, playlist_url: &str) -> CreateEmbed {
    let mut embed = CreateEmbed::default()
        .title("❌ Error al Cargar Playlist")
        .description(format!(
            "**Error**: {}\n\n💡 **Posibles soluciones**:\n• Verifica que la playlist sea pública\n• Asegúrate de que la URL sea correcta\n• Intenta con otra playlist",
            error_message
        ))
        .color(colors::ERROR_RED) // Rojo para error
        .field("🔗 URL proporcionada", format!("`{}`", playlist_url), false);

    embed = embed
        .footer(CreateEmbedFooter::new("⚠️ Revisa la URL de la playlist e intenta nuevamente"))
        .timestamp(Timestamp::now());

    embed
}

/// Crea un embed de estado de operación con botones estándar
#[allow(dead_code)]
pub fn create_operation_status_embed(
    title: &str,
    description: &str,
    status: OperationStatus,
    show_retry: bool,
) -> (CreateEmbed, Vec<CreateActionRow>) {
    let (color, emoji) = match status {
        OperationStatus::Success => (colors::SUCCESS_GREEN, "✅"),
        OperationStatus::Error => (colors::ERROR_RED, "❌"),
        OperationStatus::Warning => (colors::WARNING_ORANGE, "⚠️"),
        OperationStatus::Info => (colors::INFO_BLUE, "ℹ️"),
        OperationStatus::Loading => (colors::WARNING_ORANGE, "⏳"),
    };

    let embed = CreateEmbed::default()
        .title(format!("{} {}", emoji, title))
        .description(description)
        .color(color)
        .footer(CreateEmbedFooter::new(STANDARD_FOOTER))
        .timestamp(Timestamp::now());

    let mut buttons = Vec::new();
    if show_retry && matches!(status, OperationStatus::Error) {
        let retry_btn = crate::ui::buttons::create_retry_button();
        buttons.push(retry_btn);
    }

    (embed, buttons)
}

/// Estados de operación para embeds estandarizados
#[allow(dead_code)]
pub enum OperationStatus {
    Success,
    Error,
    Warning,
    Info,
    Loading,
}

/// Crea un embed de volumen con indicador visual
#[allow(dead_code)]
pub fn create_volume_embed(current_volume: f32, is_muted: bool) -> CreateEmbed {
    let volume_percent = (current_volume * 100.0) as u8;
    
    let volume_bar = create_volume_bar(current_volume);
    let status_emoji = if is_muted {
        "🔇"
    } else if volume_percent == 0 {
        "🔈"
    } else if volume_percent <= 50 {
        "🔉"
    } else {
        "🔊"
    };

    let description = if is_muted {
        "**Audio silenciado**".to_string()
    } else {
        format!("**Volumen actual: {}%**", volume_percent)
    };

    CreateEmbed::default()
        .title(format!("{} Control de Volumen", status_emoji))
        .description(&description)
        .field("📊 Nivel", volume_bar, false)
        .field("📈 Porcentaje", format!("{}%", volume_percent), true)
        .field("🎛️ Estado", if is_muted { "Silenciado" } else { "Activo" }, true)
        .color(if is_muted { colors::WARNING_ORANGE } else { colors::INFO_BLUE })
        .footer(CreateEmbedFooter::new("💡 Usa los botones o /volume <nivel> para ajustar"))
        .timestamp(Timestamp::now())
}

/// Crea una barra visual de volumen
#[allow(dead_code)]
fn create_volume_bar(volume: f32) -> String {
    let segments = 20;
    let filled = (volume * segments as f32) as usize;
    let empty = segments - filled;
    
    let bar = "█".repeat(filled) + &"▒".repeat(empty);
    format!("`[{}]`", bar)
}

/// Crea un embed para carga progresiva de playlist
pub fn create_playlist_loading_embed(
    playlist_title: &str,
    current: usize,
    total: usize,
    loaded_tracks: &[String], // Últimas 3 canciones cargadas
    playlist_url: &str
) -> CreateEmbed {
    let progress_percent = if total > 0 {
        current as f64 / total as f64 * 100.0
    } else {
        0.0
    };
    
    let progress_bar = create_progress_bar(progress_percent, 20);
    
    let description = format!(
        "**{}**\n\n📊 **Progreso**: {}/{} canciones ({:.1}%)\n{}\n\n⏳ Cargando canciones de YouTube...",
        playlist_title,
        current,
        total,
        progress_percent,
        progress_bar
    );

    let mut embed = CreateEmbed::default()
        .title("🔄 Cargando Playlist")
        .description(&description)
        .color(colors::WARNING_ORANGE);

    // Mostrar últimas canciones cargadas
    if !loaded_tracks.is_empty() {
        let recent_tracks = loaded_tracks
            .iter()
            .rev()
            .take(3)
            .enumerate()
            .map(|(i, track)| format!("{}. {}", current - i, track))
            .collect::<Vec<_>>()
            .join("\n");
        
        embed = embed.field("🎵 Últimas agregadas", recent_tracks, false);
    }

    // Extraer el ID de la playlist
    if let Some(list_start) = playlist_url.find("list=") {
        let list_id = &playlist_url[list_start + 5..];
        let clean_list_id = list_id.split('&').next().unwrap_or(list_id);
        embed = embed.field("🆔 Playlist ID", format!("`{}`", clean_list_id), true);
    }

    embed = embed
        .field("⏱️ Estado", "Procesando...", true)
        .footer(CreateEmbedFooter::new("💡 Puedes cancelar la carga usando el botón rojo"))
        .timestamp(Timestamp::now());

    embed
}

/// Crea un embed para mostrar información detallada de una playlist completa
pub fn create_enhanced_playlist_embed(
    playlist_title: &str,
    creator: Option<&str>,
    track_count: usize,
    total_duration: Option<Duration>,
    playlist_url: &str,
    thumbnail_url: Option<&str>
) -> CreateEmbed {
    let mut embed = CreateEmbed::default()
        .title("📋 Información de Playlist")
        .color(colors::MUSIC_PURPLE);

    // Descripción principal
    let mut description = format!("**{}**\n\n", playlist_title);
    
    if let Some(creator_name) = creator {
        description.push_str(&format!("👤 **Creador:** {}\n", creator_name));
    }
    
    description.push_str(&format!("📊 **Canciones:** {}\n", track_count));
    
    if let Some(duration) = total_duration {
        description.push_str(&format!("⏱️ **Duración total:** {}\n", format_duration(duration)));
    }
    
    description.push_str("🎵 **Fuente:** YouTube");
    
    embed = embed.description(&description);

    // Agregar thumbnail si está disponible
    if let Some(thumb) = thumbnail_url {
        embed = embed.thumbnail(thumb);
    }

    // Información adicional
    if let Some(list_start) = playlist_url.find("list=") {
        let list_id = &playlist_url[list_start + 5..];
        let clean_list_id = list_id.split('&').next().unwrap_or(list_id);
        embed = embed.field("🆔 ID de Playlist", format!("`{}`", clean_list_id), true);
    }

    embed = embed
        .field("🔗 URL", "[Ver en YouTube](".to_owned() + playlist_url + ")", true)
        .field("📈 Estado", "✅ Lista para cargar", true);

    // Estadísticas adicionales
    let stats = if track_count > 50 {
        "🔥 Playlist extensa - Carga optimizada"
    } else if track_count > 20 {
        "📊 Playlist mediana - Carga rápida"
    } else {
        "⚡ Playlist pequeña - Carga instantánea"
    };
    
    embed = embed.field("📊 Estadísticas", stats, false);

    embed = embed
        .footer(CreateEmbedFooter::new("💡 Usa los botones para cargar, previsualizar o guardar"))
        .timestamp(Timestamp::now());

    embed
}

/// Crea un embed para playlist completada con estadísticas
pub fn create_playlist_completed_embed(
    playlist_title: &str,
    loaded_count: usize,
    total_count: usize,
    failed_count: usize,
    total_duration: Option<Duration>,
    playlist_url: &str
) -> CreateEmbed {
    let success_rate = if total_count > 0 {
        loaded_count as f64 / total_count as f64 * 100.0
    } else {
        0.0
    };
    
    let (color, status_emoji) = if success_rate >= 90.0 {
        (colors::SUCCESS_GREEN, "✅")
    } else if success_rate >= 70.0 {
        (colors::WARNING_ORANGE, "⚠️")
    } else {
        (colors::ERROR_RED, "❌")
    };

    let description = format!(
        "**{}**\n\n📊 **Resultados de carga:**\n✅ Cargadas: {} canciones\n❌ Fallidas: {} canciones\n📈 Éxito: {:.1}%",
        playlist_title,
        loaded_count,
        failed_count,
        success_rate
    );

    let mut embed = CreateEmbed::default()
        .title(format!("{} Playlist Cargada", status_emoji))
        .description(&description)
        .color(color);

    if let Some(duration) = total_duration {
        embed = embed.field("⏱️ Duración total", format_duration(duration), true);
    }

    embed = embed
        .field("🎵 En cola", format!("{} canciones", loaded_count), true)
        .field("🔗 Fuente", "YouTube", true);

    // Extraer el ID de la playlist
    if let Some(list_start) = playlist_url.find("list=") {
        let list_id = &playlist_url[list_start + 5..];
        let clean_list_id = list_id.split('&').next().unwrap_or(list_id);
        embed = embed.field("🆔 Playlist ID", format!("`{}`", clean_list_id), true);
    }

    let footer_text = if failed_count > 0 {
        "⚠️ Algunas canciones no pudieron cargarse (privadas o no disponibles)"
    } else {
        "🎵 Todas las canciones se cargaron exitosamente"
    };

    embed = embed
        .footer(CreateEmbedFooter::new(footer_text))
        .timestamp(Timestamp::now());

    embed
}

/// Crea una barra de progreso visual
fn create_progress_bar(percentage: f64, length: usize) -> String {
    let filled = ((percentage / 100.0) * length as f64) as usize;
    let empty = length.saturating_sub(filled);
    
    let bar = "█".repeat(filled) + &"▒".repeat(empty);
    format!("`[{}] {:.1}%`", bar, percentage)
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
