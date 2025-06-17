use anyhow::Result;
use serenity::{
    builder::{CreateInteractionResponse, CreateInteractionResponseMessage},
    model::{
        application::{CommandInteraction, ComponentInteraction},
        id::{ChannelId, GuildId, UserId},
    },
    prelude::Context,
};
use tracing::info;

use crate::{
    bot::OpenMusicBot,
    sources::{youtube::YouTubeClient, MusicSource},
    ui::{buttons, embeds},
};

/// Maneja comandos slash
pub async fn handle_command(
    ctx: &Context,
    command: CommandInteraction,
    bot: &OpenMusicBot,
) -> Result<()> {
    let guild_id = command
        .guild_id
        .ok_or_else(|| anyhow::anyhow!("Comando usado fuera de un servidor"))?;

    info!(
        "📝 Comando /{} usado por {} en guild {}",
        command.data.name, command.user.name, guild_id
    );

    match command.data.name.as_str() {
        "play" => handle_play(ctx, command, bot).await?,
        "search" => super::search::handle_search_command(ctx, command, bot).await?,
        "pause" => handle_pause(ctx, command, bot).await?,
        "resume" => handle_resume(ctx, command, bot).await?,
        "skip" => handle_skip(ctx, command, bot).await?,
        "stop" => handle_stop(ctx, command, bot).await?,
        "queue" => handle_queue(ctx, command, bot).await?,
        "nowplaying" => handle_nowplaying(ctx, command, bot).await?,
        "shuffle" => handle_shuffle(ctx, command, bot).await?,
        "loop" => handle_loop(ctx, command, bot).await?,
        "volume" => handle_volume(ctx, command, bot).await?,
        "join" => handle_join(ctx, command, bot).await?,
        "leave" => handle_leave(ctx, command, bot).await?,
        "help" => handle_help(ctx, command, bot).await?,
        _ => {
            command
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("❌ Comando no reconocido")
                            .ephemeral(true),
                    ),
                )
                .await?;
        }
    }

    Ok(())
}

/// Maneja interacciones con componentes (botones, menús, etc.)
pub async fn handle_component(
    ctx: &Context,
    component: ComponentInteraction,
    bot: &OpenMusicBot,
) -> Result<()> {
    let guild_id = component
        .guild_id
        .ok_or_else(|| anyhow::anyhow!("Componente usado fuera de un servidor"))?;

    info!(
        "🔘 Botón {} presionado por {} en guild {}",
        component.data.custom_id, component.user.name, guild_id
    );

    match component.data.custom_id.as_str() {
        "track_selection" => {
            // Manejar selección de track del menú de búsqueda
            if let serenity::model::application::ComponentInteractionDataKind::StringSelect { values } = &component.data.kind {
                if let Some(selected_value) = values.first() {
                    if let Some(index_str) = selected_value.strip_prefix("track_") {
                        if let Ok(index) = index_str.parse::<usize>() {
                            super::search::handle_track_selection(ctx, &component, bot, index).await?;
                        }
                    }
                }
            }
        }
        "player_pause" => {
            bot.player.pause(guild_id).await?;
            component
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new().content("⏸️ Pausado"),
                    ),
                )
                .await?;
        }
        "player_resume" => {
            bot.player.resume(guild_id).await?;
            component
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new().content("▶️ Reanudado"),
                    ),
                )
                .await?;
        }
        "player_skip" => {
            if let Some(handler) = bot.get_voice_handler(guild_id) {
                bot.player.skip(guild_id, handler).await?;
                component
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::UpdateMessage(
                            CreateInteractionResponseMessage::new().content("⏭️ Saltado"),
                        ),
                    )
                    .await?;
            }
        }
        "player_stop" => {
            bot.player.stop(guild_id).await?;
            component
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new().content("⏹️ Detenido"),
                    ),
                )
                .await?;
        }
        "player_shuffle" => {
            bot.player.toggle_shuffle(guild_id).await?;
            component
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content("🔀 Modo aleatorio cambiado"),
                    ),
                )
                .await?;
        }
        _ => {
            component
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new().content("❌ Acción no reconocida"),
                    ),
                )
                .await?;
        }
    }

    Ok(())
}

// Handlers específicos para cada comando

async fn handle_play(ctx: &Context, command: CommandInteraction, bot: &OpenMusicBot) -> Result<()> {
    let guild_id = command.guild_id.unwrap();
    let query = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "query")
        .and_then(|opt| opt.value.as_str())
        .ok_or_else(|| anyhow::anyhow!("Query no proporcionado"))?;

    // Defer la respuesta ya que puede tomar tiempo
    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Defer(CreateInteractionResponseMessage::new()),
        )
        .await?;

    // Verificar que el usuario esté en un canal de voz
    let voice_channel_id = get_user_voice_channel(ctx, guild_id, command.user.id).await?;

    // Conectar al canal de voz si no está conectado
    if bot.get_voice_handler(guild_id).is_none() {
        bot.join_voice_channel(ctx, guild_id, voice_channel_id)
            .await?;
    }

    // Buscar y agregar a la cola
    let youtube_client = YouTubeClient::new();
    let track_source = if query.starts_with("http") {
        // Es una URL directa
        youtube_client.get_track(query).await?
    } else {
        // Es una búsqueda
        youtube_client
            .search(query, 1)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No se encontraron resultados"))?
    };

    // Agregar a la cola y reproducir
    if let Some(handler) = bot.get_voice_handler(guild_id) {
        bot.player
            .play(guild_id, track_source.clone(), handler)
            .await?;

        // Responder con embed
        let embed = embeds::create_track_added_embed(&track_source);
        use serenity::builder::EditInteractionResponse;
        command
            .edit_response(&ctx.http, EditInteractionResponse::new().embed(embed))
            .await?;
    }

    Ok(())
}

async fn handle_pause(
    ctx: &Context,
    command: CommandInteraction,
    bot: &OpenMusicBot,
) -> Result<()> {
    let guild_id = command.guild_id.unwrap();

    bot.player.pause(guild_id).await?;

    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content("⏸️ Reproducción pausada"),
            ),
        )
        .await?;

    Ok(())
}

async fn handle_resume(
    ctx: &Context,
    command: CommandInteraction,
    bot: &OpenMusicBot,
) -> Result<()> {
    let guild_id = command.guild_id.unwrap();

    bot.player.resume(guild_id).await?;

    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content("▶️ Reproducción reanudada"),
            ),
        )
        .await?;

    Ok(())
}

async fn handle_skip(ctx: &Context, command: CommandInteraction, bot: &OpenMusicBot) -> Result<()> {
    let guild_id = command.guild_id.unwrap();

    let amount = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "amount")
        .and_then(|opt| opt.value.as_i64())
        .unwrap_or(1) as usize;

    bot.player.skip_tracks(guild_id, amount).await?;

    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(format!("⏭️ Saltadas {} canciones", amount)),
            ),
        )
        .await?;

    Ok(())
}

async fn handle_stop(ctx: &Context, command: CommandInteraction, bot: &OpenMusicBot) -> Result<()> {
    let guild_id = command.guild_id.unwrap();

    bot.player.stop(guild_id).await?;
    bot.leave_voice_channel(ctx, guild_id).await?;

    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content("⏹️ Reproducción detenida y cola limpiada"),
            ),
        )
        .await?;

    Ok(())
}

async fn handle_queue(
    ctx: &Context,
    command: CommandInteraction,
    bot: &OpenMusicBot,
) -> Result<()> {
    let guild_id = command.guild_id.unwrap();

    let page = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "page")
        .and_then(|opt| opt.value.as_i64())
        .unwrap_or(1) as usize;

    let queue_info = bot.player.get_queue_info(guild_id).await?;
    let embed = embeds::create_queue_embed(&queue_info, page);

    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed),
            ),
        )
        .await?;

    Ok(())
}

async fn handle_nowplaying(
    ctx: &Context,
    command: CommandInteraction,
    bot: &OpenMusicBot,
) -> Result<()> {
    let guild_id = command.guild_id.unwrap();

    if let Some(current) = bot.player.get_current_track(guild_id).await {
        let embed = embeds::create_now_playing_embed(&current);
        let buttons = buttons::create_player_buttons();

        command
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .embed(embed)
                        .components(buttons),
                ),
            )
            .await?;
    } else {
        command
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("❌ No hay nada reproduciéndose actualmente")
                        .ephemeral(true),
                ),
            )
            .await?;
    }

    Ok(())
}

async fn handle_shuffle(
    ctx: &Context,
    command: CommandInteraction,
    bot: &OpenMusicBot,
) -> Result<()> {
    let guild_id = command.guild_id.unwrap();

    let shuffled = bot.player.toggle_shuffle(guild_id).await?;

    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().content(
                if shuffled {
                    "🔀 Modo aleatorio activado"
                } else {
                    "➡️ Modo aleatorio desactivado"
                },
            )),
        )
        .await?;

    Ok(())
}

async fn handle_loop(ctx: &Context, command: CommandInteraction, bot: &OpenMusicBot) -> Result<()> {
    let guild_id = command.guild_id.unwrap();

    let mode = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "mode")
        .and_then(|opt| opt.value.as_str())
        .unwrap_or("off");

    let enabled = mode != "off";
    bot.player.set_loop_mode(guild_id, enabled).await?;

    let message = match mode {
        "track" => "🔂 Repetir canción activado",
        "queue" => "🔁 Repetir cola activado",
        _ => "➡️ Repetición desactivada",
    };

    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content(message),
            ),
        )
        .await?;

    Ok(())
}

async fn handle_volume(
    ctx: &Context,
    command: CommandInteraction,
    bot: &OpenMusicBot,
) -> Result<()> {
    let guild_id = command.guild_id.unwrap();

    let volume = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "level")
        .and_then(|opt| opt.value.as_i64());

    if let Some(vol) = volume {
        let normalized = (vol as f32 / 100.0).clamp(0.0, 2.0);
        bot.player.set_volume(guild_id, normalized).await?;

        command
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(format!("🔊 Volumen ajustado a {}%", vol)),
                ),
            )
            .await?;
    } else {
        let current = bot.player.get_volume(guild_id).await.unwrap_or(0.5);
        command
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(format!("🔊 Volumen actual: {}%", (current * 100.0) as i32)),
                ),
            )
            .await?;
    }

    Ok(())
}

async fn handle_join(ctx: &Context, command: CommandInteraction, bot: &OpenMusicBot) -> Result<()> {
    let guild_id = command.guild_id.unwrap();
    let voice_channel_id = get_user_voice_channel(ctx, guild_id, command.user.id).await?;

    bot.join_voice_channel(ctx, guild_id, voice_channel_id)
        .await?;

    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content("🔊 Conectado al canal de voz"),
            ),
        )
        .await?;

    Ok(())
}

async fn handle_leave(
    ctx: &Context,
    command: CommandInteraction,
    bot: &OpenMusicBot,
) -> Result<()> {
    let guild_id = command.guild_id.unwrap();

    bot.player.stop(guild_id).await?;
    bot.leave_voice_channel(ctx, guild_id).await?;

    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().content("👋 Desconectado del canal de voz"),
            ),
        )
        .await?;

    Ok(())
}

async fn handle_help(ctx: &Context, command: CommandInteraction, _bot: &OpenMusicBot) -> Result<()> {
    let specific_command = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "command")
        .and_then(|opt| opt.value.as_str());

    let embed = if let Some(cmd) = specific_command {
        embeds::create_command_help_embed(cmd)
    } else {
        embeds::create_help_embed()
    };

    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .embed(embed)
                    .ephemeral(true),
            ),
        )
        .await?;

    Ok(())
}

// Funciones auxiliares

async fn get_user_voice_channel(
    ctx: &Context,
    guild_id: GuildId,
    user_id: UserId,
) -> Result<ChannelId> {
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
