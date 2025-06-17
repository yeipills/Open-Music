use anyhow::Result;
use parking_lot::RwLock;
use songbird::input::Input;
use std::collections::HashMap;
use tracing::{debug, info};

/// Presets de ecualizador predefinidos
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
    Custom,
}

/// Configuración de un efecto
#[derive(Debug, Clone)]
pub struct EffectConfig {
    pub enabled: bool,
    pub intensity: f32,
}

/// Sistema de efectos de audio
pub struct AudioEffects {
    equalizer_bands: RwLock<Vec<f32>>,
    current_preset: RwLock<EqualizerPreset>,
    effects: RwLock<HashMap<String, EffectConfig>>,
}

impl AudioEffects {
    pub fn new() -> Self {
        let mut effects = HashMap::new();

        // Inicializar efectos disponibles
        effects.insert(
            "bass_boost".to_string(),
            EffectConfig {
                enabled: false,
                intensity: 0.5,
            },
        );
        effects.insert(
            "8d".to_string(),
            EffectConfig {
                enabled: false,
                intensity: 0.7,
            },
        );
        effects.insert(
            "nightcore".to_string(),
            EffectConfig {
                enabled: false,
                intensity: 1.25,
            },
        );
        effects.insert(
            "vaporwave".to_string(),
            EffectConfig {
                enabled: false,
                intensity: 0.85,
            },
        );
        effects.insert(
            "tremolo".to_string(),
            EffectConfig {
                enabled: false,
                intensity: 0.5,
            },
        );
        effects.insert(
            "karaoke".to_string(),
            EffectConfig {
                enabled: false,
                intensity: 0.8,
            },
        );

        Self {
            // 10 bandas: 32, 64, 125, 250, 500, 1k, 2k, 4k, 8k, 16k Hz
            equalizer_bands: RwLock::new(vec![0.0; 10]),
            current_preset: RwLock::new(EqualizerPreset::Flat),
            effects: RwLock::new(effects),
        }
    }

    /// Procesa el input de audio aplicando efectos
    pub async fn process_input(&self, input: Input) -> Result<Input> {
        // Por ahora retornamos el input sin procesar
        // TODO: Implementar procesamiento real con FunDSP
        Ok(input)
    }

    /// Aplica un preset de ecualizador
    pub fn apply_preset(&self, preset: EqualizerPreset) {
        let bands = match preset {
            EqualizerPreset::Flat => vec![0.0; 10],
            EqualizerPreset::Bass => vec![6.0, 5.0, 4.0, 2.0, 0.0, -1.0, -2.0, -2.0, -1.0, 0.0],
            EqualizerPreset::Pop => vec![-1.0, 0.0, 2.0, 4.0, 4.0, 2.0, 0.0, -1.0, -1.0, -1.0],
            EqualizerPreset::Rock => vec![5.0, 4.0, 3.0, 0.0, -1.0, -1.0, 1.0, 3.0, 4.0, 5.0],
            EqualizerPreset::Jazz => vec![0.0, 0.0, 0.0, 2.0, 4.0, 4.0, 2.0, 0.0, 0.0, 0.0],
            EqualizerPreset::Classical => {
                vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -2.0, -2.0, -2.0, -3.0]
            }
            EqualizerPreset::Electronic => vec![5.0, 4.0, 1.0, 0.0, -2.0, 2.0, 1.0, 1.0, 4.0, 5.0],
            EqualizerPreset::Vocal => vec![-2.0, -3.0, -3.0, 1.0, 4.0, 4.0, 3.0, 1.0, 0.0, -1.0],
            EqualizerPreset::Custom => return, // No cambiar bandas para custom
        };

        *self.equalizer_bands.write() = bands;
        *self.current_preset.write() = preset;

        info!("🎛️ Preset de ecualizador aplicado: {:?}", preset);
    }

    /// Ajusta una banda específica del ecualizador
    pub fn set_band(&self, band_index: usize, gain: f32) -> Result<()> {
        let mut bands = self.equalizer_bands.write();

        if band_index >= bands.len() {
            anyhow::bail!("Índice de banda inválido: {}", band_index);
        }

        bands[band_index] = gain.clamp(-12.0, 12.0);
        *self.current_preset.write() = EqualizerPreset::Custom;

        debug!("Banda {} ajustada a {} dB", band_index, gain);
        Ok(())
    }

    /// Obtiene las bandas actuales del ecualizador
    pub fn get_bands(&self) -> Vec<f32> {
        self.equalizer_bands.read().clone()
    }

    /// Activa/desactiva un efecto
    pub fn toggle_effect(&self, effect_name: &str) -> Result<bool> {
        let mut effects = self.effects.write();

        if let Some(effect) = effects.get_mut(effect_name) {
            effect.enabled = !effect.enabled;
            info!(
                "🎭 Efecto '{}' {}",
                effect_name,
                if effect.enabled {
                    "activado"
                } else {
                    "desactivado"
                }
            );
            Ok(effect.enabled)
        } else {
            anyhow::bail!("Efecto no encontrado: {}", effect_name)
        }
    }

    /// Ajusta la intensidad de un efecto
    pub fn set_effect_intensity(&self, effect_name: &str, intensity: f32) -> Result<()> {
        let mut effects = self.effects.write();

        if let Some(effect) = effects.get_mut(effect_name) {
            effect.intensity = intensity.clamp(0.0, 2.0);
            debug!(
                "Intensidad de '{}' ajustada a {}",
                effect_name, effect.intensity
            );
            Ok(())
        } else {
            anyhow::bail!("Efecto no encontrado: {}", effect_name)
        }
    }

    /// Crea un procesador de audio FunDSP basado en la configuración actual
    // Temporarily commented out due to fundsp API changes
    /*fn create_processor(&self) -> Box<dyn AudioNode<Sample = f32> + Send> {
        let bands = self.equalizer_bands.read().clone();
        let effects = self.effects.read().clone();

        // Construir cadena de procesamiento
        let mut processor = Box::new(pass::<U2>()) as Box<dyn AudioNode<Sample = f32> + Send>;

        // Aplicar ecualizador si está habilitado
        if self.config.enable_equalizer && self.current_preset.read().clone() != EqualizerPreset::Flat {
            // Frecuencias centrales de las bandas
            let frequencies = [32.0, 64.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];

            for (i, (&freq, &gain)) in frequencies.iter().zip(bands.iter()).enumerate() {
                if gain.abs() > 0.1 {
                    // Aplicar filtro bell para cada banda
                    let q = 1.5; // Factor Q para el ancho de banda
                    let amp_gain = db_amp(gain as f64);

                    // processor = Box::new(processor >> bell_hz(freq as f64, q, amp_gain));
                }
            }
        }

        // Aplicar efectos adicionales
        if let Some(bass_boost) = effects.get("bass_boost") {
            if bass_boost.enabled {
                let boost = bass_boost.intensity * 6.0;
                // processor = Box::new(processor >> bell_hz(80.0, 0.7, db_amp(boost as f64)));
            }
        }

        if let Some(effect_8d) = effects.get("8d") {
            if effect_8d.enabled {
                // Implementar efecto 8D con paneo automático
                let lfo_freq = 0.3 * effect_8d.intensity;
                // processor = Box::new(processor >> pan(sine_hz(lfo_freq as f64)));
            }
        }

        // processor
        unimplemented!("FunDSP API needs updating")
    }*/

    /// Obtiene información sobre los efectos activos
    pub fn get_active_effects(&self) -> Vec<String> {
        self.effects
            .read()
            .iter()
            .filter(|(_, config)| config.enabled)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Resetea todos los efectos
    pub fn reset_all(&self) {
        *self.equalizer_bands.write() = vec![0.0; 10];
        *self.current_preset.write() = EqualizerPreset::Flat;

        for (_, effect) in self.effects.write().iter_mut() {
            effect.enabled = false;
        }

        info!("🔄 Todos los efectos reseteados");
    }
}

// Función auxiliar para convertir dB a amplitud
fn db_amp(db: f64) -> f64 {
    10.0_f64.powf(db / 20.0)
}
