use anyhow::Result;
use serenity::{
    all::{
        CommandInteraction, CreateInteractionResponse, CreateInteractionResponseMessage,
        CreateEmbed, Colour, ChannelId, GuildId,
    },
    prelude::Context,
};
use tracing::{error, info};

use crate::audio::hybrid_manager::HybridAudioManager;
use crate::ui::embeds::{create_success_embed, create_error_embed, create_info_embed};

pub async fn handle_hybrid_play(
    ctx: &Context,
    interaction: &CommandInteraction,
    query: &str,
) -> Result<()> {
    let guild_id = interaction.guild_id
        .ok_or_else(|| anyhow::anyhow!("Este comando solo funciona en servidores"))?;

    // Obtener canal de voz del usuario
    let channel_id = get_user_voice_channel(ctx, interaction, guild_id).await?;

    // Obtener el hybrid manager
    let hybrid_manager = {
        let data_read = ctx.data.read().await;
        data_read.get::<HybridAudioManager>()
            .ok_or_else(|| anyhow::anyhow!("Sistema de audio no está disponible"))?
            .clone()
    };

    // Respuesta inicial (diferida porque puede tomar tiempo)
    interaction.create_response(&ctx.http, CreateInteractionResponse::Defer(
        CreateInteractionResponseMessage::new()
    )).await?;

    // Conectar al canal si no está conectado
    if !hybrid_manager.is_connected(guild_id).await {
        if let Err(e) = hybrid_manager.join_channel(guild_id, channel_id, interaction.user.id).await {
            error!("Error al conectar al canal de voz: {:?}", e);
            let embed = create_error_embed("❌ Error de conexión", 
                "No pude conectarme al canal de voz. Verifica que tengo permisos.");
            
            interaction.edit_response(&ctx.http, 
                serenity::builder::EditInteractionResponse::new().embed(embed)
            ).await?;
            return Ok(());
        }
    }

    // Reproducir la canción
    match hybrid_manager.play(guild_id, query, interaction.user.id).await {
        Ok(track_info) => {
            let duration_text = if let Some(duration) = track_info.duration() {
                format!(" ({})", format_duration(duration))
            } else {
                String::new()
            };

            let embed = create_success_embed("🎵 Reproduciendo",
                &format!("**{}**{}\n\n▶️ Conectado al <#{}>",
                    track_info.title(),
                    duration_text,
                    channel_id.get()
                )
            );
            
            interaction.edit_response(&ctx.http, 
                serenity::builder::EditInteractionResponse::new().embed(embed)
            ).await?;

            info!("✅ Reproduciendo '{}' para usuario {} en guild {}", 
                  track_info.title(), interaction.user.name, guild_id);
        }
        Err(e) => {
            error!("Error al reproducir canción: {:?}", e);
            let embed = create_error_embed("❌ Error de reproducción", 
                &format!("No pude reproducir la canción: {}", e));
            
            interaction.edit_response(&ctx.http, 
                serenity::builder::EditInteractionResponse::new().embed(embed)
            ).await?;
        }
    }

    Ok(())
}

pub async fn handle_hybrid_pause(
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<()> {
    let guild_id = interaction.guild_id
        .ok_or_else(|| anyhow::anyhow!("Este comando solo funciona en servidores"))?;

    let hybrid_manager = {
        let data_read = ctx.data.read().await;
        data_read.get::<HybridAudioManager>()
            .ok_or_else(|| anyhow::anyhow!("Sistema de audio no está disponible"))?
            .clone()
    };

    match hybrid_manager.pause(guild_id).await {
        Ok(()) => {
            let embed = create_success_embed("⏸️ Pausado", 
                "Reproducción pausada.");
            interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            )).await?;
        }
        Err(e) => {
            let embed = create_error_embed("❌ Error", &format!("No pude pausar: {}", e));
            interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            )).await?;
        }
    }

    Ok(())
}

pub async fn handle_hybrid_resume(
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<()> {
    let guild_id = interaction.guild_id
        .ok_or_else(|| anyhow::anyhow!("Este comando solo funciona en servidores"))?;

    let hybrid_manager = {
        let data_read = ctx.data.read().await;
        data_read.get::<HybridAudioManager>()
            .ok_or_else(|| anyhow::anyhow!("Sistema de audio no está disponible"))?
            .clone()
    };

    match hybrid_manager.resume(guild_id).await {
        Ok(()) => {
            let embed = create_success_embed("▶️ Reanudado", 
                "Reproducción reanudada.");
            interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            )).await?;
        }
        Err(e) => {
            let embed = create_error_embed("❌ Error", &format!("No pude reanudar: {}", e));
            interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            )).await?;
        }
    }

    Ok(())
}

pub async fn handle_hybrid_skip(
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<()> {
    let guild_id = interaction.guild_id
        .ok_or_else(|| anyhow::anyhow!("Este comando solo funciona en servidores"))?;

    let hybrid_manager = {
        let data_read = ctx.data.read().await;
        data_read.get::<HybridAudioManager>()
            .ok_or_else(|| anyhow::anyhow!("Sistema de audio no está disponible"))?
            .clone()
    };

    match hybrid_manager.skip(guild_id).await {
        Ok(()) => {
            let embed = create_success_embed("⏭️ Saltado", 
                "Canción saltada.");
            interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            )).await?;
        }
        Err(e) => {
            let embed = create_error_embed("❌ Error", &format!("No pude saltar: {}", e));
            interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            )).await?;
        }
    }

    Ok(())
}

pub async fn handle_hybrid_stop(
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<()> {
    let guild_id = interaction.guild_id
        .ok_or_else(|| anyhow::anyhow!("Este comando solo funciona en servidores"))?;

    let hybrid_manager = {
        let data_read = ctx.data.read().await;
        data_read.get::<HybridAudioManager>()
            .ok_or_else(|| anyhow::anyhow!("Sistema de audio no está disponible"))?
            .clone()
    };

    match hybrid_manager.stop(guild_id).await {
        Ok(()) => {
            let embed = create_success_embed("⏹️ Detenido", 
                "Reproducción detenida.");
            interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            )).await?;
        }
        Err(e) => {
            let embed = create_error_embed("❌ Error", &format!("No pude detener: {}", e));
            interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            )).await?;
        }
    }

    Ok(())
}

pub async fn handle_hybrid_leave(
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<()> {
    let guild_id = interaction.guild_id
        .ok_or_else(|| anyhow::anyhow!("Este comando solo funciona en servidores"))?;

    let hybrid_manager = {
        let data_read = ctx.data.read().await;
        data_read.get::<HybridAudioManager>()
            .ok_or_else(|| anyhow::anyhow!("Sistema de audio no está disponible"))?
            .clone()
    };

    match hybrid_manager.leave_channel(guild_id).await {
        Ok(()) => {
            let embed = create_success_embed("👋 Desconectado", 
                "Me he desconectado del canal de voz.");
            interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            )).await?;
        }
        Err(e) => {
            let embed = create_error_embed("❌ Error", &format!("Error al desconectar: {}", e));
            interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            )).await?;
        }
    }

    Ok(())
}

pub async fn handle_hybrid_nowplaying(
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<()> {
    let guild_id = interaction.guild_id
        .ok_or_else(|| anyhow::anyhow!("Este comando solo funciona en servidores"))?;

    let hybrid_manager = {
        let data_read = ctx.data.read().await;
        data_read.get::<HybridAudioManager>()
            .ok_or_else(|| anyhow::anyhow!("Sistema de audio no está disponible"))?
            .clone()
    };

    match hybrid_manager.now_playing(guild_id).await {
        Ok(Some(info)) => {
            let mut description = format!("**{}**", info.title);
            
            if let Some(artist) = info.artist {
                description.push_str(&format!("\nPor: {}", artist));
            }
            
            let position_text = format_duration(info.position);
            if let Some(duration) = info.duration {
                let duration_text = format_duration(duration);
                description.push_str(&format!("\n\n⏱️ {} / {}", position_text, duration_text));
            } else {
                description.push_str(&format!("\n\n⏱️ {}", position_text));
            }
            
            description.push_str(&format!("\n👤 Solicitado por: <@{}>", info.requester));

            let embed = CreateEmbed::new()
                .title("🎵 Reproduciendo ahora")
                .description(description)
                .color(Colour::BLURPLE);

            interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            )).await?;
        }
        Ok(None) => {
            let embed = create_info_embed("🔇 Silencio", 
                "No hay ninguna canción reproduciéndose actualmente.");
            interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            )).await?;
        }
        Err(e) => {
            let embed = create_error_embed("❌ Error", &format!("Error obteniendo información: {}", e));
            interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            )).await?;
        }
    }

    Ok(())
}

// Función auxiliar para obtener el canal de voz del usuario
async fn get_user_voice_channel(
    ctx: &Context,
    interaction: &CommandInteraction,
    guild_id: GuildId,
) -> Result<ChannelId> {
    let guild = guild_id.to_guild_cached(&ctx.cache)
        .ok_or_else(|| anyhow::anyhow!("No se pudo encontrar el servidor"))?;

    let voice_state = guild.voice_states.get(&interaction.user.id)
        .ok_or_else(|| anyhow::anyhow!("Debes estar en un canal de voz para usar este comando"))?;

    voice_state.channel_id
        .ok_or_else(|| anyhow::anyhow!("No se pudo detectar tu canal de voz"))
}

// Función auxiliar para formatear duración
fn format_duration(duration: std::time::Duration) -> String {
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