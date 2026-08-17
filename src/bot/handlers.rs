//! Despacho de interacciones.
//!
//! ## Cómo se valida un comando
//!
//! Antes cada handler comprobaba a mano lo que se le ocurría: uno miraba si
//! había conexión, otro no, otro respondía "no hay nada sonando" cuando el
//! problema real era que el usuario estaba en otro canal. Ahora hay **una sola
//! puerta**, [`run_command`], que aplica en orden:
//!
//! 1. límite de frecuencia,
//! 2. rol de DJ si el comando lo exige,
//! 3. limpieza de una conexión que Discord ya cerró,
//! 4. la tabla de requisitos de voz según el comando ([`check_voice_connections`]),
//! 5. y recién entonces ejecuta.
//!
//! Cualquier error sube como [`BotError`] y se responde en un único sitio
//! ([`dispatch_command`]), así que es imposible dejar una interacción sin
//! contestar.

use serenity::{
    builder::{
        CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage,
        EditInteractionResponse,
    },
    model::{
        application::{CommandInteraction, ComponentInteraction},
        id::{ChannelId, GuildId, UserId},
    },
    prelude::Context,
};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serenity::prelude::Mentionable;
use tokio::io::AsyncBufReadExt;
use tracing::{info, warn};

use crate::{
    audio::queue::LoopMode,
    bot::{
        connection::{check_voice_connections, get_voice_channel_for_user, Connection},
        OpenMusicBot,
    },
    errors::{verify, BotError, BotResult},
    sources::{MusicSource, SourceType, TrackSource, YtDlpOptimizedClient},
    ui::{buttons, embeds},
};

// ===== RATE LIMITING =====

static RATE_LIMITER: LazyLock<Mutex<HashMap<(GuildId, UserId), (Instant, u32)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const RATE_LIMIT_WINDOW_SECS: u64 = 10;
const RATE_LIMIT_MAX_COMMANDS: u32 = 5;

fn check_rate_limit(guild_id: GuildId, user_id: UserId) -> bool {
    let mut limiter = RATE_LIMITER.lock();
    let key = (guild_id, user_id);
    let now = Instant::now();

    match limiter.get_mut(&key) {
        Some((last_time, count)) => {
            if now.duration_since(*last_time).as_secs() > RATE_LIMIT_WINDOW_SECS {
                *last_time = now;
                *count = 1;
                false
            } else if *count >= RATE_LIMIT_MAX_COMMANDS {
                true
            } else {
                *count += 1;
                false
            }
        }
        None => {
            limiter.insert(key, (now, 1));
            false
        }
    }
}

// ===== PERMISOS =====

const DJ_REQUIRED_COMMANDS: &[&str] = &[
    "stop", "clear", "skip", "remove", "jump", "volume", "equalizer",
];

async fn has_dj_permission(
    ctx: &Context,
    guild_id: GuildId,
    user_id: UserId,
    command_name: &str,
    bot: &OpenMusicBot,
) -> bool {
    if !DJ_REQUIRED_COMMANDS.contains(&command_name) {
        return true;
    }

    let dj_role_id = {
        let storage = bot.storage.lock().await;
        storage.get_dj_role(guild_id.get())
    };

    let Some(dj_role) = dj_role_id.map(serenity::model::id::RoleId::from) else {
        return true; // sin rol configurado, manda cualquiera
    };

    let Ok(member) = guild_id.member(&ctx.http, user_id).await else {
        return false;
    };
    if member.roles.contains(&dj_role) {
        return true;
    }

    ctx.cache
        .guild(guild_id)
        .map(|guild| guild.member_permissions(&member).administrator())
        .unwrap_or(false)
}

/// Comandos que exigen que el bot y quien invoca compartan canal de voz.
const NEEDS_SHARED_CHANNEL: &[&str] = &[
    "pause", "resume", "skip", "stop", "leave", "clear", "shuffle", "loop", "volume", "equalizer",
    "remove", "jump", "previous", "restart", "seek",
];
/// Comandos que conectan al bot al canal de quien invoca.
const NEEDS_AUTHOR_IN_VOICE: &[&str] = &["play", "playlist", "join", "add", "search"];
/// Comandos de sólo lectura del estado de reproducción.
const NEEDS_ANY_CONNECTION: &[&str] = &["nowplaying", "queue"];

// ===== DESPACHO =====

/// Punto de entrada de los comandos slash: ejecuta y, si algo falla, responde.
pub async fn dispatch_command(ctx: &Context, mut command: CommandInteraction, bot: &OpenMusicBot) {
    let name = command.data.name.clone();

    if let Err(err) = run_command(ctx, &mut command, bot).await {
        warn!("/{} falló: {}", name, err);
        respond_text(ctx, &command, &format!("{err}"), true).await;
    }
}

async fn run_command(
    ctx: &Context,
    command: &mut CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    let user_id = command.user.id;
    let command_name = command.data.name.clone();

    if check_rate_limit(guild_id, user_id) {
        return Err(BotError::RateLimited);
    }

    if !has_dj_permission(ctx, guild_id, user_id, &command_name, bot).await {
        return Err(BotError::DjRequired);
    }

    info!(
        "/{} usado por {} en guild {}",
        command_name, command.user.name, guild_id
    );

    // Si Discord ya cerró la conexión por su cuenta, el `Call` que queda es un
    // fantasma. Ojo: `Call::leave()` NO lo saca del manager, así que
    // `manager.get()` seguiría devolviéndolo y el bot se creería conectado.
    // Hay que usar `manager.remove()`, que además de cerrar borra la entrada.
    if let Some(call) = bot.player.call(guild_id) {
        let is_ghost = call.lock().await.current_connection().is_none();
        if is_ghost {
            bot.player.manager().remove(guild_id).await.ok();
            bot.player.forget_guild(guild_id);
        }
    }

    check_voice_requirements(ctx, command, guild_id, &command_name)?;

    match command_name.as_str() {
        "play" => handle_play(ctx, command, bot).await,
        "pause" => handle_pause(ctx, command, bot).await,
        "resume" => handle_resume(ctx, command, bot).await,
        "skip" => handle_skip(ctx, command, bot).await,
        "stop" => handle_stop(ctx, command, bot).await,
        "leave" => handle_leave(ctx, command, bot).await,
        "nowplaying" => handle_nowplaying(ctx, command, bot).await,
        "volume" => handle_volume(ctx, command, bot).await,
        "queue" => handle_queue(ctx, command, bot).await,
        "search" => super::search::handle_search_command(ctx, command.clone(), bot)
            .await
            .map_err(BotError::from),
        "shuffle" => handle_shuffle(ctx, command, bot).await,
        "loop" => handle_loop(ctx, command, bot).await,
        "join" => handle_join(ctx, command, bot).await,
        "equalizer" => handle_equalizer(ctx, command, bot).await,
        "clear" => handle_clear(ctx, command, bot).await,
        "playlist" => handle_playlist(ctx, command, bot).await,
        "previous" => handle_previous(ctx, command, bot).await,
        "restart" => handle_restart(ctx, command, bot).await,
        "seek" => handle_seek(ctx, command, bot).await,
        "add" => handle_add(ctx, command, bot).await,
        "remove" => handle_remove(ctx, command, bot).await,
        "jump" => handle_jump(ctx, command, bot).await,
        "help" => handle_help(ctx, command).await,
        "health" => handle_health(ctx, command, bot).await,
        "metrics" => handle_metrics(ctx, command, bot).await,
        _ => Err(BotError::Other("❌ Comando no reconocido")),
    }
}

/// Tabla de requisitos de voz por comando.
///
/// Traduce la situación real (`Connection`) al error concreto, para que el
/// usuario sepa qué le falta: unirse a un canal, o unirse **al mismo** canal.
fn check_voice_requirements(
    ctx: &Context,
    command: &CommandInteraction,
    guild_id: GuildId,
    command_name: &str,
) -> BotResult<()> {
    let guild = ctx
        .cache
        .guild(guild_id)
        .ok_or(BotError::Other("❌ No encuentro este servidor en caché"))?
        .clone();

    let user_id = command.user.id;
    let bot_id = ctx.cache.current_user().id;
    let state = check_voice_connections(&guild, &user_id, &bot_id);

    if NEEDS_SHARED_CHANNEL.contains(&command_name) {
        return match state {
            Connection::Mutual(..) => Ok(()),
            Connection::User(_) | Connection::Neither => Err(BotError::NotConnected),
            Connection::Bot(bot_channel) => {
                Err(BotError::AuthorDisconnected(bot_channel.mention()))
            }
            Connection::Separate(..) => Err(BotError::WrongVoiceChannel),
        };
    }

    if NEEDS_AUTHOR_IN_VOICE.contains(&command_name) {
        return match state {
            Connection::User(_) | Connection::Mutual(..) => Ok(()),
            Connection::Bot(_) | Connection::Neither => Err(BotError::AuthorNotFound),
            Connection::Separate(bot_channel, _) => {
                Err(BotError::AlreadyConnected(bot_channel.mention()))
            }
        };
    }

    if NEEDS_ANY_CONNECTION.contains(&command_name) {
        return match state {
            Connection::Neither | Connection::User(_) => Err(BotError::NotConnected),
            _ => Ok(()),
        };
    }

    Ok(())
}

// ===== RESPUESTAS =====

/// Responde a la interacción, o edita la respuesta si ya se había diferido.
///
/// Un comando que hizo `defer` no puede volver a crear respuesta; sin este
/// reintento, cualquier error posterior al defer se perdería y el usuario vería
/// "la interacción falló".
async fn respond_text(ctx: &Context, command: &CommandInteraction, content: &str, ephemeral: bool) {
    let message = CreateInteractionResponseMessage::new()
        .content(content)
        .ephemeral(ephemeral);

    if command
        .create_response(&ctx.http, CreateInteractionResponse::Message(message))
        .await
        .is_err()
    {
        let edit = EditInteractionResponse::new().content(content);
        if let Err(e) = command.edit_response(&ctx.http, edit).await {
            warn!("No se pudo responder a la interacción: {e}");
        }
    }
}

async fn reply(ctx: &Context, command: &CommandInteraction, content: impl Into<String>) -> BotResult<()> {
    let message = CreateInteractionResponseMessage::new().content(content);
    command
        .create_response(&ctx.http, CreateInteractionResponse::Message(message))
        .await?;
    Ok(())
}

async fn reply_embed(
    ctx: &Context,
    command: &CommandInteraction,
    embed: CreateEmbed,
    components: Option<Vec<serenity::builder::CreateActionRow>>,
    ephemeral: bool,
) -> BotResult<()> {
    let mut message = CreateInteractionResponseMessage::new()
        .embed(embed)
        .ephemeral(ephemeral);
    if let Some(components) = components {
        message = message.components(components);
    }
    command
        .create_response(&ctx.http, CreateInteractionResponse::Message(message))
        .await?;
    Ok(())
}

/// Difiere la respuesta: obligatorio antes de cualquier trabajo que pueda pasar
/// de los 3 segundos que da Discord (búsquedas y arranques de yt-dlp).
async fn defer(ctx: &Context, command: &CommandInteraction) -> BotResult<()> {
    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Defer(CreateInteractionResponseMessage::new()),
        )
        .await?;
    Ok(())
}

async fn edit(ctx: &Context, command: &CommandInteraction, content: impl Into<String>) -> BotResult<()> {
    command
        .edit_response(&ctx.http, EditInteractionResponse::new().content(content))
        .await?;
    Ok(())
}

// ===== COMPONENTES =====

pub async fn handle_component(
    ctx: &Context,
    component: ComponentInteraction,
    bot: &OpenMusicBot,
) -> anyhow::Result<()> {
    let guild_id = component
        .guild_id
        .ok_or_else(|| anyhow::anyhow!("Componente usado fuera de un servidor"))?;

    info!(
        "Botón {} presionado por {} en guild {}",
        component.data.custom_id, component.user.name, guild_id
    );

    match component.data.custom_id.as_str() {
        "track_selection" => {
            if let serenity::model::application::ComponentInteractionDataKind::StringSelect {
                values,
            } = &component.data.kind
            {
                if let Some(index) = values
                    .first()
                    .and_then(|v| v.strip_prefix("track_"))
                    .and_then(|i| i.parse::<usize>().ok())
                {
                    super::search::handle_track_selection(ctx, &component, bot, index).await?;
                }
            }
        }
        id if id.starts_with("music_") || id.starts_with("playlist_") => {
            buttons::handle_music_component(ctx, &component, bot).await?;
        }
        _ => {
            component
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("❌ Acción no reconocida")
                            .ephemeral(true),
                    ),
                )
                .await?;
        }
    }

    Ok(())
}

// ===== COMANDOS =====

fn option_str<'a>(command: &'a CommandInteraction, name: &str) -> Option<&'a str> {
    command
        .data
        .options
        .iter()
        .find(|opt| opt.name == name)
        .and_then(|opt| opt.value.as_str())
}

fn option_bool(command: &CommandInteraction, name: &str) -> Option<bool> {
    command
        .data
        .options
        .iter()
        .find(|opt| opt.name == name)
        .and_then(|opt| opt.value.as_bool())
}

fn option_i64(command: &CommandInteraction, name: &str) -> Option<i64> {
    command
        .data
        .options
        .iter()
        .find(|opt| opt.name == name)
        .and_then(|opt| opt.value.as_i64())
}

/// Garantiza que el bot esté conectado **y vivo** en el canal de quien invocó.
///
/// No alcanza con preguntar si existe un `Call`: tras echar al bot del canal
/// puede quedar uno registrado pero sin conexión, y darlo por bueno era
/// justamente lo que dejaba al bot mudo (encolaba en un driver muerto). Aquí se
/// comprueba que haya conexión establecida **y** que sea al canal correcto;
/// cualquier otra cosa se descarta y se entra de nuevo.
async fn ensure_connected(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
    guild_id: GuildId,
) -> BotResult<()> {
    let guild = ctx
        .cache
        .guild(guild_id)
        .ok_or(BotError::Other("No encuentro este servidor en caché"))?
        .clone();

    let channel_id =
        get_voice_channel_for_user(&guild, &command.user.id).ok_or(BotError::AuthorNotFound)?;

    if let Some(call) = bot.player.call(guild_id) {
        let handler = call.lock().await;
        let live_here = handler.current_connection().is_some()
            && handler.current_channel().map(|c| ChannelId::from(c.0)) == Some(channel_id);
        drop(handler);

        if live_here {
            return Ok(());
        }

        bot.player.manager().remove(guild_id).await.ok();
    }

    bot.join_voice_channel(ctx, guild_id, channel_id, command.channel_id)
        .await
        .map_err(BotError::from)
}

async fn handle_play(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    let query = option_str(command, "query")
        .ok_or(BotError::Other("❌ Falta la búsqueda o el enlace"))?
        .to_string();

    defer(ctx, command).await?;
    ensure_connected(ctx, command, bot, guild_id).await?;

    let is_url = query.starts_with("http");
    // `list=` cubre tanto /playlist?list=... como watch?v=...&list=..., que es
    // como se comparte una lista desde un video.
    let is_playlist = is_url && query.contains("list=");

    if is_playlist {
        return play_playlist_stream(ctx, command, bot, guild_id, &query).await;
    }

    let was_playing = bot.player.is_playing(guild_id).await;

    let source_manager = crate::sources::SourceManager::new();
    let track = if is_url {
        source_manager
            .get_track_from_url(&query, command.user.id)
            .await
            .map_err(|e| BotError::TrackFail(e.to_string()))?
    } else {
        info!("Buscando: {}", query);
        let results = source_manager
            .search_all(&query, 5)
            .await
            .map_err(|e| BotError::TrackFail(e.to_string()))?;

        let best = results
            .first()
            .and_then(|r| r.tracks.first())
            .cloned()
            .ok_or_else(|| BotError::TrackFail(format!("Sin resultados para «{query}»")))?;

        info!("Elegido: {}", best.title());
        best.with_requested_by(command.user.id)
    };

    bot.player.play(guild_id, track.clone()).await?;

    command
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().embed(embeds::create_track_added_embed(&track)),
        )
        .await?;

    // El "sonando ahora" sólo tiene sentido si este tema arrancó la
    // reproducción; si se sumó a una cola activa, basta con el "agregado".
    if !was_playing {
        send_now_playing(ctx, command.channel_id, bot, guild_id).await;
    }

    Ok(())
}

/// Carga una playlist en streaming: suena el primer tema apenas se conoce y el
/// resto se encola por detrás.
///
/// Leer la lista entera antes de empezar hacía esperar decenas de segundos en
/// listas grandes (y para siempre en los mixes infinitos `list=RD...`).
async fn play_playlist_stream(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
    guild_id: GuildId,
    query: &str,
) -> BotResult<()> {
    /// Tope de temas al cargar una lista con `/play`. Para listas completas
    /// está `/playlist`.
    const PLAY_PLAYLIST_LIMIT: usize = 15;

    info!("Playlist detectada: {}", query);

    let cookies = YtDlpOptimizedClient::cookies_working_copy();
    let mut child =
        YtDlpOptimizedClient::spawn_playlist_stream(query, cookies.as_deref(), Some(PLAY_PLAYLIST_LIMIT))
            .map_err(|e| BotError::TrackFail(format!("No pude leer la playlist: {e}")))?;

    let stdout = child
        .stdout
        .take()
        .ok_or(BotError::Other("❌ yt-dlp no devolvió salida"))?;
    let mut lines = tokio::io::BufReader::new(stdout).lines();

    let user_id = command.user.id;

    let mut first_track = None;
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(track) = YtDlpOptimizedClient::parse_playlist_line(&line, user_id) {
            first_track = Some(track);
            break;
        }
    }
    let first_track = first_track
        .ok_or_else(|| BotError::TrackFail("La playlist está vacía".to_string()))?;

    bot.player.play(guild_id, first_track.clone()).await?;

    command
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new()
                .embed(embeds::create_track_added_embed(&first_track))
                .components(buttons::create_playlist_buttons()),
        )
        .await?;

    let player = bot.player.clone();
    tokio::spawn(async move {
        let mut count = 1usize;
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(track) = YtDlpOptimizedClient::parse_playlist_line(&line, user_id) {
                if player.play(guild_id, track).await.is_ok() {
                    count += 1;
                }
            }
        }
        child.wait().await.ok();
        info!("Playlist encolada: {} canciones", count);
    });

    Ok(())
}

/// Publica el embed de "sonando ahora" en el canal de texto.
async fn send_now_playing(
    ctx: &Context,
    channel_id: ChannelId,
    bot: &OpenMusicBot,
    guild_id: GuildId,
) {
    // Dar un instante a que la pista arranque de verdad antes de anunciarla.
    tokio::time::sleep(Duration::from_millis(500)).await;

    let Some(current) = bot.player.get_current_track(guild_id).await else {
        return;
    };
    let Ok(queue_info) = bot.player.get_queue_info(guild_id).await else {
        return;
    };

    let embed = embeds::create_now_playing_embed_from_source(&current);
    let controls = buttons::create_enhanced_player_buttons(
        bot.player.is_playing(guild_id).await,
        queue_info.total_items > 0,
        &format!("{:?}", queue_info.loop_mode).to_lowercase(),
    );

    if let Err(e) = channel_id
        .send_message(
            &ctx.http,
            serenity::builder::CreateMessage::new()
                .embed(embed)
                .components(controls),
        )
        .await
    {
        warn!("No se pudo enviar el «sonando ahora»: {e}");
    }
}

async fn handle_pause(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    verify(bot.player.is_playing(guild_id).await, BotError::NothingPlaying)?;

    bot.player.pause(guild_id).await?;
    reply(ctx, command, "⏸️ Reproducción pausada").await
}

async fn handle_resume(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    verify(
        bot.player.current_meta(guild_id).await.is_some(),
        BotError::QueueEmpty,
    )?;

    bot.player.resume(guild_id).await?;
    reply(ctx, command, "▶️ Reproducción reanudada").await
}

async fn handle_skip(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    let amount = option_i64(command, "amount").unwrap_or(1).max(1) as usize;

    verify(
        bot.player.current_meta(guild_id).await.is_some(),
        BotError::NothingPlaying,
    )?;

    defer(ctx, command).await?;
    bot.player.skip_tracks(guild_id, amount).await?;

    let content = match bot.player.current_meta(guild_id).await {
        Some(next) => format!("⏭️ Ahora suena: **{}**", next.title),
        None => "⏭️ Cola terminada".to_string(),
    };
    edit(ctx, command, content).await
}

async fn handle_stop(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    verify(
        bot.player.current_meta(guild_id).await.is_some(),
        BotError::NothingPlaying,
    )?;

    bot.player.stop(guild_id).await?;
    reply(
        ctx,
        command,
        "⏹️ Reproducción detenida y cola limpiada (me quedo por acá un rato)",
    )
    .await
}

async fn handle_leave(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;

    bot.player.stop(guild_id).await?;
    bot.leave_voice_channel(ctx, guild_id).await?;
    reply(ctx, command, "👋 Desconectado del canal de voz").await
}

async fn handle_join(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    ensure_connected(ctx, command, bot, guild_id).await?;
    reply(ctx, command, "🔊 Conectado al canal de voz").await
}

async fn handle_queue(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    let page = option_i64(command, "page").unwrap_or(1).max(1) as usize;

    let queue_info = bot.player.get_queue_info(guild_id).await?;
    let embed = embeds::create_queue_embed(&queue_info, page);

    reply_embed(ctx, command, embed, None, false).await
}

async fn handle_nowplaying(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    let current = bot
        .player
        .get_current_track(guild_id)
        .await
        .ok_or(BotError::NothingPlaying)?;

    let mut embed = embeds::create_now_playing_embed_from_source(&current);
    embed = embed.field("Audio", bot.player.get_equalizer_details(guild_id), false);

    if let Some(volume) = bot.player.get_volume(guild_id).await {
        let etiqueta = if volume > 1.0 {
            "Amplificado"
        } else if volume < 0.3 {
            "Bajo"
        } else {
            "Normal"
        };
        embed = embed.field(
            "Volumen",
            format!("{:.0}% ({})", volume * 100.0, etiqueta),
            true,
        );
    }

    if let Some(position) = bot.player.current_position(guild_id).await {
        embed = embed.field("Posición", format_duration(position), true);
    }

    reply_embed(
        ctx,
        command,
        embed,
        Some(buttons::create_player_buttons()),
        false,
    )
    .await
}

async fn handle_shuffle(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    let shuffled = bot.player.toggle_shuffle(guild_id).await?;

    reply(
        ctx,
        command,
        if shuffled {
            "🔀 Modo aleatorio activado (cola mezclada)"
        } else {
            "➡️ Modo aleatorio desactivado"
        },
    )
    .await
}

async fn handle_loop(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    let mode = option_str(command, "mode").unwrap_or("off");

    let (loop_mode, message) = match mode {
        "track" => (LoopMode::Track, "🔂 Repetir canción activado"),
        "queue" => (LoopMode::Queue, "🔁 Repetir cola activado"),
        _ => (LoopMode::Off, "➡️ Repetición desactivada"),
    };

    bot.player.set_loop_mode_specific(guild_id, loop_mode).await?;
    reply(ctx, command, message).await
}

async fn handle_volume(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;

    let Some(level) = option_i64(command, "level") else {
        let current = bot.player.get_volume(guild_id).await.unwrap_or(0.5);
        let percent = (current * 100.0) as i32;
        let emoji = volume_emoji(percent);
        return reply(ctx, command, format!("{emoji} Volumen actual: {percent}%")).await;
    };

    if !(0..=200).contains(&level) {
        return Err(BotError::NotInRange("volumen", level as isize, 0, 200));
    }

    bot.player
        .set_volume(guild_id, (level as f32 / 100.0).clamp(0.0, 2.0))
        .await?;

    let message = if level > 100 {
        format!("Volumen al {level}%\nPor encima del 100% puede distorsionar")
    } else if level == 0 {
        "🔇 Audio silenciado".to_string()
    } else {
        format!("{} Volumen al {}%", volume_emoji(level as i32), level)
    };

    reply(ctx, command, message).await
}

fn volume_emoji(percent: i32) -> &'static str {
    match percent {
        0 => "",
        1..=30 => "",
        _ => "",
    }
}

async fn handle_previous(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;

    defer(ctx, command).await?;
    let previous = bot
        .player
        .play_previous(guild_id)
        .await
        .map_err(|_| BotError::NoHistory)?;

    edit(ctx, command, format!("Volviendo a: **{}**", previous.title)).await
}

async fn handle_restart(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;

    defer(ctx, command).await?;
    let current = bot
        .player
        .restart_current(guild_id)
        .await
        .map_err(|_| BotError::NothingPlaying)?;

    edit(ctx, command, format!("🔁 Reiniciando: **{}**", current.title)).await
}

async fn handle_seek(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    let time_str = option_str(command, "time").ok_or(BotError::Other(
        "❌ Indicá la posición (por ejemplo `1:30`)",
    ))?;

    let seconds = parse_time_string(time_str)?;
    let position = Duration::from_secs(seconds);

    let current = bot
        .player
        .current_meta(guild_id)
        .await
        .ok_or(BotError::NothingPlaying)?;

    if let Some(duration) = current.duration {
        if position >= duration {
            return Err(BotError::NotInRange(
                "posición",
                seconds as isize,
                0,
                duration.as_secs() as isize,
            ));
        }
    }

    defer(ctx, command).await?;
    let track = bot
        .player
        .seek_current(guild_id, position)
        .await
        .map_err(|e| BotError::TrackFail(e.to_string()))?;

    edit(
        ctx,
        command,
        format!("⏩ **{}** desde {}", track.title, format_duration(position)),
    )
    .await
}

async fn handle_add(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    let query = option_str(command, "query")
        .ok_or(BotError::Other("❌ Falta la búsqueda"))?
        .to_string();

    defer(ctx, command).await?;
    ensure_connected(ctx, command, bot, guild_id).await?;

    let source_manager = crate::sources::SourceManager::new();
    let results = source_manager
        .search_all(&query, 1)
        .await
        .map_err(|e| BotError::TrackFail(e.to_string()))?;

    let track = results
        .first()
        .and_then(|r| r.tracks.first())
        .cloned()
        .ok_or_else(|| BotError::TrackFail(format!("Sin resultados para «{query}»")))?
        .with_requested_by(command.user.id);

    let title = track.title();
    bot.player.play(guild_id, track).await?;

    edit(ctx, command, format!("➕ **{title}** agregado a la cola")).await
}

async fn handle_remove(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    let position = option_i64(command, "position")
        .ok_or(BotError::Other("❌ Indicá la posición a quitar"))?
        .max(0) as usize;

    let removed = bot
        .player
        .remove_track(guild_id, position)
        .await
        .map_err(|e| BotError::Dynamic(format!("{e}")))?;

    reply(
        ctx,
        command,
        format!("🗑️ Quitada de la cola: **{}**", removed.title),
    )
    .await
}

async fn handle_jump(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    let position = option_i64(command, "position")
        .ok_or(BotError::Other("❌ Indicá la posición"))?
        .max(0) as usize;

    defer(ctx, command).await?;
    let target = bot
        .player
        .jump_to(guild_id, position)
        .await
        .map_err(|e| BotError::Dynamic(format!("{e}")))?;

    edit(
        ctx,
        command,
        format!("🎯 Saltando a la posición {position}: **{}**", target.title),
    )
    .await
}

async fn handle_clear(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    let target = option_str(command, "target").unwrap_or("queue");

    let message = match target {
        "queue" => {
            bot.player.clear_queue(guild_id).await?;
            "🗑️ Cola limpiada".to_string()
        }
        "duplicates" => {
            let removed = bot.player.clear_duplicates(guild_id).await?;
            format!("🗑️ Eliminados {removed} duplicados")
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
            format!("🗑️ Eliminadas {removed} canciones de {}", user.mention())
        }
        _ => return Err(BotError::Other("❌ Objetivo de limpieza no válido")),
    };

    reply(ctx, command, message).await
}

async fn handle_equalizer(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    use crate::audio::effects::EqualizerPreset;

    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    let preset_name = option_str(command, "preset").unwrap_or("flat");

    let preset = match preset_name {
        "bass" => EqualizerPreset::Bass,
        "pop" => EqualizerPreset::Pop,
        "rock" => EqualizerPreset::Rock,
        "jazz" => EqualizerPreset::Jazz,
        "classical" => EqualizerPreset::Classical,
        "electronic" => EqualizerPreset::Electronic,
        "vocal" => EqualizerPreset::Vocal,
        _ => EqualizerPreset::Flat,
    };

    bot.player.apply_equalizer_preset(guild_id, preset).await?;

    // El filtro se resuelve al abrir el stream de cada pista, así que la que ya
    // está sonando conserva el preset viejo; el resto de la cola ya sale con el
    // nuevo sin tocar nada.
    let content = if bot.player.is_playing(guild_id).await {
        format!(
            "Preset **{preset_name}** activado.\nSe oye desde la próxima canción (o usá `/restart` para aplicarlo ya)."
        )
    } else {
        format!("🎛️ Preset **{preset_name}** aplicado")
    };

    reply(ctx, command, content).await
}

async fn handle_playlist(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let guild_id = command.guild_id.ok_or(BotError::NotInGuild)?;
    let url = option_str(command, "url")
        .ok_or(BotError::Other("❌ Falta la URL de la playlist"))?
        .to_string();

    defer(ctx, command).await?;
    ensure_connected(ctx, command, bot, guild_id).await?;

    let result = if url.contains("youtube.com") || url.contains("youtu.be") {
        load_youtube_playlist(ctx, command, bot, guild_id, &url).await
    } else {
        load_direct_url(ctx, command, bot, guild_id, &url).await
    };

    // La opción `shuffle` del comando estaba declarada pero nunca se usaba.
    if result.is_ok() && option_bool(command, "shuffle").unwrap_or(false) {
        bot.player.shuffle_pending(guild_id).await;
    }

    result
}

/// Carga una playlist completa de YouTube mostrando el progreso.
async fn load_youtube_playlist(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
    guild_id: GuildId,
    playlist_url: &str,
) -> BotResult<()> {
    if !playlist_url.contains("list=") {
        return Err(BotError::Other(
            "❌ La URL no es de una playlist (debe contener `list=`)",
        ));
    }

    command
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new()
                .embed(embeds::create_playlist_loading_embed(
                    "Analizando playlist...",
                    0,
                    0,
                    &[],
                    playlist_url,
                ))
                .components(buttons::MusicControls::create_playlist_loading_controls(None)),
        )
        .await?;

    let client = YtDlpOptimizedClient::new();
    let tracks = client
        .get_playlist(playlist_url)
        .await
        .map_err(|e| BotError::TrackFail(format!("No pude cargar la playlist: {e}")))?;

    if tracks.is_empty() {
        return Err(BotError::TrackFail(
            "La playlist no tiene canciones válidas".to_string(),
        ));
    }

    let total = tracks.len();
    let mut added = 0usize;
    let mut failed = 0usize;
    let mut recent: Vec<String> = Vec::new();
    let mut total_duration = Duration::ZERO;

    for (i, track) in tracks.iter().enumerate() {
        let current = i + 1;

        if current % 5 == 0 || current == total {
            let progress = EditInteractionResponse::new()
                .embed(embeds::create_playlist_loading_embed(
                    "Cargando playlist...",
                    current,
                    total,
                    &recent,
                    playlist_url,
                ))
                .components(buttons::MusicControls::create_playlist_loading_controls(
                    Some((current, total)),
                ));

            if let Err(e) = command.edit_response(&ctx.http, progress).await {
                warn!("No se pudo actualizar el progreso de la playlist: {e}");
            }
        }

        match bot.player.play(guild_id, track.clone()).await {
            Ok(()) => {
                added += 1;
                recent.push(track.title());
                if recent.len() > 10 {
                    recent.remove(0);
                }
                if let Some(duration) = track.duration() {
                    total_duration += duration;
                }
            }
            Err(e) => {
                failed += 1;
                warn!("No se pudo encolar «{}»: {:?}", track.title(), e);
            }
        }
    }

    let final_embed = embeds::create_playlist_completed_embed(
        "Playlist de YouTube",
        added,
        total,
        failed,
        (total_duration > Duration::ZERO).then_some(total_duration),
        playlist_url,
    );

    command
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new()
                .embed(final_embed)
                .components(if added > 0 {
                    buttons::create_playlist_buttons()
                } else {
                    vec![]
                }),
        )
        .await?;

    info!("Playlist cargada: {}/{} canciones", added, total);
    Ok(())
}

async fn load_direct_url(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
    guild_id: GuildId,
    url: &str,
) -> BotResult<()> {
    let track = TrackSource::new(
        "Audio desde URL".to_string(),
        url.to_string(),
        SourceType::DirectUrl,
        command.user.id,
    );

    bot.player.play(guild_id, track).await?;

    command
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().embed(embeds::create_success_embed(
                "🎵 Audio agregado",
                "✅ URL directa agregada a la cola",
            )),
        )
        .await?;

    Ok(())
}

async fn handle_help(ctx: &Context, command: &CommandInteraction) -> BotResult<()> {
    let embed = match option_str(command, "command") {
        Some(cmd) => embeds::create_command_help_embed(cmd),
        None => embeds::create_help_embed(),
    };

    reply_embed(ctx, command, embed, None, true).await
}

async fn handle_health(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let health = bot.monitoring.perform_health_check().await;
    let metrics = bot.monitoring.get_system_metrics().await;

    let emoji = match health {
        crate::monitoring::HealthStatus::Healthy => "",
        crate::monitoring::HealthStatus::Warning => "",
        crate::monitoring::HealthStatus::Critical => "",
        crate::monitoring::HealthStatus::Unknown => "",
    };

    let embed = embeds::create_info_embed(
        &format!("{emoji} Estado de salud del bot"),
        &format!(
            "**Estado**: {:?}\n**Tiempo activo**: {:?}\n**Comandos procesados**: {}\n**Errores**: {}\n**Tasa de error**: {:.2}%",
            health, metrics.uptime, metrics.total_commands, metrics.total_errors, metrics.error_rate
        ),
    );

    reply_embed(ctx, command, embed, None, true).await
}

async fn handle_metrics(
    ctx: &Context,
    command: &CommandInteraction,
    bot: &OpenMusicBot,
) -> BotResult<()> {
    let embed = match option_str(command, "type").unwrap_or("performance") {
        "errors" => {
            let report = bot.monitoring.get_error_report(Some(24)).await;
            let mut description =
                format!("**Errores en las últimas 24 h**: {}\n\n", report.total_errors);
            for category in report.categories.iter().take(5) {
                description.push_str(&format!(
                    "**{}**: {} errores\n",
                    category.category, category.total_count
                ));
            }
            embeds::create_info_embed("🔍 Reporte de errores", &description)
        }
        "performance" => {
            let metrics = bot.monitoring.get_system_metrics().await;
            embeds::create_info_embed(
                "📊 Métricas de rendimiento",
                &format!(
                    "**Tiempo activo**: {:?}\n**Comandos totales**: {}\n**Tasa de error**: {:.2}%\n**Estado**: {:?}",
                    metrics.uptime, metrics.total_commands, metrics.error_rate, metrics.health_status
                ),
            )
        }
        _ => {
            let metrics = bot.monitoring.get_system_metrics().await;
            embeds::create_info_embed(
                "📈 Métricas del sistema",
                &format!(
                    "**Tiempo activo**: {:?}\n**Comandos**: {}\n**Errores**: {}\n**Avisos**: {}",
                    metrics.uptime,
                    metrics.total_commands,
                    metrics.total_errors,
                    metrics.total_warnings
                ),
            )
        }
    };

    reply_embed(ctx, command, embed, None, true).await
}

// ===== UTILIDADES =====

/// Acepta `90`, `1:30` y `1:30:00`.
fn parse_time_string(time_str: &str) -> BotResult<u64> {
    let invalid = || BotError::Other("❌ Formato de tiempo inválido. Usá `seg`, `min:seg` o `hora:min:seg`");

    let parts: Vec<u64> = time_str
        .split(':')
        .map(|p| p.trim().parse::<u64>())
        .collect::<Result<_, _>>()
        .map_err(|_| invalid())?;

    match parts.as_slice() {
        [s] => Ok(*s),
        [m, s] => Ok(m * 60 + s),
        [h, m, s] => Ok(h * 3600 + m * 60 + s),
        _ => Err(invalid()),
    }
}

fn format_duration(duration: Duration) -> String {
    let total = duration.as_secs();
    let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);

    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_formatos_de_tiempo() {
        assert_eq!(parse_time_string("90").unwrap(), 90);
        assert_eq!(parse_time_string("1:30").unwrap(), 90);
        assert_eq!(parse_time_string("1:30:00").unwrap(), 5400);
        assert!(parse_time_string("abc").is_err());
        assert!(parse_time_string("1:2:3:4").is_err());
    }

    #[test]
    fn formatea_duraciones() {
        assert_eq!(format_duration(Duration::from_secs(90)), "1:30");
        assert_eq!(format_duration(Duration::from_secs(5400)), "1:30:00");
    }
}
