use anyhow::Result;
use serenity::{
    builder::{CreateInteractionResponse, CreateInteractionResponseMessage, CreateEmbed},
    model::{
        application::{CommandInteraction, ComponentInteraction},
        id::{ChannelId, GuildId, UserId},
    },
    prelude::Context,
};
use tracing::{info, warn};

use crate::{
    bot::OpenMusicBot,
    sources::{youtube_fast::YouTubeFastClient, MusicSource, TrackSource, SourceType},
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
        "equalizer" => handle_equalizer(ctx, command, bot).await?,
        "clear" => handle_clear(ctx, command, bot).await?,
        "playlist" => handle_playlist(ctx, command, bot).await?,
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
        // Delegar todos los botones musicales al handler especializado
        id if id.starts_with("music_") => {
            crate::ui::buttons::handle_music_component(ctx, &component, bot).await?;
        }
        // Delegar todos los botones de playlist al handler especializado
        id if id.starts_with("playlist_") => {
            crate::ui::buttons::handle_music_component(ctx, &component, bot).await?;
        }
        _ => {
            component
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("❌ Acción no reconocida")
                            .ephemeral(true)
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
    let youtube_client = YouTubeFastClient::new();
    
    let is_url = query.starts_with("http");
    let is_playlist = is_url && crate::sources::youtube::YouTubeClient::is_youtube_playlist(query);
    
    if is_playlist {
        // Es una playlist de YouTube
        info!("📋 Detectada playlist de YouTube: {}", query);
        
        let playlist_tracks = youtube_client.get_playlist(query).await?;
        if playlist_tracks.is_empty() {
            anyhow::bail!("La playlist está vacía o no se pudo acceder");
        }
        
        info!("📋 Playlist cargada con {} canciones", playlist_tracks.len());
        
        // Agregar todas las canciones a la cola
        let queue = bot.player.get_or_create_queue(guild_id);
        let mut added_count = 0;
        
        for track in playlist_tracks {
            let track_with_user = track.with_requested_by(command.user.id);
            
            let mut q = queue.write();
            if let Ok(()) = q.add_track(track_with_user) {
                added_count += 1;
            }
            drop(q); // Liberar el lock antes de la siguiente iteración
        }
        
        // Iniciar reproducción si no hay nada reproduciéndose
        if !bot.player.is_playing(guild_id).await {
            if let Some(handler) = bot.get_voice_handler(guild_id) {
                if let Err(e) = bot.player.play_next(guild_id, handler).await {
                    warn!("Error iniciando reproducción de playlist: {:?}", e);
                }
            }
        }
        
        // Responder con confirmación de playlist mejorada
        let embed = embeds::create_playlist_added_embed(added_count, query);
        let playlist_buttons = crate::ui::buttons::create_playlist_buttons();
        
        use serenity::builder::EditInteractionResponse;
        command
            .edit_response(&ctx.http, EditInteractionResponse::new()
                .embed(embed)
                .components(playlist_buttons)
            )
            .await?;
            
        return Ok(());
        
    } 
    
    // Manejar canciones individuales (URL o búsqueda)
    let mut track_source = if is_url {
        // Es una URL directa de video individual
        youtube_client.get_track(query).await?
    } else {
        // Es una búsqueda - buscar múltiples resultados y filtrar
        info!("🔍 Buscando canción manualmente: {}", query);
        
        // Búsqueda ultrarrápida
        let search_results = youtube_client.search_fast(query, 3).await?;
        
        if !search_results.is_empty() {
            // Tomar el mejor resultado
            let best_result = &search_results[0];
            info!("✅ Mejor resultado encontrado: {}", best_result.title);
            
            // Convertir metadata a TrackSource
            let mut track = TrackSource::new(
                best_result.title.clone(),
                best_result.url.clone().unwrap_or_default(),
                SourceType::YouTube,
                command.user.id,
            );
            
            if let Some(artist) = &best_result.artist {
                track = track.with_artist(artist.clone());
            }
            
            if let Some(duration) = best_result.duration {
                track = track.with_duration(duration);
            }
            
            if let Some(thumbnail) = &best_result.thumbnail {
                track = track.with_thumbnail(thumbnail.clone());
            }
            
            track
        } else {
            // Fallback: buscar con el método original
            warn!("⚠️ Búsqueda rápida no encontró resultados, intentando método tradicional...");
            
            let simple_results = youtube_client.search(query, 5).await?;
            if simple_results.is_empty() {
                anyhow::bail!("No se encontraron resultados para: {}", query);
            }
            
            // Usar el primer track directamente
            let fallback_track = &simple_results[0];
            info!("✅ Resultado fallback encontrado: {}", fallback_track.title());
            
            fallback_track.clone()
        }
    };

    // Establecer el usuario que solicitó la canción
    track_source = track_source.with_requested_by(command.user.id);

    // Agregar a la cola y reproducir
    if let Some(handler) = bot.get_voice_handler(guild_id) {
        bot.player
            .play(guild_id, track_source.clone(), handler)
            .await?;

        // Responder con confirmación de que la canción fue agregada
        let embed = embeds::create_track_added_embed(&track_source);
        use serenity::builder::EditInteractionResponse;
        command
            .edit_response(&ctx.http, EditInteractionResponse::new().embed(embed))
            .await?;

        // Enviar mensaje de "now playing" con botones mejorados en el canal
        // Esperar un momento para que la canción se procese
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        if let Some(current_track) = bot.player.get_current_track(guild_id).await {
            let now_playing_embed = embeds::create_now_playing_embed_from_source(&current_track);
            
            // Verificar si hay cola para mostrar botones mejorados
            let queue_info = bot.player.get_queue_info(guild_id).await?;
            let has_queue = queue_info.total_items > 0;
            let is_playing = bot.player.is_playing(guild_id).await;
            let loop_mode = format!("{:?}", queue_info.loop_mode).to_lowercase();
            
            let buttons = buttons::create_enhanced_player_buttons(is_playing, has_queue, &loop_mode);
            
            command.channel_id.send_message(
                &ctx.http,
                serenity::builder::CreateMessage::new()
                    .embed(now_playing_embed)
                    .components(buttons)
            ).await?;
        }
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

    // Obtener el handler para reproducir la siguiente canción
    if let Some(handler) = bot.get_voice_handler(guild_id) {
        bot.player.skip_tracks(guild_id, amount, handler).await?;
    } else {
        anyhow::bail!("No hay conexión de voz activa");
    }

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
        // **NUEVA IMPLEMENTACIÓN**: Crear embed mejorado con estadísticas de audio
        let mut embed = embeds::create_now_playing_embed_from_source(&current);
        
        // Agregar información del ecualizador
        let eq_details = bot.player.get_equalizer_details();
        embed = embed.field("🎛️ Audio", eq_details, false);
        
        // Agregar estadísticas de volumen
        if let Some(volume) = bot.player.get_volume(guild_id).await {
            let volume_text = format!("{:.0}% ({})", volume * 100.0, 
                if volume > 1.0 { "🔊 Amplificado" } 
                else if volume < 0.3 { "🔉 Bajo" } 
                else { "🔊 Normal" });
            embed = embed.field("🔊 Volumen", volume_text, true);
        }
        
        // Información del procesador
        embed = embed.field("🎧 Procesamiento", "🎵 Audio Nativo", true);
        
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

    // Set the proper loop mode
    let loop_mode = match mode {
        "track" => crate::audio::queue::LoopMode::Track,
        "queue" => crate::audio::queue::LoopMode::Queue,
        _ => crate::audio::queue::LoopMode::Off,
    };
    bot.player.set_loop_mode_specific(guild_id, loop_mode).await?;

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

async fn handle_equalizer(
    ctx: &Context,
    command: CommandInteraction,
    bot: &OpenMusicBot,
) -> Result<()> {
    let guild_id = command.guild_id.unwrap();

    let preset_name = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "preset")
        .and_then(|opt| opt.value.as_str())
        .unwrap_or("flat");

    let preset = match preset_name {
        "bass" => crate::audio::effects::EqualizerPreset::Bass,
        "pop" => crate::audio::effects::EqualizerPreset::Pop,
        "rock" => crate::audio::effects::EqualizerPreset::Rock,
        "jazz" => crate::audio::effects::EqualizerPreset::Jazz,
        "classical" => crate::audio::effects::EqualizerPreset::Classical,
        "electronic" => crate::audio::effects::EqualizerPreset::Electronic,
        "vocal" => crate::audio::effects::EqualizerPreset::Vocal,
        _ => crate::audio::effects::EqualizerPreset::Flat,
    };

    bot.player.apply_equalizer_preset(guild_id, preset).await?;
    
    info!("✅ Ecualizador aplicado: {:?}", preset);

    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content(format!("🎛️ Preset de ecualizador '{}' aplicado", preset_name)),
            ),
        )
        .await?;

    Ok(())
}

async fn handle_clear(
    ctx: &Context,
    command: CommandInteraction,
    bot: &OpenMusicBot,
) -> Result<()> {
    let guild_id = command.guild_id.unwrap();

    let target = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "target")
        .and_then(|opt| opt.value.as_str())
        .unwrap_or("queue");

    match target {
        "queue" => {
            bot.player.clear_queue(guild_id).await?;
            command
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("🗑️ Cola limpiada"),
                    ),
                )
                .await?;
        }
        "duplicates" => {
            let removed = bot.player.clear_duplicates(guild_id).await?;
            command
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content(format!("🗑️ Eliminados {} duplicados", removed)),
                    ),
                )
                .await?;
        }
        "user" => {
            let user = command
                .data
                .options
                .iter()
                .find(|opt| opt.name == "user")
                .and_then(|opt| opt.value.as_user_id())
                .unwrap_or(command.user.id);

            let removed = bot.player.clear_user_tracks(guild_id, user).await?;
            command
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content(format!("🗑️ Eliminadas {} canciones del usuario", removed)),
                    ),
                )
                .await?;
        }
        _ => {
            command
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("❌ Objetivo de limpieza no válido")
                            .ephemeral(true),
                    ),
                )
                .await?;
        }
    }

    Ok(())
}

async fn handle_playlist(
    ctx: &Context,
    command: CommandInteraction,
    bot: &OpenMusicBot,
) -> Result<()> {
    let guild_id = command.guild_id.unwrap();
    
    // Obtener la URL de la playlist del comando
    let playlist_url = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "url")
        .and_then(|opt| opt.value.as_str())
        .ok_or_else(|| anyhow::anyhow!("URL de playlist requerida"))?;

    // Verificar que el usuario esté en un canal de voz
    let voice_channel_id = get_user_voice_channel(ctx, guild_id, command.user.id).await?;

    // Conectar al canal de voz si no está conectado
    if bot.get_voice_handler(guild_id).is_none() {
        bot.join_voice_channel(ctx, guild_id, voice_channel_id)
            .await?;
    }

    // Defer la respuesta porque las playlists pueden tomar tiempo
    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Defer(CreateInteractionResponseMessage::new()),
        )
        .await?;

    info!("🎵 Cargando playlist: {} por {}", playlist_url, command.user.name);

    // Determinar el tipo de playlist
    if playlist_url.contains("youtube.com") || playlist_url.contains("youtu.be") {
        handle_youtube_playlist(ctx, &command, bot, guild_id, playlist_url).await?;
    } else {
        // Intentar como URL directa
        handle_direct_url_playlist(ctx, &command, bot, guild_id, playlist_url).await?;
    }

    Ok(())
}

/// Maneja playlist de YouTube
async fn handle_youtube_playlist(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
    guild_id: GuildId,
    playlist_url: &str,
) -> Result<()> {
    use serenity::builder::EditInteractionResponse;
    
    let youtube_client = crate::sources::YouTubeClient::new();
    
    // Verificar si es una URL de playlist válida
    if !playlist_url.contains("list=") {
        command
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new()
                    .embed(embeds::create_error_embed("Error", "URL de playlist de YouTube inválida. Debe contener 'list='"))
            )
            .await?;
        return Ok(());
    }

    // Obtener tracks de la playlist
    match youtube_client.get_playlist(playlist_url).await {
        Ok(tracks) => {
            if tracks.is_empty() {
                command
                    .edit_response(
                        &ctx.http,
                        EditInteractionResponse::new()
                            .embed(embeds::create_error_embed("Playlist Vacía", "La playlist no contiene canciones válidas"))
                    )
                    .await?;
                return Ok(());
            }

            let track_count = tracks.len();
            let handler = bot.get_voice_handler(guild_id)
                .ok_or_else(|| anyhow::anyhow!("No hay conexión de voz activa"))?;

            // Agregar todas las canciones a la cola
            let mut added_count = 0;
            for track in tracks {
                if let Ok(_) = bot.player.play(guild_id, track, handler.clone()).await {
                    added_count += 1;
                }
            }

            // Crear respuesta de éxito
            let embed = embeds::create_success_embed(
                "🎵 Playlist Agregada",
                &format!("✅ **{}** de **{}** canciones agregadas a la cola\n🎵 Reproduciendo desde YouTube", 
                    added_count, track_count)
            );

            command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().embed(embed)
                )
                .await?;
        }
        Err(e) => {
            command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new()
                        .embed(embeds::create_error_embed("Error", &format!("Error al cargar playlist: {}", e)))
                )
                .await?;
        }
    }

    Ok(())
}


/// Maneja URL directa (intentar como canción individual)
async fn handle_direct_url_playlist(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
    guild_id: GuildId,
    url: &str,
) -> Result<()> {
    use serenity::builder::EditInteractionResponse;
    
    // Intentar agregar como canción individual
    let track_source = TrackSource::new(
        "Audio desde URL".to_string(),
        url.to_string(),
        SourceType::DirectUrl,
        command.user.id,
    );

    let handler = bot.get_voice_handler(guild_id)
        .ok_or_else(|| anyhow::anyhow!("No hay conexión de voz activa"))?;

    match bot.player.play(guild_id, track_source, handler).await {
        Ok(_) => {
            let embed = embeds::create_success_embed(
                "🎵 Audio Agregado",
                "✅ URL directa agregada a la cola"
            );

            command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().embed(embed)
                )
                .await?;
        }
        Err(e) => {
            command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new()
                        .embed(embeds::create_error_embed("Error", &format!("No se pudo cargar el audio: {}", e)))
                )
                .await?;
        }
    }

    Ok(())
}

