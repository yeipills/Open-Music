use anyhow::Result;
use serenity::{
    all::{ButtonStyle, Colour, ComponentInteraction, Context, Timestamp},
    builder::{CreateActionRow, CreateButton, CreateEmbed, CreateEmbedFooter},
};
use std::time::Duration;
use tracing::debug;

use crate::{audio::player::AudioPlayer, sources::TrackSource};

/// IDs personalizados para los botones
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
}

/// Constructor de controles de música
pub struct MusicControls;

impl MusicControls {
    /// Crea los controles principales del reproductor
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
}

/// Función de utilidad para crear controles básicos del reproductor
pub fn create_player_buttons() -> Vec<CreateActionRow> {
    MusicControls::create_player_controls(false, false, false)
}

/// Constructor de embeds para el reproductor
pub struct MusicEmbeds;

impl MusicEmbeds {
    /// Crea un embed para la canción actual
    pub fn now_playing(track: &TrackSource, is_playing: bool) -> CreateEmbed {
        let status = if is_playing {
            "▶️ Reproduciendo"
        } else {
            "⏸️ Pausado"
        };
        let progress_bar = Self::create_progress_bar(0.3, 20); // TODO: Calcular progreso real

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
    pub fn error_embed(error_msg: &str) -> CreateEmbed {
        CreateEmbed::default()
            .title("❌ Error")
            .description(error_msg)
            .colour(Colour::RED)
            .timestamp(Timestamp::now())
    }

    /// Crea un embed de éxito
    pub fn success_embed(title: &str, description: &str) -> CreateEmbed {
        CreateEmbed::default()
            .title(format!("✅ {}", title))
            .description(description)
            .colour(Colour::from_rgb(67, 181, 129))
            .timestamp(Timestamp::now())
    }

    /// Formatea la duración en formato legible
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
    player: &AudioPlayer,
) -> Result<()> {
    let guild_id = interaction
        .guild_id
        .ok_or_else(|| anyhow::anyhow!("No guild ID"))?;

    // Defer la respuesta para evitar timeout
    interaction.defer(&ctx.http).await?;

    match interaction.data.custom_id.as_str() {
        button_ids::PLAY_PAUSE => {
            if player.is_playing(guild_id).await {
                player.pause(guild_id).await?;
                update_response(ctx, interaction, "⏸️ Música pausada").await?;
            } else {
                player.resume(guild_id).await?;
                update_response(ctx, interaction, "▶️ Música reanudada").await?;
            }
        }
        button_ids::SKIP => {
            // TODO: Implementar skip con handler
            update_response(ctx, interaction, "⏭️ Saltando a la siguiente canción...").await?;
        }
        button_ids::STOP => {
            player.stop(guild_id).await?;
            update_response(ctx, interaction, "⏹️ Reproducción detenida").await?;
        }
        button_ids::SHUFFLE => {
            let enabled = player.toggle_shuffle(guild_id).await?;
            let msg = if enabled {
                "🔀 Modo aleatorio activado"
            } else {
                "🔀 Modo aleatorio desactivado"
            };
            update_response(ctx, interaction, msg).await?;
        }
        button_ids::LOOP_TRACK => {
            let enabled = player.toggle_loop(guild_id).await?;
            let msg = if enabled {
                "🔁 Repetición activada"
            } else {
                "🔁 Repetición desactivada"
            };
            update_response(ctx, interaction, msg).await?;
        }
        button_ids::VOLUME_DOWN => {
            // TODO: Implementar control de volumen
            update_response(ctx, interaction, "🔉 Volumen disminuido").await?;
        }
        button_ids::VOLUME_UP => {
            // TODO: Implementar control de volumen
            update_response(ctx, interaction, "🔊 Volumen aumentado").await?;
        }
        button_ids::QUEUE => {
            // TODO: Mostrar cola
            update_response(ctx, interaction, "📋 Mostrando cola...").await?;
        }
        button_ids::EFFECTS => {
            // TODO: Mostrar menú de efectos
            update_response(ctx, interaction, "🎛️ Menú de efectos...").await?;
        }
        _ => {
            debug!("Componente no manejado: {}", interaction.data.custom_id);
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
