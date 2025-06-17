use anyhow::Result;
use parking_lot::RwLock;
use std::{collections::HashMap, sync::Arc};
use tracing::info;

/// Frecuencias centrales para el ecualizador de 10 bandas
const EQ_FREQUENCIES: [f32; 10] = [
    32.0,    // Sub-bass
    64.0,    // Bass
    125.0,   // Low-mid
    250.0,   // Mid
    500.0,   // Upper-mid
    1000.0,  // Presence
    2000.0,  // Brilliance
    4000.0,  // High
    8000.0,  // Very high
    16000.0, // Air
];

/// Ancho de banda Q para cada frecuencia
const EQ_Q: f32 = 1.414; // Factor Q estándar (octava)

/// Preset de ecualizador
#[derive(Debug, Clone)]
pub struct EqPreset {
    pub name: String,
    pub gains: [f32; 10], // Ganancias en dB para cada banda
}

impl EqPreset {
    /// Crea un preset personalizado
    pub fn custom(gains: [f32; 10]) -> Self {
        Self {
            name: "Custom".to_string(),
            gains,
        }
    }
}

/// Presets predefinidos
pub struct EqPresets;

impl EqPresets {
    pub fn normal() -> EqPreset {
        EqPreset {
            name: "Normal".to_string(),
            gains: [0.0; 10],
        }
    }

    pub fn bass() -> EqPreset {
        EqPreset {
            name: "Bass".to_string(),
            gains: [6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn pop() -> EqPreset {
        EqPreset {
            name: "Pop".to_string(),
            gains: [-1.0, 2.0, 4.0, 5.0, 3.0, 0.0, -1.0, -1.0, 0.0, 0.0],
        }
    }

    pub fn rock() -> EqPreset {
        EqPreset {
            name: "Rock".to_string(),
            gains: [5.0, 4.0, 3.0, 1.0, -1.0, -1.0, 0.0, 2.0, 3.0, 4.0],
        }
    }

    pub fn jazz() -> EqPreset {
        EqPreset {
            name: "Jazz".to_string(),
            gains: [0.0, 1.0, 2.0, 3.0, 2.0, 1.0, 0.0, 1.0, 2.0, 3.0],
        }
    }

    pub fn classical() -> EqPreset {
        EqPreset {
            name: "Classical".to_string(),
            gains: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -2.0, -2.0, -2.0, -3.0],
        }
    }

    pub fn electronic() -> EqPreset {
        EqPreset {
            name: "Electronic".to_string(),
            gains: [5.0, 4.0, 1.0, 0.0, -2.0, 2.0, 1.0, 0.0, 3.0, 4.0],
        }
    }

    pub fn vocal() -> EqPreset {
        EqPreset {
            name: "Vocal".to_string(),
            gains: [-2.0, -1.0, 0.0, 2.0, 4.0, 3.0, 2.0, 1.0, 0.0, -1.0],
        }
    }

    /// Obtiene un preset por nombre
    pub fn get(name: &str) -> Option<EqPreset> {
        match name.to_lowercase().as_str() {
            "normal" => Some(Self::normal()),
            "bass" => Some(Self::bass()),
            "pop" => Some(Self::pop()),
            "rock" => Some(Self::rock()),
            "jazz" => Some(Self::jazz()),
            "classical" => Some(Self::classical()),
            "electronic" => Some(Self::electronic()),
            "vocal" => Some(Self::vocal()),
            _ => None,
        }
    }

    /// Lista todos los presets disponibles
    pub fn list() -> Vec<&'static str> {
        vec![
            "normal",
            "bass",
            "pop",
            "rock",
            "jazz",
            "classical",
            "electronic",
            "vocal",
        ]
    }
}

/// Ecualizador paramétrico de 10 bandas
#[derive(Debug)]
pub struct Equalizer {
    current_preset: Arc<RwLock<EqPreset>>,
    enabled: Arc<RwLock<bool>>,
    custom_presets: Arc<RwLock<HashMap<String, EqPreset>>>,
}

impl Equalizer {
    /// Crea un nuevo ecualizador
    pub fn new() -> Self {
        Self {
            current_preset: Arc::new(RwLock::new(EqPresets::normal())),
            enabled: Arc::new(RwLock::new(true)),
            custom_presets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Aplica un preset
    pub fn apply_preset(&self, preset_name: &str) -> Result<()> {
        if let Some(preset) = EqPresets::get(preset_name) {
            *self.current_preset.write() = preset;
            info!("🎛️ Preset '{}' aplicado", preset_name);
            Ok(())
        } else if let Some(custom) = self.custom_presets.read().get(preset_name) {
            *self.current_preset.write() = custom.clone();
            info!("🎛️ Preset personalizado '{}' aplicado", preset_name);
            Ok(())
        } else {
            anyhow::bail!("Preset '{}' no encontrado", preset_name)
        }
    }

    /// Aplica ganancias personalizadas
    pub fn apply_custom(&self, gains: [f32; 10]) -> Result<()> {
        // Validar que las ganancias estén en rango (-15 a +15 dB)
        for (i, &gain) in gains.iter().enumerate() {
            if gain < -15.0 || gain > 15.0 {
                anyhow::bail!(
                    "Ganancia fuera de rango en banda {}: {} dB (debe estar entre -15 y +15)",
                    i + 1,
                    gain
                );
            }
        }

        *self.current_preset.write() = EqPreset::custom(gains);
        info!("🎛️ Ecualizador personalizado aplicado");
        Ok(())
    }

    /// Guarda un preset personalizado
    pub fn save_custom_preset(&self, name: String, gains: [f32; 10]) -> Result<()> {
        let preset = EqPreset {
            name: name.clone(),
            gains,
        };

        self.custom_presets.write().insert(name.clone(), preset);
        info!("💾 Preset personalizado '{}' guardado", name);
        Ok(())
    }

    /// Activa/desactiva el ecualizador
    pub fn set_enabled(&self, enabled: bool) {
        *self.enabled.write() = enabled;
        info!(
            "🎛️ Ecualizador {}",
            if enabled { "activado" } else { "desactivado" }
        );
    }

    /// Obtiene el estado actual
    pub fn is_enabled(&self) -> bool {
        *self.enabled.read()
    }

    /// Obtiene el preset actual
    pub fn current_preset(&self) -> EqPreset {
        self.current_preset.read().clone()
    }

    /// Obtiene las ganancias actuales
    pub fn current_gains(&self) -> [f32; 10] {
        self.current_preset.read().gains
    }

    /// Resetea a valores por defecto
    pub fn reset(&self) {
        *self.current_preset.write() = EqPresets::normal();
        info!("🔄 Ecualizador reseteado");
    }

    /// Parsea una cadena de configuración de ecualizador
    /// Formato: "32:2 64:1 125:0 250:-1 500:0 1k:1 2k:2 4k:1 8k:0 16k:-1"
    pub fn parse_config(config: &str) -> Result<[f32; 10]> {
        let mut gains = [0.0; 10];
        let parts: Vec<&str> = config.split_whitespace().collect();

        if parts.len() != 10 {
            anyhow::bail!("Se esperan 10 valores de ganancia");
        }

        for (i, part) in parts.iter().enumerate() {
            let kv: Vec<&str> = part.split(':').collect();
            if kv.len() != 2 {
                anyhow::bail!("Formato inválido en '{}', use 'frecuencia:ganancia'", part);
            }

            let gain: f32 = kv[1]
                .parse()
                .map_err(|_| anyhow::anyhow!("Ganancia inválida en '{}'", part))?;

            if gain < -15.0 || gain > 15.0 {
                anyhow::bail!(
                    "Ganancia fuera de rango: {} (debe estar entre -15 y +15)",
                    gain
                );
            }

            gains[i] = gain;
        }

        Ok(gains)
    }
}

impl Default for Equalizer {
    fn default() -> Self {
        Self::new()
    }
}
