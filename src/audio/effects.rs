use dashmap::DashMap;
use serenity::model::id::GuildId;
use tracing::info;

/// Presets de ecualizador disponibles
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EqualizerPreset {
    Flat,
    Bass,
    Pop,
    Rock,
    Jazz,
    Classical,
    Electronic,
    Vocal,
}

/// Sistema de ecualizador con presets, **por servidor (guild)**.
///
/// Antes había un único preset global compartido por todas las guilds: cambiarlo
/// en un servidor afectaba a todos. Ahora cada guild tiene el suyo.
pub struct AudioEffects {
    presets: DashMap<GuildId, EqualizerPreset>,
}

impl AudioEffects {
    pub fn new() -> Self {
        info!("🎛️ Sistema de ecualizador inicializado");
        Self {
            presets: DashMap::new(),
        }
    }

    /// Construye la cadena de filtros ffmpeg (`-af`) para el preset de la guild.
    ///
    /// Siempre incluye `loudnorm` (EBU R128) para igualar el volumen percibido entre
    /// temas, y añade bandas `equalizer` según el preset. Para `Flat` solo normaliza.
    pub fn build_filter(&self, guild_id: GuildId) -> String {
        // loudnorm de una sola pasada: consistente y apto para streaming.
        let loudnorm = "loudnorm=I=-16:TP=-1.5:LRA=11";

        let preset = self.get_current_preset(guild_id);
        let eq = match preset {
            EqualizerPreset::Flat => "",
            // f=frecuencia(Hz), t=o (ancho en octavas), w=ancho, g=ganancia(dB)
            EqualizerPreset::Bass =>
                "equalizer=f=60:t=o:w=2:g=6,equalizer=f=120:t=o:w=2:g=3",
            EqualizerPreset::Pop =>
                "equalizer=f=100:t=o:w=2:g=2,equalizer=f=3000:t=o:w=2:g=3,equalizer=f=8000:t=o:w=2:g=2",
            EqualizerPreset::Rock =>
                "equalizer=f=80:t=o:w=2:g=4,equalizer=f=1000:t=o:w=2:g=-1,equalizer=f=4000:t=o:w=2:g=3",
            EqualizerPreset::Jazz =>
                "equalizer=f=100:t=o:w=2:g=2,equalizer=f=500:t=o:w=2:g=2,equalizer=f=5000:t=o:w=2:g=2",
            EqualizerPreset::Classical =>
                "equalizer=f=80:t=o:w=2:g=2,equalizer=f=10000:t=o:w=2:g=2",
            EqualizerPreset::Electronic =>
                "equalizer=f=50:t=o:w=2:g=5,equalizer=f=4000:t=o:w=2:g=2,equalizer=f=10000:t=o:w=2:g=4",
            EqualizerPreset::Vocal =>
                "equalizer=f=200:t=o:w=2:g=-2,equalizer=f=3000:t=o:w=2:g=4",
        };

        let filter = if eq.is_empty() {
            loudnorm.to_string()
        } else {
            format!("{},{}", loudnorm, eq)
        };
        info!("🎛️ Filtro ffmpeg ({:?}) guild {}: {}", preset, guild_id, filter);
        filter
    }

    /// Aplica preset de ecualizador a una guild
    pub fn apply_equalizer_preset(&self, guild_id: GuildId, preset: EqualizerPreset) {
        self.presets.insert(guild_id, preset);
        info!("🎛️ Preset de ecualizador aplicado: {:?} (guild {})", preset, guild_id);
    }

    /// Obtiene el preset actual de la guild (Flat por defecto)
    pub fn get_current_preset(&self, guild_id: GuildId) -> EqualizerPreset {
        self.presets.get(&guild_id).map(|p| *p).unwrap_or(EqualizerPreset::Flat)
    }

    /// Obtiene detalles del ecualizador de la guild
    pub fn get_equalizer_details(&self, guild_id: GuildId) -> String {
        let preset = self.get_current_preset(guild_id);
        match preset {
            EqualizerPreset::Flat => "Ecualizador: Plano".to_string(),
            EqualizerPreset::Bass => "Ecualizador: Bass Boost".to_string(),
            EqualizerPreset::Pop => "Ecualizador: Pop".to_string(),
            EqualizerPreset::Rock => "Ecualizador: Rock".to_string(),
            EqualizerPreset::Jazz => "Ecualizador: Jazz".to_string(),
            EqualizerPreset::Classical => "Ecualizador: Clásica".to_string(),
            EqualizerPreset::Electronic => "Ecualizador: Electrónica".to_string(),
            EqualizerPreset::Vocal => "Ecualizador: Vocal".to_string(),
        }
    }

    /// Resetea el ecualizador de la guild a plano
    #[allow(dead_code)]
    pub fn reset_equalizer(&self, guild_id: GuildId) {
        self.apply_equalizer_preset(guild_id, EqualizerPreset::Flat);
        info!("🔄 Ecualizador reseteado a plano (guild {})", guild_id);
    }
}