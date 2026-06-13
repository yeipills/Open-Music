use parking_lot::RwLock;
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

/// Sistema de ecualizador con presets
pub struct AudioEffects {
    current_preset: RwLock<EqualizerPreset>,
}

impl AudioEffects {
    pub fn new() -> Self {
        info!("🎛️ Sistema de ecualizador inicializado");
        Self {
            current_preset: RwLock::new(EqualizerPreset::Flat),
        }
    }

    /// Construye la cadena de filtros ffmpeg (`-af`) para el preset actual.
    ///
    /// Siempre incluye `loudnorm` (EBU R128) para igualar el volumen percibido entre
    /// temas, y añade bandas `equalizer` según el preset. Para `Flat` solo normaliza.
    pub fn build_filter(&self) -> String {
        // loudnorm de una sola pasada: consistente y apto para streaming.
        let loudnorm = "loudnorm=I=-16:TP=-1.5:LRA=11";

        let eq = match self.get_current_preset() {
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
        info!("🎛️ Filtro ffmpeg ({:?}): {}", self.get_current_preset(), filter);
        filter
    }

    /// Aplica preset de ecualizador
    pub fn apply_equalizer_preset(&self, preset: EqualizerPreset) {
        *self.current_preset.write() = preset;
        info!("🎛️ Preset de ecualizador aplicado: {:?}", preset);
    }

    /// Obtiene preset actual
    pub fn get_current_preset(&self) -> EqualizerPreset {
        *self.current_preset.read()
    }

    /// Obtiene detalles del ecualizador
    pub fn get_equalizer_details(&self) -> String {
        let preset = self.get_current_preset();
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

    /// Resetea el ecualizador a plano
    #[allow(dead_code)]
    pub fn reset_equalizer(&self) {
        self.apply_equalizer_preset(EqualizerPreset::Flat);
        info!("🔄 Ecualizador reseteado a plano");
    }
}