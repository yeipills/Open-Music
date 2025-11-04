//! # Monitoring Module
//!
//! Sistema de monitoreo y métricas para Open Music Bot.
//!
//! Proporciona seguimiento de performance, health checks y tracking de errores.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Configuración del sistema de monitoreo
#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    /// Intervalo de muestreo de métricas
    pub sample_interval: Duration,
    /// Retención de métricas
    pub retention_period: Duration,
    /// Habilitar métricas detalladas
    pub detailed_metrics: bool,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            sample_interval: Duration::from_secs(30),
            retention_period: Duration::from_secs(3600), // 1 hora
            detailed_metrics: false,
        }
    }
}

/// Sistema de monitoreo principal
pub struct MonitoringSystem {
    config: MonitoringConfig,
    start_time: Instant,

    // Contadores atómicos
    commands_executed: Arc<AtomicU64>,
    errors_count: Arc<AtomicU64>,
    tracks_played: Arc<AtomicU64>,
    guilds_active: Arc<AtomicU64>,
}

impl MonitoringSystem {
    /// Crea una nueva instancia del sistema de monitoreo
    pub fn new(config: MonitoringConfig) -> Self {
        tracing::info!("📊 Inicializando sistema de monitoreo");

        Self {
            config,
            start_time: Instant::now(),
            commands_executed: Arc::new(AtomicU64::new(0)),
            errors_count: Arc::new(AtomicU64::new(0)),
            tracks_played: Arc::new(AtomicU64::new(0)),
            guilds_active: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Registra la ejecución de un comando
    pub fn record_command(&self) {
        self.commands_executed.fetch_add(1, Ordering::Relaxed);
    }

    /// Registra un error
    pub fn record_error(&self) {
        self.errors_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Registra una canción reproducida
    pub fn record_track_played(&self) {
        self.tracks_played.fetch_add(1, Ordering::Relaxed);
    }

    /// Actualiza el número de guilds activos
    pub fn update_active_guilds(&self, count: u64) {
        self.guilds_active.store(count, Ordering::Relaxed);
    }

    /// Obtiene el uptime del bot
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Obtiene métricas actuales
    pub fn metrics(&self) -> Metrics {
        Metrics {
            uptime: self.uptime(),
            commands_executed: self.commands_executed.load(Ordering::Relaxed),
            errors_count: self.errors_count.load(Ordering::Relaxed),
            tracks_played: self.tracks_played.load(Ordering::Relaxed),
            guilds_active: self.guilds_active.load(Ordering::Relaxed),
        }
    }

    /// Genera un resumen de métricas para logging
    pub fn summary(&self) -> String {
        let metrics = self.metrics();
        format!(
            "Uptime: {:?} | Commands: {} | Tracks: {} | Guilds: {} | Errors: {}",
            metrics.uptime,
            metrics.commands_executed,
            metrics.tracks_played,
            metrics.guilds_active,
            metrics.errors_count
        )
    }

    /// Realiza un health check del sistema y retorna estado
    pub async fn perform_health_check(&self) -> HealthStatus {
        let metrics = self.metrics();
        let error_rate = metrics.error_rate();

        if error_rate >= 20.0 {
            HealthStatus::Critical
        } else if error_rate >= 10.0 {
            HealthStatus::Warning
        } else if self.uptime() > Duration::from_secs(10) {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unknown
        }
    }

    /// Obtiene información detallada de salud
    pub async fn get_health_info(&self) -> HealthInfo {
        let status = self.perform_health_check().await;
        let uptime = self.uptime();
        let metrics = self.metrics();

        // Simular métricas de sistema (en producción esto vendría de sysinfo u otra fuente)
        let memory_usage_mb = 75.0; // Placeholder
        let cpu_usage_percent = 5.0; // Placeholder

        HealthInfo {
            status: status.clone(),
            message: match status {
                HealthStatus::Healthy => "Sistema operando normalmente".to_string(),
                HealthStatus::Warning => format!("Tasa de errores elevada: {:.2}%", metrics.error_rate()),
                HealthStatus::Critical => "Sistema con problemas críticos".to_string(),
                HealthStatus::Unknown => "Estado desconocido".to_string(),
            },
            uptime,
            memory_usage_mb,
            cpu_usage_percent,
        }
    }

    /// Obtiene métricas del sistema
    pub async fn get_system_metrics(&self) -> SystemMetrics {
        let metrics = self.metrics();

        SystemMetrics {
            memory_usage_mb: 75.0, // Placeholder - usar sysinfo en producción
            cpu_usage_percent: 5.0, // Placeholder
            thread_count: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
            uptime: self.uptime(),
            total_commands: metrics.commands_executed,
            total_errors: metrics.errors_count,
            total_warnings: 0, // Placeholder - necesitaría tracking de warnings
            error_rate: metrics.error_rate(),
            health_status: if metrics.error_rate() < 10.0 {
                "Healthy".to_string()
            } else {
                "Degraded".to_string()
            },
        }
    }

    /// Obtiene reporte de errores
    pub async fn get_error_report(&self, _hours: Option<usize>) -> ErrorReport {
        let metrics = self.metrics();

        // Placeholder - en producción esto vendría de un sistema de logging real
        let categories = vec![
            ErrorCategory {
                name: "Network Errors".to_string(),
                count: metrics.errors_count / 3,
                percentage: 33.3,
            },
            ErrorCategory {
                name: "Audio Errors".to_string(),
                count: metrics.errors_count / 3,
                percentage: 33.3,
            },
            ErrorCategory {
                name: "Command Errors".to_string(),
                count: metrics.errors_count / 3,
                percentage: 33.4,
            },
        ];

        ErrorReport {
            total_errors: metrics.errors_count,
            recent_errors: vec![], // Placeholder - necesitaría un log de errores
            error_rate: metrics.error_rate(),
            categories,
        }
    }
}

/// Estado de salud del sistema (enum)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

/// Información detallada de salud
#[derive(Debug, Clone)]
pub struct HealthInfo {
    pub status: HealthStatus,
    pub message: String,
    pub uptime: Duration,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
}

/// Métricas del sistema
#[derive(Debug, Clone)]
pub struct SystemMetrics {
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub thread_count: usize,
    pub uptime: Duration,
    pub total_commands: u64,
    pub total_errors: u64,
    pub total_warnings: u64,
    pub error_rate: f64,
    pub health_status: String,
}

/// Categoría de errores
#[derive(Debug, Clone)]
pub struct ErrorCategory {
    pub name: String,
    pub count: u64,
    pub percentage: f64,
}

/// Reporte de errores
#[derive(Debug, Clone)]
pub struct ErrorReport {
    pub total_errors: u64,
    pub recent_errors: Vec<String>,
    pub error_rate: f64,
    pub categories: Vec<ErrorCategory>,
}

/// Snapshot de métricas
#[derive(Debug, Clone)]
pub struct Metrics {
    pub uptime: Duration,
    pub commands_executed: u64,
    pub errors_count: u64,
    pub tracks_played: u64,
    pub guilds_active: u64,
}

impl Metrics {
    /// Calcula tasa de comandos por minuto
    pub fn commands_per_minute(&self) -> f64 {
        let minutes = self.uptime.as_secs() as f64 / 60.0;
        if minutes > 0.0 {
            self.commands_executed as f64 / minutes
        } else {
            0.0
        }
    }

    /// Calcula tasa de errores
    pub fn error_rate(&self) -> f64 {
        if self.commands_executed > 0 {
            (self.errors_count as f64 / self.commands_executed as f64) * 100.0
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests;
