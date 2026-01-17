//! Lavalink-specific command handlers
//! These are used when Lavalink server is available
#![allow(dead_code)]

use anyhow::Result;
use serenity::{
    all::{
        CommandInteraction, CreateInteractionResponse, CreateInteractionResponseMessage,
        CreateEmbed, Colour, ChannelId, GuildId,
    },
    prelude::Context,
};

use crate::audio::lavalink_simple::LavalinkManager;
use crate::ui::embeds::{create_success_embed, create_error_embed, create_info_embed};

pub async fn handle_lavalink_play(
    ctx: &Context,
    interaction: &CommandInteraction,
    query: &str,
) -> Result<()> {
    let guild_id = interaction.guild_id
        .ok_or_else(|| anyhow::anyhow!("Este comando solo funciona en servidores"))?;

    let channel_id = get_user_voice_channel(ctx, interaction, guild_id).await?;

    let lavalink = {
        let data_read = ctx.data.read().await;
        data_read.get::<LavalinkManager>()
            .ok_or_else(|| anyhow::anyhow!("Lavalink no está disponible"))?
            .clone()
    };

    interaction.create_response(&ctx.http, CreateInteractionResponse::Defer(
        CreateInteractionResponseMessage::new()
    )).await?;

    lavalink.join_channel(guild_id, channel_id).await?;

    let tracks = lavalink.search(query).await?;
    
    if tracks.is_empty() {
        let embed = create_error_embed("❌ No encontrado", 
            &format!("No se encontraron resultados para: `{}`", query));
        
        interaction.edit_response(&ctx.http, 
            serenity::builder::EditInteractionResponse::new().embed(embed)
        ).await?;
        return Ok(());
    }

    let first_track = tracks[0].clone();
    lavalink.play(guild_id, &first_track).await?;

    let embed = create_success_embed("🎵 Reproduciendo",
        &format!("**{}**\n\n▶️ Reproduciendo ahora (Lavalink)", first_track.info.title)
    );
    
    interaction.edit_response(&ctx.http, 
        serenity::builder::EditInteractionResponse::new().embed(embed)
    ).await?;

    Ok(())
}

pub async fn handle_lavalink_pause(
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<()> {
    let guild_id = interaction.guild_id
        .ok_or_else(|| anyhow::anyhow!("Este comando solo funciona en servidores"))?;

    let lavalink = {
        let data_read = ctx.data.read().await;
        data_read.get::<LavalinkManager>()
            .ok_or_else(|| anyhow::anyhow!("Lavalink no está disponible"))?
            .clone()
    };

    lavalink.pause(guild_id).await?;
    
    let embed = create_success_embed("⏸️ Pausado", 
        "Reproducción pausada (Lavalink).");
    interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().embed(embed)
    )).await?;

    Ok(())
}

pub async fn handle_lavalink_resume(
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<()> {
    let guild_id = interaction.guild_id
        .ok_or_else(|| anyhow::anyhow!("Este comando solo funciona en servidores"))?;

    let lavalink = {
        let data_read = ctx.data.read().await;
        data_read.get::<LavalinkManager>()
            .ok_or_else(|| anyhow::anyhow!("Lavalink no está disponible"))?
            .clone()
    };

    lavalink.resume(guild_id).await?;
    
    let embed = create_success_embed("▶️ Reanudado", 
        "Reproducción reanudada (Lavalink).");
    interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().embed(embed)
    )).await?;

    Ok(())
}

pub async fn handle_lavalink_skip(
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<()> {
    let guild_id = interaction.guild_id
        .ok_or_else(|| anyhow::anyhow!("Este comando solo funciona en servidores"))?;

    let lavalink = {
        let data_read = ctx.data.read().await;
        data_read.get::<LavalinkManager>()
            .ok_or_else(|| anyhow::anyhow!("Lavalink no está disponible"))?
            .clone()
    };

    match lavalink.skip(guild_id).await? {
        Some(next_track) => {
            let embed = create_success_embed("⏭️ Saltado", 
                &format!("Ahora reproduciendo: **{}** (Lavalink)", next_track)
            );
            interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            )).await?;
        }
        None => {
            let embed = create_info_embed("⏭️ Saltado", 
                "No hay más canciones en la cola (Lavalink).");
            interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            )).await?;
        }
    }

    Ok(())
}

pub async fn handle_lavalink_stop(
    ctx: &Context,
    interaction: &CommandInteraction,
) -> Result<()> {
    let guild_id = interaction.guild_id
        .ok_or_else(|| anyhow::anyhow!("Este comando solo funciona en servidores"))?;

    let lavalink = {
        let data_read = ctx.data.read().await;
        data_read.get::<LavalinkManager>()
            .ok_or_else(|| anyhow::anyhow!("Lavalink no está disponible"))?
            .clone()
    };

    lavalink.stop(guild_id).await?;
    lavalink.clear_queue(guild_id).await?;
    
    let embed = create_success_embed("⏹️ Detenido", 
        "Reproducción detenida y cola limpiada (Lavalink).");
    interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().embed(embed)
    )).await?;

    Ok(())
}

pub async fn handle_lavalink_volume(
    ctx: &Context,
    interaction: &CommandInteraction,
    volume: i64,
) -> Result<()> {
    let guild_id = interaction.guild_id
        .ok_or_else(|| anyhow::anyhow!("Este comando solo funciona en servidores"))?;

    let volume = volume.clamp(0, 200) as i32;

    let lavalink = {
        let data_read = ctx.data.read().await;
        data_read.get::<LavalinkManager>()
            .ok_or_else(|| anyhow::anyhow!("Lavalink no está disponible"))?
            .clone()
    };

    lavalink.set_volume(guild_id, volume).await?;
    
    let embed = create_success_embed("🔊 Volumen ajustado", 
        &format!("Volumen establecido a **{}%** (Lavalink)", volume));
    interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().embed(embed)
    )).await?;

    Ok(())
}

pub async fn handle_lavalink_queue(
    ctx: &Context,
    interaction: &CommandInteraction,
    page: Option<i64>,
) -> Result<()> {
    let guild_id = interaction.guild_id
        .ok_or_else(|| anyhow::anyhow!("Este comando solo funciona en servidores"))?;

    let lavalink = {
        let data_read = ctx.data.read().await;
        data_read.get::<LavalinkManager>()
            .ok_or_else(|| anyhow::anyhow!("Lavalink no está disponible"))?
            .clone()
    };

    let queue = lavalink.get_queue(guild_id).await;
    
    if queue.is_empty() {
        let embed = create_info_embed("📋 Cola vacía", 
            "No hay canciones en la cola (Lavalink).");
        interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed)
        )).await?;
        return Ok(());
    }

    let page = page.unwrap_or(1).max(1) as usize;
    let per_page = 10;
    let start_idx = (page - 1) * per_page;

    let mut description = String::new();
    description.push_str("**📋 Cola de reproducción (Lavalink):**\n");
    
    let queue_slice = queue.iter()
        .skip(start_idx)
        .take(per_page)
        .enumerate();

    for (i, track) in queue_slice {
        let position = start_idx + i + 1;
        description.push_str(&format!("{}. {}\n", position, track));
    }

    description.push_str(&format!("\n**Total: {} canciones**", queue.len()));

    let embed = CreateEmbed::new()
        .title("🎵 Cola de reproducción")
        .description(description)
        .color(Colour::BLURPLE);

    interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().embed(embed)
    )).await?;

    Ok(())
}

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