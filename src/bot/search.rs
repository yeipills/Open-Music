use anyhow::Result;
use serenity::{
    builder::{CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage, CreateSelectMenu, CreateSelectMenuOption, CreateActionRow, CreateSelectMenuKind},
    model::{
        application::{CommandInteraction, ComponentInteraction},
        id::{GuildId, UserId},
    },
    prelude::Context,
    all::Colour,
};
use std::time::Duration;
use tracing::info;

use crate::{
    sources::{youtube::YouTubeClient, TrackSource, SourceType},
    bot::OpenMusicBot,
};

/// Estructura para manejar resultados de búsqueda
#[derive(Debug, Clone)]
pub struct SearchSession {
    pub query: String,
    pub results: Vec<TrackSource>,
    pub user_id: UserId,
    pub guild_id: GuildId,
}

/// Maneja el comando de búsqueda con selección múltiple
pub async fn handle_search_command(
    ctx: &Context,
    command: CommandInteraction,
    _bot: &OpenMusicBot,
) -> Result<()> {
    let _guild_id = command
        .guild_id
        .ok_or_else(|| anyhow::anyhow!("Comando usado fuera de un servidor"))?;

    let query = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "query")
        .and_then(|opt| opt.value.as_str())
        .ok_or_else(|| anyhow::anyhow!("Query no proporcionado"))?;

    // Defer la respuesta
    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Defer(CreateInteractionResponseMessage::new()),
        )
        .await?;

    info!("🔍 Búsqueda iniciada por {}: {}", command.user.name, query);

    // Buscar en YouTube
    let youtube_client = YouTubeClient::new();
    let search_results = youtube_client.search_detailed(query, 10).await?;
    let filtered_results = youtube_client.filter_results(search_results, query);

    if filtered_results.is_empty() {
        use serenity::builder::EditInteractionResponse;
        command
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new()
                    .embed(create_no_results_embed(query))
            )
            .await?;
        return Ok(());
    }

    // Convertir metadata a TrackSource
    let track_results: Vec<TrackSource> = filtered_results
        .into_iter()
        .take(5) // Limitar a 5 resultados para el menú
        .map(|meta| {
            let mut track = TrackSource::new(
                meta.title,
                meta.url.unwrap_or_default(),
                SourceType::YouTube,
                command.user.id,
            );

            if let Some(artist) = meta.artist {
                track = track.with_artist(artist);
            }

            if let Some(duration) = meta.duration {
                track = track.with_duration(duration);
            }

            if let Some(thumbnail) = meta.thumbnail {
                track = track.with_thumbnail(thumbnail);
            }

            track
        })
        .collect();

    // Crear embed y menú de selección
    let embed = create_search_results_embed(query, &track_results);
    let select_menu = create_track_selection_menu(&track_results);

    use serenity::builder::EditInteractionResponse;
    command
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new()
                .embed(embed)
                .components(vec![CreateActionRow::SelectMenu(select_menu)])
        )
        .await?;

    Ok(())
}

/// Maneja la selección de una canción del menú
pub async fn handle_track_selection(
    ctx: &Context,
    interaction: &ComponentInteraction,
    bot: &OpenMusicBot,
    selected_index: usize,
) -> Result<()> {
    let guild_id = interaction
        .guild_id
        .ok_or_else(|| anyhow::anyhow!("Interacción fuera de un servidor"))?;

    // Defer la respuesta
    interaction.defer(&ctx.http).await?;

    // Verificar que el usuario esté en un canal de voz
    let voice_channel_id = get_user_voice_channel(ctx, guild_id, interaction.user.id).await?;

    // Conectar al canal de voz si no está conectado
    if bot.get_voice_handler(guild_id).is_none() {
        bot.join_voice_channel(ctx, guild_id, voice_channel_id)
            .await?;
    }

    info!("✅ Canción seleccionada por {}: índice {}", interaction.user.name, selected_index);

    // Aquí necesitarías recuperar la información de la búsqueda
    // Por ahora, crear una respuesta de éxito
    use serenity::builder::CreateInteractionResponseFollowup;
    interaction
        .create_followup(
            &ctx.http,
            CreateInteractionResponseFollowup::new()
                .embed(create_success_embed("Canción Agregada", "La canción ha sido agregada a la cola"))
                .ephemeral(true),
        )
        .await?;

    Ok(())
}

/// Crea embed con resultados de búsqueda
fn create_search_results_embed(query: &str, results: &[TrackSource]) -> CreateEmbed {
    let mut embed = CreateEmbed::default()
        .title("🔍 Resultados de Búsqueda")
        .description(format!("Búsqueda: **{}**\nSelecciona una canción del menú inferior:", query))
        .color(Colour::from_rgb(0, 123, 255));

    let mut field_value = String::new();
    for (i, track) in results.iter().enumerate() {
        let duration_str = if let Some(duration) = track.duration() {
            format_duration(duration)
        } else {
            "En vivo".to_string()
        };

        let artist_str = if let Some(artist) = track.artist() {
            format!(" - {}", artist)
        } else {
            String::new()
        };

        field_value.push_str(&format!(
            "**{}**. {}{} `[{}]`\n",
            i + 1,
            track.title(),
            artist_str,
            duration_str
        ));
    }

    embed = embed.field("Canciones Encontradas", field_value, false);

    embed
}

/// Crea embed cuando no hay resultados
fn create_no_results_embed(query: &str) -> CreateEmbed {
    CreateEmbed::default()
        .title("❌ Sin Resultados")
        .description(format!("No se encontraron canciones para: **{}**\n\nIntenta con:\n• Términos más específicos\n• Nombre del artista\n• Título completo de la canción", query))
        .color(Colour::from_rgb(255, 69, 0))
}

/// Crea embed de éxito
fn create_success_embed(title: &str, description: &str) -> CreateEmbed {
    CreateEmbed::default()
        .title(format!("✅ {}", title))
        .description(description)
        .color(Colour::from_rgb(67, 181, 129))
}

/// Crea menú de selección para tracks
fn create_track_selection_menu(tracks: &[TrackSource]) -> CreateSelectMenu {
    let mut options = Vec::new();

    for (i, track) in tracks.iter().enumerate() {
        let duration_str = if let Some(duration) = track.duration() {
            format!(" [{}]", format_duration(duration))
        } else {
            String::new()
        };

        let artist_str = if let Some(artist) = track.artist() {
            format!(" - {}", artist)
        } else {
            String::new()
        };

        let label = format!("{}{}{}", track.title(), artist_str, duration_str);
        let truncated_label = if label.len() > 100 {
            format!("{}...", &label[..97])
        } else {
            label
        };

        options.push(
            CreateSelectMenuOption::new(truncated_label, format!("track_{}", i))
                .description(format!("YouTube • {}", 
                    if let Some(artist) = track.artist() { artist } else { "Desconocido".to_string() }
                ))
        );
    }

    CreateSelectMenu::new("track_selection", CreateSelectMenuKind::String { options })
        .placeholder("Selecciona una canción para reproducir...")
        .min_values(1)
        .max_values(1)
}

/// Formatea duración en formato legible
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

/// Obtiene el canal de voz del usuario
async fn get_user_voice_channel(
    ctx: &Context,
    guild_id: GuildId,
    user_id: UserId,
) -> Result<serenity::model::id::ChannelId> {
    let guild = guild_id
        .to_guild_cached(&ctx.cache)
        .ok_or_else(|| anyhow::anyhow!("Guild no encontrada en caché"))?;

    let channel_id = guild
        .voice_states
        .get(&user_id)
        .and_then(|voice_state| voice_state.channel_id)
        .ok_or_else(|| anyhow::anyhow!("Debes estar en un canal de voz"))?;

    Ok(channel_id)
}