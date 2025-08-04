use anyhow::Result;
use serenity::{
    all::{
        CommandInteraction, CreateInteractionResponse, CreateInteractionResponseMessage,
        CreateEmbed, Colour, ChannelId, GuildId,
    },
    prelude::Context,
};
use tracing::{error, info, warn};

use crate::audio::lavalink_client::LavalinkManager;
use crate::ui::embeds::{create_success_embed, create_error_embed, create_info_embed};

pub async fn handle_lavalink_play(
    ctx: &Context,
    interaction: &CommandInteraction,
    query: &str,
) -> Result<()> {
    // Obtener guild_id
    let guild_id = interaction.guild_id
        .ok_or_else(|| anyhow::anyhow!("Este comando solo funciona en servidores"))?;

    // Obtener canal de voz del usuario
    let channel_id = get_user_voice_channel(ctx, interaction, guild_id).await?;

    // Obtener cliente Lavalink
    let lavalink = {
        let data_read = ctx.data.read().await;
        data_read.get::<LavalinkManager>()
            .ok_or_else(|| anyhow::anyhow!("Lavalink no está disponible"))?
            .clone()
    };

    // Respuesta inicial
    interaction.create_response(&ctx.http, CreateInteractionResponse::Defer(
        CreateInteractionResponseMessage::new()
    )).await?;

    // Unirse al canal si no está conectado
    if let Err(e) = lavalink.join_channel(guild_id, channel_id).await {
        warn!("No se pudo unir al canal: {:?}", e);
    }

    // Buscar tracks
    let tracks = match lavalink.search(query).await {
        Ok(tracks) if !tracks.is_empty() => tracks,
        Ok(_) => {
            let embed = create_error_embed("❌ No encontrado", 
                &format!("No se encontraron resultados para: `{}`", query));
            
            interaction.edit_response(&ctx.http, 
                serenity::builder::EditInteractionResponse::new().embed(embed)
            ).await?;
            return Ok(());
        }
        Err(e) => {
            error!("Error al buscar: {:?}", e);
            let embed = create_error_embed("❌ Error de búsqueda", 
                "Ocurrió un error al buscar la canción. Inténtalo de nuevo.");
            
            interaction.edit_response(&ctx.http, 
                serenity::builder::EditInteractionResponse::new().embed(embed)
            ).await?;
            return Ok(());
        }
    };

    let first_track = tracks[0].clone();
    let remaining_tracks = if tracks.len() > 1 {
        tracks[1..].to_vec()
    } else {
        Vec::new()
    };

    // Obtener estado actual del player
    let player_state = lavalink.get_player_state(guild_id).await;

    if player_state.current_track.is_none() {
        // No hay nada reproduciéndose, reproducir inmediatamente
        if let Err(e) = lavalink.play(guild_id, first_track.clone()).await {
            error!("Error al reproducir: {:?}", e);
            let embed = create_error_embed("❌ Error de reproducción", 
                "No se pudo reproducir la canción.");
            
            interaction.edit_response(&ctx.http, 
                serenity::builder::EditInteractionResponse::new().embed(embed)
            ).await?;
            return Ok(());
        }

        // Agregar canciones restantes a la cola si es una playlist
        if !remaining_tracks.is_empty() {
            let added_count = lavalink.add_to_queue(guild_id, remaining_tracks).await?;
            
            let embed = create_success_embed("🎵 Reproduciendo",
                &format!("**{}**\n\n▶️ Reproduciendo ahora\n📋 {} canciones agregadas a la cola",
                    first_track.info.title.unwrap_or_else(|| "Título desconocido".to_string()),
                    added_count
                )
            );
            
            interaction.edit_response(&ctx.http, 
                serenity::builder::EditInteractionResponse::new().embed(embed)
            ).await?;
        } else {
            let embed = create_success_embed("🎵 Reproduciendo",
                &format!("**{}**\n\n▶️ Reproduciendo ahora",
                    first_track.info.title.unwrap_or_else(|| "Título desconocido".to_string())
                )
            );
            
            interaction.edit_response(&ctx.http, 
                serenity::builder::EditInteractionResponse::new().embed(embed)
            ).await?;
        }
    } else {
        // Ya hay algo reproduciéndose, agregar a la cola
        let added_count = lavalink.add_to_queue(guild_id, tracks).await?;
        
        let embed = create_success_embed("📋 Agregado a la cola",
            &format!("**{}**\n\n📋 {} canción(es) agregada(s) a la cola",
                first_track.info.title.unwrap_or_else(|| "Título desconocido".to_string()),
                added_count
            )
        );
        
        interaction.edit_response(&ctx.http, 
            serenity::builder::EditInteractionResponse::new().embed(embed)
        ).await?;
    }

    info!("Comando play ejecutado exitosamente para guild {}", guild_id);
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

    let player_state = lavalink.get_player_state(guild_id).await;
    
    if player_state.current_track.is_none() {
        let embed = create_error_embed("❌ Nada reproduciéndose", 
            "No hay ninguna canción reproduciéndose actualmente.");
        interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed)
        )).await?;
        return Ok(());
    }

    if player_state.is_paused {
        let embed = create_info_embed("⏸️ Ya pausado", 
            "La reproducción ya está pausada.");
        interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed)
        )).await?;
        return Ok(());
    }

    lavalink.pause(guild_id).await?;
    
    let embed = create_success_embed("⏸️ Pausado", 
        "Reproducción pausada.");
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

    let player_state = lavalink.get_player_state(guild_id).await;
    
    if player_state.current_track.is_none() {
        let embed = create_error_embed("❌ Nada reproduciéndose", 
            "No hay ninguna canción para reanudar.");
        interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed)
        )).await?;
        return Ok(());
    }

    if !player_state.is_paused {
        let embed = create_info_embed("▶️ Ya reproduciéndose", 
            "La canción ya se está reproduciendo.");
        interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed)
        )).await?;
        return Ok(());
    }

    lavalink.resume(guild_id).await?;
    
    let embed = create_success_embed("▶️ Reanudado", 
        "Reproducción reanudada.");
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

    let player_state = lavalink.get_player_state(guild_id).await;
    
    if player_state.current_track.is_none() {
        let embed = create_error_embed("❌ Nada reproduciéndose", 
            "No hay ninguna canción para saltar.");
        interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed)
        )).await?;
        return Ok(());
    }

    match lavalink.skip(guild_id).await? {
        Some(next_track) => {
            let embed = create_success_embed("⏭️ Saltado", 
                &format!("Ahora reproduciendo: **{}**",
                    next_track.info.title.unwrap_or_else(|| "Título desconocido".to_string())
                )
            );
            interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            )).await?;
        }
        None => {
            let embed = create_info_embed("⏭️ Saltado", 
                "No hay más canciones en la cola.");
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
        "Reproducción detenida y cola limpiada.");
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
        &format!("Volumen establecido a **{}%**", volume));
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

    let player_state = lavalink.get_player_state(guild_id).await;
    
    if player_state.current_track.is_none() && player_state.queue.is_empty() {
        let embed = create_info_embed("📋 Cola vacía", 
            "No hay canciones en la cola.");
        interaction.create_response(&ctx.http, CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed)
        )).await?;
        return Ok(());
    }

    let page = page.unwrap_or(1).max(1) as usize;
    let per_page = 10;
    let start_idx = (page - 1) * per_page;
    let total_pages = (player_state.queue.len() + per_page - 1) / per_page;

    let mut description = String::new();
    
    // Canción actual
    if let Some(current) = &player_state.current_track {
        description.push_str(&format!("**🎵 Reproduciendo ahora:**\n{}\n\n", 
            current.info.title.as_ref().unwrap_or(&"Título desconocido".to_string())));
    }

    // Cola
    if !player_state.queue.is_empty() {
        description.push_str("**📋 En cola:**\n");
        
        let queue_slice = player_state.queue.iter()
            .skip(start_idx)
            .take(per_page)
            .enumerate();

        for (i, track) in queue_slice {
            let position = start_idx + i + 1;
            let title = track.info.title.as_ref().unwrap_or(&"Título desconocido".to_string());
            description.push_str(&format!("{}. {}\n", position, title));
        }

        if total_pages > 1 {
            description.push_str(&format!("\n**Página {} de {}**", page, total_pages));
        }
        
        description.push_str(&format!("\n**Total: {} canciones**", player_state.queue.len()));
    }

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