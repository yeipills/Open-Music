use anyhow::Result;
use serenity::{
    all::{ButtonStyle, Colour, ComponentInteraction, Context, Timestamp},
    builder::{CreateActionRow, CreateButton, CreateEmbed, CreateEmbedFooter},
};
use std::time::Duration;
use tracing::{debug, error};

use crate::{
    audio::player::AudioPlayer, 
    sources::TrackSource,
};
use serenity::model::id::GuildId;

/// IDs personalizados para los botones
#[allow(dead_code)]
pub mod button_ids {
    pub const PLAY_PAUSE: &str = "music_play_pause";
    pub const SKIP: &str = "music_skip";
    pub const STOP: &str = "music_stop";
    pub const SHUFFLE: &str = "music_shuffle";
    pub const LOOP_TRACK: &str = "music_loop";
    pub const QUEUE: &str = "music_queue";
    pub const VOLUME_UP: &str = "music_volume_up";
    pub const VOLUME_DOWN: &str = "music_volume_down";
    pub const EFFECTS: &str = "music_effects";
    pub const PREVIOUS_PAGE: &str = "music_prev_page";
    pub const NEXT_PAGE: &str = "queue_next";
    
    // Botones específicos para playlists
    pub const PLAYLIST_LOAD: &str = "playlist_load";
    pub const PLAYLIST_PREVIEW: &str = "playlist_preview";
    pub const PLAYLIST_CONFIRM: &str = "playlist_confirm";
    pub const PLAYLIST_CANCEL: &str = "playlist_cancel";
    pub const PLAYLIST_INFO: &str = "playlist_info";
}

/// Constructor de controles de música
#[allow(dead_code)]
pub struct MusicControls;

impl MusicControls {
    /// Crea los controles principales del reproductor
    #[allow(dead_code)]
    pub fn create_player_controls(
        is_playing: bool,
        is_looping: bool,
        is_shuffled: bool,
    ) -> Vec<CreateActionRow> {
        let play_pause_emoji = if is_playing { "⏸️" } else { "▶️" };
        let loop_style = if is_looping {
            ButtonStyle::Success
        } else {
            ButtonStyle::Secondary
        };
        let shuffle_style = if is_shuffled {
            ButtonStyle::Success
        } else {
            ButtonStyle::Secondary
        };

        // First row of buttons
        let play_pause_btn = CreateButton::new(button_ids::PLAY_PAUSE)
            .emoji(play_pause_emoji.chars().next().unwrap())
            .style(ButtonStyle::Primary);

        let skip_btn = CreateButton::new(button_ids::SKIP)
            .emoji('⏭')
            .style(ButtonStyle::Secondary);

        let stop_btn = CreateButton::new(button_ids::STOP)
            .emoji('⏹')
            .style(ButtonStyle::Danger);

        let shuffle_btn = CreateButton::new(button_ids::SHUFFLE)
            .emoji('🔀')
            .style(shuffle_style);

        let loop_btn = CreateButton::new(button_ids::LOOP_TRACK)
            .emoji('🔁')
            .style(loop_style);

        let row1 = CreateActionRow::Buttons(vec![
            play_pause_btn,
            skip_btn,
            stop_btn,
            shuffle_btn,
            loop_btn,
        ]);

        // Second row of buttons
        let vol_down_btn = CreateButton::new(button_ids::VOLUME_DOWN)
            .emoji('🔉')
            .style(ButtonStyle::Secondary);

        let vol_up_btn = CreateButton::new(button_ids::VOLUME_UP)
            .emoji('🔊')
            .style(ButtonStyle::Secondary);

        let queue_btn = CreateButton::new(button_ids::QUEUE)
            .label("Cola")
            .emoji('📋')
            .style(ButtonStyle::Secondary);

        let effects_btn = CreateButton::new(button_ids::EFFECTS)
            .label("Efectos")
            .emoji('🎛')
            .style(ButtonStyle::Secondary);

        let row2 = CreateActionRow::Buttons(vec![vol_down_btn, vol_up_btn, queue_btn, effects_btn]);

        vec![row1, row2]
    }

    /// Crea controles de paginación para la cola
    #[allow(dead_code)]
    pub fn create_pagination_controls(current_page: usize, total_pages: usize) -> CreateActionRow {
        let prev_btn = CreateButton::new(button_ids::PREVIOUS_PAGE)
            .emoji('◀')
            .style(ButtonStyle::Primary)
            .disabled(current_page == 0);

        let next_btn = CreateButton::new(button_ids::NEXT_PAGE)
            .emoji('▶')
            .style(ButtonStyle::Primary)
            .disabled(current_page >= total_pages - 1);

        let close_btn = CreateButton::new("close")
            .label("Cerrar")
            .style(ButtonStyle::Danger);

        let row = CreateActionRow::Buttons(vec![prev_btn, next_btn, close_btn]);

        row
    }
    
    /// Crea controles específicos para playlists
    #[allow(dead_code)]
    pub fn create_playlist_controls() -> Vec<CreateActionRow> {
        // Primera fila: Controles básicos de playlist
        let load_btn = CreateButton::new(button_ids::PLAYLIST_LOAD)
            .label("Cargar")
            .emoji('📥')
            .style(ButtonStyle::Success);

        let preview_btn = CreateButton::new(button_ids::PLAYLIST_PREVIEW)
            .label("Vista Previa")
            .emoji('👁')
            .style(ButtonStyle::Secondary);

        let info_btn = CreateButton::new(button_ids::PLAYLIST_INFO)
            .label("Info")
            .emoji('ℹ')
            .style(ButtonStyle::Secondary);

        let row1 = CreateActionRow::Buttons(vec![load_btn, preview_btn, info_btn]);

        vec![row1]
    }
    
    /// Crea controles de confirmación para playlists
    #[allow(dead_code)]
    pub fn create_playlist_confirmation_controls() -> CreateActionRow {
        let confirm_btn = CreateButton::new(button_ids::PLAYLIST_CONFIRM)
            .label("Sí, agregar playlist")
            .emoji('✅')
            .style(ButtonStyle::Success);

        let cancel_btn = CreateButton::new(button_ids::PLAYLIST_CANCEL)
            .label("Cancelar")
            .emoji('❌')
            .style(ButtonStyle::Danger);

        CreateActionRow::Buttons(vec![confirm_btn, cancel_btn])
    }
    
    /// Crea botones mejorados para el reproductor con más opciones
    #[allow(dead_code)]
    pub fn create_enhanced_player_buttons(
        is_playing: bool,
        has_queue: bool,
        loop_mode: &str
    ) -> Vec<CreateActionRow> {
        let play_pause_emoji = if is_playing { "⏸️" } else { "▶️" };
        let loop_emoji = match loop_mode {
            "track" => "🔂",
            "queue" => "🔁",
            _ => "🔁",
        };
        
        // Primera fila: Controles principales
        let play_pause_btn = CreateButton::new(button_ids::PLAY_PAUSE)
            .emoji(play_pause_emoji.chars().next().unwrap())
            .style(if is_playing { ButtonStyle::Secondary } else { ButtonStyle::Success });

        let skip_btn = CreateButton::new(button_ids::SKIP)
            .emoji('⏭')
            .style(ButtonStyle::Primary)
            .disabled(!has_queue);

        let stop_btn = CreateButton::new(button_ids::STOP)
            .emoji('⏹')
            .style(ButtonStyle::Danger);

        let shuffle_btn = CreateButton::new(button_ids::SHUFFLE)
            .emoji('🔀')
            .style(ButtonStyle::Secondary);

        let loop_btn = CreateButton::new(button_ids::LOOP_TRACK)
            .emoji(loop_emoji.chars().next().unwrap())
            .style(ButtonStyle::Secondary);

        let row1 = CreateActionRow::Buttons(vec![
            play_pause_btn,
            skip_btn,
            stop_btn,
            shuffle_btn,
            loop_btn,
        ]);

        // Segunda fila: Controles de audio y información
        let vol_down_btn = CreateButton::new(button_ids::VOLUME_DOWN)
            .emoji('🔉')
            .style(ButtonStyle::Secondary);

        let vol_up_btn = CreateButton::new(button_ids::VOLUME_UP)
            .emoji('🔊')
            .style(ButtonStyle::Secondary);

        let queue_btn = CreateButton::new(button_ids::QUEUE)
            .label("Cola")
            .emoji('📋')
            .style(ButtonStyle::Secondary);

        let effects_btn = CreateButton::new(button_ids::EFFECTS)
            .label("Efectos")
            .emoji('🎛')
            .style(ButtonStyle::Secondary);

        let row2 = CreateActionRow::Buttons(vec![vol_down_btn, vol_up_btn, queue_btn, effects_btn]);

        vec![row1, row2]
    }
}

/// Función de utilidad para crear controles básicos del reproductor
pub fn create_player_buttons() -> Vec<CreateActionRow> {
    MusicControls::create_player_controls(false, false, false)
}

/// Función de utilidad para crear controles específicos de playlist
pub fn create_playlist_buttons() -> Vec<CreateActionRow> {
    MusicControls::create_playlist_controls()
}

/// Función de utilidad para crear controles mejorados del reproductor
pub fn create_enhanced_player_buttons(is_playing: bool, has_queue: bool, loop_mode: &str) -> Vec<CreateActionRow> {
    MusicControls::create_enhanced_player_buttons(is_playing, has_queue, loop_mode)
}

/// Crea botón de reintentar para errores
#[allow(dead_code)]
pub fn create_retry_button() -> CreateActionRow {
    let retry_btn = CreateButton::new("retry_action")
        .label("Reintentar")
        .emoji('🔄')
        .style(ButtonStyle::Primary);
    
    CreateActionRow::Buttons(vec![retry_btn])
}

/// Crea botones de confirmación estándar (Sí/No)
#[allow(dead_code)]
pub fn create_confirmation_buttons(action_id: &str) -> CreateActionRow {
    let confirm_btn = CreateButton::new(format!("confirm_{}", action_id))
        .label("Sí, confirmar")
        .emoji('✅')
        .style(ButtonStyle::Success);
    
    let cancel_btn = CreateButton::new("cancel_action")
        .label("Cancelar")
        .emoji('❌')
        .style(ButtonStyle::Danger);
    
    CreateActionRow::Buttons(vec![confirm_btn, cancel_btn])
}

/// Crea botones de navegación mejorados
#[allow(dead_code)]
pub fn create_navigation_buttons(has_prev: bool, has_next: bool, current_page: usize, total_pages: usize) -> CreateActionRow {
    let first_btn = CreateButton::new("nav_first")
        .emoji('⏪')
        .style(ButtonStyle::Secondary)
        .disabled(!has_prev || current_page == 1);
    
    let prev_btn = CreateButton::new("nav_prev")
        .emoji('◀')
        .style(ButtonStyle::Primary)
        .disabled(!has_prev);
    
    let page_btn = CreateButton::new("nav_info")
        .label(format!("{}/{}", current_page, total_pages))
        .style(ButtonStyle::Secondary)
        .disabled(true);
    
    let next_btn = CreateButton::new("nav_next")
        .emoji('▶')
        .style(ButtonStyle::Primary)
        .disabled(!has_next);
    
    let last_btn = CreateButton::new("nav_last")
        .emoji('⏩')
        .style(ButtonStyle::Secondary)
        .disabled(!has_next || current_page == total_pages);
    
    CreateActionRow::Buttons(vec![first_btn, prev_btn, page_btn, next_btn, last_btn])
}

/// Crea botones de control de volumen
#[allow(dead_code)]
pub fn create_volume_control_buttons(current_volume: f32) -> CreateActionRow {
    let mute_btn = CreateButton::new("volume_mute")
        .emoji(if current_volume == 0.0 { '🔊' } else { '🔇' })
        .style(if current_volume == 0.0 { ButtonStyle::Success } else { ButtonStyle::Secondary });
    
    let vol_down_btn = CreateButton::new("volume_down_big")
        .label("-10")
        .emoji('🔉')
        .style(ButtonStyle::Secondary)
        .disabled(current_volume <= 0.0);
    
    let vol_down_small_btn = CreateButton::new("volume_down_small")
        .label("-5")
        .style(ButtonStyle::Secondary)
        .disabled(current_volume <= 0.0);
    
    let vol_up_small_btn = CreateButton::new("volume_up_small")
        .label("+5")
        .style(ButtonStyle::Secondary)
        .disabled(current_volume >= 2.0);
    
    let vol_up_btn = CreateButton::new("volume_up_big")
        .label("+10")
        .emoji('🔊')
        .style(ButtonStyle::Secondary)
        .disabled(current_volume >= 2.0);
    
    CreateActionRow::Buttons(vec![mute_btn, vol_down_btn, vol_down_small_btn, vol_up_small_btn, vol_up_btn])
}

/// Constructor de embeds para el reproductor
#[allow(dead_code)]
pub struct MusicEmbeds;

impl MusicEmbeds {
    /// Crea un embed para la canción actual
    #[allow(dead_code)]
    pub fn now_playing(track: &TrackSource, is_playing: bool) -> CreateEmbed {
        let status = if is_playing {
            "▶️ Reproduciendo"
        } else {
            "⏸️ Pausado"
        };
        let progress_bar = Self::create_progress_bar(0.0, 20); // Progress not available in static context

        let embed = CreateEmbed::default()
            .title(status)
            .description(&track.title())
            .field(
                "Artista",
                track.artist().unwrap_or("Desconocido".to_string()),
                true,
            )
            .field("Duración", Self::format_duration(track.duration()), true)
            .field("Fuente", format!("{:?}", track.source_type()), true)
            .field("Progreso", progress_bar, false)
            .colour(Colour::from_rgb(255, 73, 108))
            .thumbnail(track.thumbnail().unwrap_or_default())
            .footer(CreateEmbedFooter::new("🎵 Open Music Bot"))
            .timestamp(Timestamp::now());
        embed
    }

    /// Crea un embed para la cola
    #[allow(dead_code)]
    pub fn queue_embed(
        tracks: &[TrackSource],
        page: usize,
        total_pages: usize,
        total_duration: Duration,
    ) -> CreateEmbed {
        let tracks_per_page = 10;
        let start = page * tracks_per_page;
        let end = (start + tracks_per_page).min(tracks.len());

        let mut description = String::new();
        for (i, track) in tracks[start..end].iter().enumerate() {
            let position = start + i + 1;
            description.push_str(&format!(
                "**{}**. {} - {}\n",
                position,
                track.title(),
                Self::format_duration(Some(track.duration().unwrap_or_default()))
            ));
        }

        if description.is_empty() {
            description = "La cola está vacía".to_string();
        }

        let embed = CreateEmbed::default()
            .title("📋 Cola de Reproducción")
            .description(description)
            .field("Total de canciones", tracks.len().to_string(), true)
            .field(
                "Duración total",
                Self::format_duration(Some(total_duration)),
                true,
            )
            .field("Página", format!("{}/{}", page + 1, total_pages), true)
            .colour(Colour::from_rgb(114, 137, 218))
            .footer(CreateEmbedFooter::new("Usa los botones para navegar"));
        embed
    }

    /// Crea un embed de error
    #[allow(dead_code)]
    pub fn error_embed(error_msg: &str) -> CreateEmbed {
        CreateEmbed::default()
            .title("❌ Error")
            .description(error_msg)
            .colour(Colour::RED)
            .timestamp(Timestamp::now())
    }

    /// Crea un embed de éxito
    #[allow(dead_code)]
    pub fn success_embed(title: &str, description: &str) -> CreateEmbed {
        CreateEmbed::default()
            .title(format!("✅ {}", title))
            .description(description)
            .colour(Colour::from_rgb(67, 181, 129))
            .timestamp(Timestamp::now())
    }

    /// Formatea la duración en formato legible
    #[allow(dead_code)]
    fn format_duration(duration: Option<Duration>) -> String {
        match duration {
            Some(d) => {
                let total_seconds = d.as_secs();
                let hours = total_seconds / 3600;
                let minutes = (total_seconds % 3600) / 60;
                let seconds = total_seconds % 60;

                if hours > 0 {
                    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
                } else {
                    format!("{:02}:{:02}", minutes, seconds)
                }
            }
            None => "En vivo".to_string(),
        }
    }

    /// Crea una barra de progreso visual
    #[allow(dead_code)]
    fn create_progress_bar(progress: f32, length: usize) -> String {
        let filled = (progress * length as f32) as usize;
        let empty = length - filled;

        format!(
            "{}{}{}",
            "▬".repeat(filled),
            "🔘",
            "▬".repeat(empty.saturating_sub(1))
        )
    }
}

/// Manejador de interacciones con componentes
pub async fn handle_music_component(
    ctx: &Context,
    interaction: &ComponentInteraction,
    bot: &crate::bot::OpenMusicBot,
) -> Result<()> {
    let player = &bot.player;
    let guild_id = interaction
        .guild_id
        .ok_or_else(|| anyhow::anyhow!("No guild ID"))?;

    match interaction.data.custom_id.as_str() {
        button_ids::PLAY_PAUSE => {
            if player.is_playing(guild_id).await {
                player.pause(guild_id).await?;
                respond_with_updated_now_playing(ctx, interaction, guild_id, player, "⏸️ Música pausada").await?;
            } else {
                player.resume(guild_id).await?;
                respond_with_updated_now_playing(ctx, interaction, guild_id, player, "▶️ Música reanudada").await?;
            }
        }
        button_ids::SKIP => {
            interaction.defer(&ctx.http).await?;
            
            // Obtener el handler para reproducir la siguiente canción
            if let Some(handler) = bot.get_voice_handler(guild_id) {
                match player.skip_tracks(guild_id, 1, handler).await {
                    Ok(_) => {
                        update_response(ctx, interaction, "⏭️ Saltando a la siguiente canción").await?;
                    }
                    Err(_) => {
                        update_response(ctx, interaction, "⏭️ No hay más canciones en la cola").await?;
                    }
                }
            } else {
                update_response(ctx, interaction, "❌ No hay conexión de voz activa").await?;
            }
        }
        button_ids::STOP => {
            interaction.defer(&ctx.http).await?;
            player.stop(guild_id).await?;
            update_response(ctx, interaction, "⏹️ Reproducción detenida").await?;
        }
        button_ids::SHUFFLE => {
            interaction.defer(&ctx.http).await?;
            let enabled = player.toggle_shuffle(guild_id).await?;
            let msg = if enabled {
                "🔀 Modo aleatorio activado"
            } else {
                "🔀 Modo aleatorio desactivado"
            };
            update_response(ctx, interaction, msg).await?;
        }
        button_ids::LOOP_TRACK => {
            interaction.defer(&ctx.http).await?;
            let enabled = player.toggle_loop(guild_id).await?;
            let msg = if enabled {
                "🔁 Repetición activada"
            } else {
                "🔁 Repetición desactivada"
            };
            update_response(ctx, interaction, msg).await?;
        }
        button_ids::VOLUME_DOWN => {
            interaction.defer(&ctx.http).await?;
            let current_volume = player.get_volume(guild_id).await.unwrap_or(0.5);
            let new_volume = (current_volume - 0.1).max(0.0);
            
            if let Err(e) = player.set_volume(guild_id, new_volume).await {
                error!("Error ajustando volumen: {:?}", e);
                update_response(ctx, interaction, "❌ Error al ajustar el volumen").await?;
            } else {
                let volume_percent = (new_volume * 100.0) as u8;
                let msg = format!("🔉 Volumen: {}%", volume_percent);
                update_response(ctx, interaction, &msg).await?;
            }
        }
        button_ids::VOLUME_UP => {
            interaction.defer(&ctx.http).await?;
            let current_volume = player.get_volume(guild_id).await.unwrap_or(0.5);
            let new_volume = (current_volume + 0.1).min(2.0);
            
            if let Err(e) = player.set_volume(guild_id, new_volume).await {
                error!("Error ajustando volumen: {:?}", e);
                update_response(ctx, interaction, "❌ Error al ajustar el volumen").await?;
            } else {
                let volume_percent = (new_volume * 100.0) as u8;
                let msg = format!("🔊 Volumen: {}%", volume_percent);
                update_response(ctx, interaction, &msg).await?;
            }
        }
        button_ids::QUEUE => {
            match player.get_queue_info(guild_id).await {
                Ok(queue_info) => {
                    let embed = crate::ui::embeds::create_queue_embed(&queue_info, 1);
                    interaction.create_response(&ctx.http, 
                        serenity::builder::CreateInteractionResponse::Message(
                            serenity::builder::CreateInteractionResponseMessage::new()
                                .embed(embed)
                                .ephemeral(true)
                        )
                    ).await?;
                }
                Err(e) => {
                    error!("Error obteniendo información de cola: {:?}", e);
                    interaction.create_response(&ctx.http,
                        serenity::builder::CreateInteractionResponse::Message(
                            serenity::builder::CreateInteractionResponseMessage::new()
                                .content("❌ Error al obtener la cola")
                                .ephemeral(true)
                        )
                    ).await?;
                }
            }
        }
        button_ids::EFFECTS => {
            let eq_details = player.get_equalizer_details();
            
            let mut status = String::new();
            status.push_str("🎛️ **Estado del Ecualizador**\n\n");
            status.push_str(&format!("🎵 {}\n\n", eq_details));
            status.push_str("**Presets Disponibles:**\n");
            status.push_str("🎵 Bass - Enfatiza graves\n");
            status.push_str("🎤 Pop - Equilibrado moderno\n");
            status.push_str("🎸 Rock - Graves y agudos\n");
            status.push_str("🎺 Jazz - Claridad vocal\n");
            status.push_str("🎼 Classical - Dinámico natural\n");
            status.push_str("🔊 Electronic - Sintético\n");
            status.push_str("🗣️ Vocal - Enfatiza voces\n");
            status.push_str("📏 Flat - Sin modificaciones\n\n");
            status.push_str("💡 *Usa `/equalizer <preset>` para cambiar*");
            
            interaction.create_response(&ctx.http,
                serenity::builder::CreateInteractionResponse::Message(
                    serenity::builder::CreateInteractionResponseMessage::new()
                        .content(&status)
                        .ephemeral(true)
                )
            ).await?;
        }
        // Manejo de botones específicos de playlist
        button_ids::PLAYLIST_LOAD => {
            interaction.create_response(&ctx.http,
                serenity::builder::CreateInteractionResponse::Message(
                    serenity::builder::CreateInteractionResponseMessage::new()
                        .content("📥 **Cargar Playlist**\n\nUsa `/play <url_de_playlist>` para cargar una playlist de YouTube.\n\n📋 **Ejemplos:**\n• `https://youtube.com/playlist?list=...`\n• `https://music.youtube.com/playlist?list=...`")
                        .ephemeral(true)
                )
            ).await?;
        }
        button_ids::PLAYLIST_PREVIEW => {
            if let Ok(queue_info) = player.get_queue_info(guild_id).await {
                let total_tracks = queue_info.total_items;
                let preview_msg = format!(
                    "👁️ **Vista Previa de la Cola**\n\n📊 **Estadísticas:**\n• Total de canciones: {}\n• Duración total: {}\n• Modo loop: {:?}\n• Shuffle: {}\n\n💡 Usa `/queue` para ver la lista completa",
                    total_tracks,
                    if queue_info.total_duration.as_secs() > 0 {
                        format!("{} minutos", queue_info.total_duration.as_secs() / 60)
                    } else {
                        "Desconocida".to_string()
                    },
                    queue_info.loop_mode,
                    if queue_info.shuffle { "Activado" } else { "Desactivado" }
                );
                
                interaction.create_response(&ctx.http,
                    serenity::builder::CreateInteractionResponse::Message(
                        serenity::builder::CreateInteractionResponseMessage::new()
                            .content(&preview_msg)
                            .ephemeral(true)
                    )
                ).await?;
            } else {
                interaction.create_response(&ctx.http,
                    serenity::builder::CreateInteractionResponse::Message(
                        serenity::builder::CreateInteractionResponseMessage::new()
                            .content("❌ No se pudo obtener información de la cola")
                            .ephemeral(true)
                    )
                ).await?;
            }
        }
        button_ids::PLAYLIST_INFO => {
            let info_msg = "ℹ️ **Información de Playlists**\n\n📋 **Características:**\n• Soporte para playlists de YouTube\n• Carga automática hasta 50 canciones\n• Integración con cola de reproducción\n• Detección inteligente de URLs\n\n🎵 **Formatos soportados:**\n• `youtube.com/playlist?list=...`\n• `music.youtube.com/playlist?list=...`\n• URLs de video con parámetro `&list=`\n\n💡 **Consejos:**\n• Las playlists públicas funcionan mejor\n• Se respeta el orden original\n• Compatible con todos los controles del bot";
            
            interaction.create_response(&ctx.http,
                serenity::builder::CreateInteractionResponse::Message(
                    serenity::builder::CreateInteractionResponseMessage::new()
                        .content(info_msg)
                        .ephemeral(true)
                )
            ).await?;
        }
        _ => {
            debug!("Componente no manejado: {}", interaction.data.custom_id);
            interaction.create_response(&ctx.http,
                serenity::builder::CreateInteractionResponse::Message(
                    serenity::builder::CreateInteractionResponseMessage::new()
                        .content("⚠️ Función no implementada")
                        .ephemeral(true)
                )
            ).await?;
        }
    }

    Ok(())
}

/// Actualiza la respuesta de una interacción
async fn update_response(
    ctx: &Context,
    interaction: &ComponentInteraction,
    content: &str,
) -> Result<()> {
    use serenity::builder::CreateInteractionResponseFollowup;
    interaction
        .create_followup(
            &ctx.http,
            CreateInteractionResponseFollowup::new()
                .content(content)
                .ephemeral(true),
        )
        .await?;

    Ok(())
}

/// Responde a la interacción con el embed actualizado de "now playing"
async fn respond_with_updated_now_playing(
    ctx: &Context,
    interaction: &ComponentInteraction,
    guild_id: GuildId,
    player: &AudioPlayer,
    ephemeral_message: &str,
) -> Result<()> {
    if let Some(current_track) = player.get_current_track(guild_id).await {
        let embed = crate::ui::embeds::create_now_playing_embed_from_source(&current_track);
        let buttons = create_player_buttons();

        // Responder actualizando el mensaje original
        interaction.create_response(
            &ctx.http,
            serenity::builder::CreateInteractionResponse::UpdateMessage(
                serenity::builder::CreateInteractionResponseMessage::new()
                    .embed(embed)
                    .components(buttons)
            )
        ).await?;

        // También enviar un mensaje ephemeral de confirmación
        interaction.create_followup(
            &ctx.http,
            serenity::builder::CreateInteractionResponseFollowup::new()
                .content(ephemeral_message)
                .ephemeral(true),
        ).await?;
    } else {
        // Si no hay track actual, solo enviar mensaje ephemeral
        interaction.create_response(
            &ctx.http,
            serenity::builder::CreateInteractionResponse::Message(
                serenity::builder::CreateInteractionResponseMessage::new()
                    .content(ephemeral_message)
                    .ephemeral(true)
            )
        ).await?;
    }
    Ok(())
}
