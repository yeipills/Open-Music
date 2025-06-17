use anyhow::Result;
use serenity::{
    builder::{CreateCommand, CreateCommandOption},
    model::{application::CommandOptionType, id::GuildId},
    prelude::Context,
};

/// Registra comandos globales
pub async fn register_global_commands(ctx: &Context) -> Result<()> {
    let commands = vec![
        play_command(),
        search_command(),
        playlist_command(),
        pause_command(),
        resume_command(),
        skip_command(),
        stop_command(),
        queue_command(),
        nowplaying_command(),
        shuffle_command(),
        loop_command(),
        clear_command(),
        volume_command(),
        equalizer_command(),
        effect_command(),
        join_command(),
        leave_command(),
        lyrics_command(),
        help_command(),
    ];

    for command in commands {
        ctx.http.create_global_command(&command).await?;
    }

    Ok(())
}

/// Registra comandos para una guild específica (desarrollo)
pub async fn register_guild_commands(ctx: &Context, guild_id: GuildId) -> Result<()> {
    let commands = vec![
        play_command(),
        search_command(),
        playlist_command(),
        pause_command(),
        resume_command(),
        skip_command(),
        stop_command(),
        queue_command(),
        nowplaying_command(),
        shuffle_command(),
        loop_command(),
        clear_command(),
        volume_command(),
        equalizer_command(),
        effect_command(),
        join_command(),
        leave_command(),
        lyrics_command(),
        help_command(),
    ];

    guild_id.set_commands(&ctx.http, commands).await?;

    Ok(())
}

// Comandos de reproducción

fn play_command() -> CreateCommand {
    CreateCommand::new("play")
        .description("Reproduce una canción o playlist")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "query",
                "URL o término de búsqueda",
            )
            .required(true),
        )
}

fn search_command() -> CreateCommand {
    CreateCommand::new("search")
        .description("Busca canciones y muestra resultados")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "query", "Término de búsqueda")
                .required(true),
        )
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "source", "Fuente de búsqueda")
                .add_string_choice("YouTube", "youtube")
                .add_string_choice("Spotify", "spotify")
                .add_string_choice("SoundCloud", "soundcloud")
                .add_string_choice("Tidal", "tidal"),
        )
}

fn playlist_command() -> CreateCommand {
    CreateCommand::new("playlist")
        .description("Carga una playlist completa")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "url", "URL de la playlist")
                .required(true),
        )
        .add_option(CreateCommandOption::new(
            CommandOptionType::Boolean,
            "shuffle",
            "Mezclar la playlist al cargar",
        ))
}

// Comandos de control

fn pause_command() -> CreateCommand {
    CreateCommand::new("pause").description("Pausa la reproducción actual")
}

fn resume_command() -> CreateCommand {
    CreateCommand::new("resume").description("Reanuda la reproducción pausada")
}

fn skip_command() -> CreateCommand {
    CreateCommand::new("skip")
        .description("Salta a la siguiente canción")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                "amount",
                "Número de canciones a saltar",
            )
            .min_int_value(1)
            .max_int_value(100),
        )
}

fn stop_command() -> CreateCommand {
    CreateCommand::new("stop").description("Detiene la reproducción y limpia la cola")
}

// Comandos de cola

fn queue_command() -> CreateCommand {
    CreateCommand::new("queue")
        .description("Muestra la cola de reproducción")
        .add_option(
            CreateCommandOption::new(CommandOptionType::Integer, "page", "Número de página")
                .min_int_value(1),
        )
}

fn nowplaying_command() -> CreateCommand {
    CreateCommand::new("nowplaying").description("Muestra información de la canción actual")
}

fn shuffle_command() -> CreateCommand {
    CreateCommand::new("shuffle").description("Activa/desactiva el modo aleatorio")
}

fn loop_command() -> CreateCommand {
    CreateCommand::new("loop")
        .description("Configura el modo de repetición")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "mode", "Modo de repetición")
                .add_string_choice("Desactivar", "off")
                .add_string_choice("Canción", "track")
                .add_string_choice("Cola", "queue")
                .required(true),
        )
}

fn clear_command() -> CreateCommand {
    CreateCommand::new("clear")
        .description("Limpia la cola de reproducción")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "filter", "Filtro para limpiar")
                .add_string_choice("Todo", "all")
                .add_string_choice("Duplicados", "duplicates")
                .add_string_choice("Usuario", "user"),
        )
        .add_option(CreateCommandOption::new(
            CommandOptionType::User,
            "user",
            "Usuario específico (requiere filtro 'user')",
        ))
}

// Comandos de audio

fn volume_command() -> CreateCommand {
    CreateCommand::new("volume")
        .description("Ajusta el volumen de reproducción")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::Integer,
                "level",
                "Nivel de volumen (0-200)",
            )
            .min_int_value(0)
            .max_int_value(200),
        )
}

fn equalizer_command() -> CreateCommand {
    CreateCommand::new("equalizer")
        .description("Configura el ecualizador")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "preset", "Preset de ecualizador")
                .add_string_choice("Normal", "normal")
                .add_string_choice("Bass", "bass")
                .add_string_choice("Pop", "pop")
                .add_string_choice("Rock", "rock")
                .add_string_choice("Jazz", "jazz")
                .add_string_choice("Classical", "classical")
                .add_string_choice("Electronic", "electronic")
                .add_string_choice("Vocal", "vocal")
                .add_string_choice("Custom", "custom"),
        )
        .add_option(CreateCommandOption::new(
            CommandOptionType::String,
            "bands",
            "Valores personalizados (ej: 32:2 64:1 125:0)",
        ))
}

fn effect_command() -> CreateCommand {
    CreateCommand::new("effect")
        .description("Activa/desactiva efectos de audio")
        .add_option(
            CreateCommandOption::new(CommandOptionType::String, "type", "Tipo de efecto")
                .add_string_choice("Bass Boost", "bassboost")
                .add_string_choice("8D Audio", "8d")
                .add_string_choice("Nightcore", "nightcore")
                .add_string_choice("Vaporwave", "vaporwave")
                .add_string_choice("Tremolo", "tremolo")
                .add_string_choice("Karaoke", "karaoke")
                .add_string_choice("Ninguno", "none")
                .required(true),
        )
}

// Comandos de conexión

fn join_command() -> CreateCommand {
    CreateCommand::new("join").description("Conecta el bot a tu canal de voz")
}

fn leave_command() -> CreateCommand {
    CreateCommand::new("leave").description("Desconecta el bot del canal de voz")
}

// Comandos adicionales

fn lyrics_command() -> CreateCommand {
    CreateCommand::new("lyrics")
        .description("Busca la letra de una canción")
        .add_option(CreateCommandOption::new(
            CommandOptionType::String,
            "query",
            "Nombre de la canción o artista",
        ))
}

fn help_command() -> CreateCommand {
    CreateCommand::new("help")
        .description("Muestra información de ayuda")
        .add_option(CreateCommandOption::new(
            CommandOptionType::String,
            "command",
            "Comando específico",
        ))
}
