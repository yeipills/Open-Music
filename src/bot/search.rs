use anyhow::Result;
use serenity::{
    builder::{CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage, CreateSelectMenu, CreateSelectMenuOption, CreateActionRow, CreateSelectMenuKind},
    model::{
        application::{CommandInteraction, ComponentInteraction},
        id::{GuildId, UserId},
    },
    prelude::Context,
};
use dashmap::DashMap;
use std::sync::LazyLock;

use crate::{
    ui::embeds::{colors, create_success_embed, create_error_embed},
    sources::TrackSource,
    bot::OpenMusicBot,
};
use std::time::Duration;
use tracing::info;

// Almacén global para sesiones de búsqueda
static SEARCH_SESSIONS: LazyLock<DashMap<String, Vec<TrackSource>>> = LazyLock::new(DashMap::new);

use crate::{
    sources::{youtube_fast::YouTubeFastClient, SourceType},
};

/// Estructura para manejar resultados de búsqueda
#[derive(Debug, Clone)]
#[allow(dead_code)]
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

    // Buscar en YouTube (rápido)
    let youtube_client = YouTubeFastClient::new();
    let search_results = youtube_client.search_fast(query, 5).await?;

    if search_results.is_empty() {
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
    let track_results: Vec<TrackSource> = search_results
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

    // Almacenar resultados en la sesión
    let session_key = format!("{}_{}", command.user.id, command.guild_id.unwrap_or_default());
    SEARCH_SESSIONS.insert(session_key, track_results.clone());

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

    // Recuperar resultados de la sesión
    let session_key = format!("{}_{}", interaction.user.id, guild_id);
    let track_results = match SEARCH_SESSIONS.get(&session_key) {
        Some(results) => results.clone(),
        None => {
            use serenity::builder::CreateInteractionResponseFollowup;
            interaction
                .create_followup(
                    &ctx.http,
                    CreateInteractionResponseFollowup::new()
                        .embed(create_error_embed("Sesión Expirada", "Los resultados de búsqueda han expirado. Realiza una nueva búsqueda."))
                        .ephemeral(true),
                )
                .await?;
            return Ok(());
        }
    };

    // Verificar que el índice sea válido
    if selected_index >= track_results.len() {
        use serenity::builder::CreateInteractionResponseFollowup;
        interaction
            .create_followup(
                &ctx.http,
                CreateInteractionResponseFollowup::new()
                    .embed(create_error_embed("Error", "Selección inválida"))
                    .ephemeral(true),
            )
            .await?;
        return Ok(());
    }

    let selected_track = &track_results[selected_index];
    info!("✅ Canción seleccionada por {}: {}", interaction.user.name, selected_track.title());

    // Obtener el handler de voz  
    let handler = bot.get_voice_handler(guild_id)
        .ok_or_else(|| anyhow::anyhow!("No hay conexión de voz activa"))?;

    // Obtener posición actual en la cola antes de agregar
    let queue_size = bot.player.get_queue(guild_id).await.unwrap_or_default().len();
    
    // Agregar la canción a la cola y reproducir si es necesario
    match bot.player.play(guild_id, selected_track.clone(), handler).await {
        Ok(()) => {
            use serenity::builder::CreateInteractionResponseFollowup;
            let embed = if queue_size == 0 {
                create_success_embed(
                    "🎵 Reproduciendo Ahora",
                    &format!("**{}**\n{}", selected_track.title(), 
                        selected_track.artist().as_deref().unwrap_or("Artista desconocido"))
                )
            } else {
                create_success_embed(
                    "✅ Agregado a la Cola",
                    &format!("**{}**\n{}\n📍 Posición en cola: **{}**", 
                        selected_track.title(), 
                        selected_track.artist().as_deref().unwrap_or("Artista desconocido"),
                        queue_size + 1)
                )
            };

            interaction
                .create_followup(
                    &ctx.http,
                    CreateInteractionResponseFollowup::new()
                        .embed(embed)
                        .ephemeral(true),
                )
                .await?;

            // El método play ya se encarga de iniciar la reproducción si es necesario
        }
        Err(e) => {
            use serenity::builder::CreateInteractionResponseFollowup;
            interaction
                .create_followup(
                    &ctx.http,
                    CreateInteractionResponseFollowup::new()
                        .embed(create_error_embed("Error", &format!("No se pudo agregar la canción: {}", e)))
                        .ephemeral(true),
                )
                .await?;
        }
    }

    // Limpiar la sesión después de usar
    SEARCH_SESSIONS.remove(&session_key);

    Ok(())
}

/// Crea embed con resultados de búsqueda
fn create_search_results_embed(query: &str, results: &[TrackSource]) -> CreateEmbed {
    let mut embed = CreateEmbed::default()
        .title("🔍 Resultados de Búsqueda")
        .description(format!("🎵 **Búsqueda:** `{}`\n📜 Selecciona una canción del menú desplegable:", query))
        .color(colors::INFO_BLUE);

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
        .title("❌ Sin Resultados de Búsqueda")
        .description(format!("🔍 **Búsqueda:** `{}`\n\n😔 No se encontraron canciones que coincidan\n\n💡 **Sugerencias:**\n• Verifica la ortografía\n• Usa términos más específicos\n• Incluye el nombre del artista\n• Intenta con el título completo", query))
        .color(colors::WARNING_ORANGE)
        .footer(serenity::builder::CreateEmbedFooter::new("🎵 También puedes usar URLs directas de YouTube"))
        .timestamp(serenity::all::Timestamp::now())
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