use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, atomic::{AtomicU64, Ordering}},
    time::{Duration, Instant},
};
use tokio::{sync::RwLock, time::interval};
use tracing::{debug, info, warn, error, Level};

pub mod error_tracker;
pub mod performance_monitor;
pub mod health_checker;

/// Sistema completo de monitoreo y logging de errores
#[derive(Debug)]
pub struct MonitoringSystem {
    /// Tracker de errores por categoría
    error_tracker: Arc<error_tracker::ErrorTracker>,
    
    /// Monitor de rendimiento
    performance_monitor: Arc<performance_monitor::PerformanceMonitor>,
    
    /// Checker de salud del sistema
    health_checker: Arc<health_checker::HealthChecker>,
    
    /// Configuración global
    config: MonitoringConfig,
    
    /// Métricas globales
    global_metrics: Arc<GlobalMetrics>,
}

#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    /// Habilitar logging detallado
    #[allow(dead_code)]
    pub detailed_logging: bool,
    
    /// Nivel mínimo de log
    #[allow(dead_code)]
    pub log_level: Level,
    
    /// Intervalo de reporte de métricas
    pub metrics_interval: Duration,
    
    /// Retener errores por este tiempo
    pub error_retention: Duration,
    
    /// Umbral de errores para alertas
    #[allow(dead_code)]
    pub error_threshold: u32,
    
    /// Habilitar monitoreo de rendimiento
    #[allow(dead_code)]
    pub performance_monitoring: bool,
    
    /// Habilitar health checks
    pub health_checks: bool,
}

#[derive(Debug)]
struct GlobalMetrics {
    /// Total de comandos procesados
    total_commands: AtomicU64,
    
    /// Total de errores registrados
    total_errors: AtomicU64,
    
    /// Total de warnings
    total_warnings: AtomicU64,
    
    /// Tiempo de inicio del sistema
    start_time: Instant,
    
    /// Último reporte de métricas
    last_metrics_report: RwLock<DateTime<Utc>>,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            detailed_logging: true,
            log_level: Level::INFO,
            metrics_interval: Duration::from_secs(300), // 5 minutos
            error_retention: Duration::from_secs(3600 * 24), // 24 horas
            error_threshold: 10,
            performance_monitoring: true,
            health_checks: true,
        }
    }
}

impl MonitoringSystem {
    pub fn new(config: MonitoringConfig) -> Self {
        let system = Self {
            error_tracker: Arc::new(error_tracker::ErrorTracker::new(config.error_retention)),
            performance_monitor: Arc::new(performance_monitor::PerformanceMonitor::new()),
            health_checker: Arc::new(health_checker::HealthChecker::new()),
            config: config.clone(),
            global_metrics: Arc::new(GlobalMetrics {
                total_commands: AtomicU64::new(0),
                total_errors: AtomicU64::new(0),
                total_warnings: AtomicU64::new(0),
                start_time: Instant::now(),
                last_metrics_report: RwLock::new(Utc::now()),
            }),
        };

        // Iniciar tareas de monitoreo
        system.start_monitoring_tasks();
        
        info!("Sistema de monitoreo iniciado");
        system
    }

    /// Registra un error en el sistema
    #[allow(dead_code)]
    pub async fn log_error(&self, category: &str, error: &str, context: Option<HashMap<String, String>>) {
        self.global_metrics.total_errors.fetch_add(1, Ordering::Relaxed);
        
        // Registrar en el tracker de errores
        self.error_tracker.record_error(category, error, context.clone()).await;
        
        // Log estructurado
        if self.config.detailed_logging {
            if let Some(ctx) = context {
                error!("{}: {} | Context: {:?}", category, error, ctx);
            } else {
                error!("{}: {}", category, error);
            }
        } else {
            error!("{}: {}", category, error);
        }

        // Verificar si se alcanzó el umbral de errores
        self.check_error_threshold(category).await;
    }

    /// Registra un warning
    #[allow(dead_code)]
    pub async fn log_warning(&self, category: &str, message: &str, context: Option<HashMap<String, String>>) {
        self.global_metrics.total_warnings.fetch_add(1, Ordering::Relaxed);
        
        self.error_tracker.record_warning(category, message, context.clone()).await;
        
        if self.config.detailed_logging {
            if let Some(ctx) = context {
                warn!("{}: {} | Context: {:?}", category, message, ctx);
            } else {
                warn!("{}: {}", category, message);
            }
        } else {
            warn!("{}: {}", category, message);
        }
    }

    /// Registra información de performance
    #[allow(dead_code)]
    pub async fn log_performance(&self, operation: &str, duration: Duration, metadata: Option<HashMap<String, String>>) {
        if !self.config.performance_monitoring {
            return;
        }

        self.performance_monitor.record_operation(operation, duration, metadata.clone()).await;
        
        if duration > Duration::from_secs(5) {
            warn!("Operación lenta detectada: {} tardó {:?}", operation, duration);
        } else if self.config.detailed_logging {
            debug!("{}: {:?}", operation, duration);
        }
    }

    /// Registra comando ejecutado
    #[allow(dead_code)]
    pub fn log_command(&self, command: &str, user_id: u64, guild_id: Option<u64>) {
        self.global_metrics.total_commands.fetch_add(1, Ordering::Relaxed);
        
        let mut context = HashMap::new();
        context.insert("user_id".to_string(), user_id.to_string());
        if let Some(guild) = guild_id {
            context.insert("guild_id".to_string(), guild.to_string());
        }
        
        info!("Comando ejecutado: {} por usuario {}", command, user_id);
        
        // Registrar en performance monitor
        tokio::spawn({
            let perf_monitor = self.performance_monitor.clone();
            let cmd = command.to_string();
            async move {
                perf_monitor.record_command(&cmd).await;
            }
        });
    }

    /// Realiza health check completo del sistema
    pub async fn perform_health_check(&self) -> HealthStatus {
        if !self.config.health_checks {
            return HealthStatus::Unknown;
        }

        self.health_checker.perform_full_check().await
    }

    /// Obtiene métricas completas del sistema
    pub async fn get_system_metrics(&self) -> SystemMetrics {
        let uptime = self.global_metrics.start_time.elapsed();
        let error_stats = self.error_tracker.get_summary().await;
        let performance_stats = self.performance_monitor.get_summary().await;
        let health_status = self.perform_health_check().await;

        SystemMetrics {
            uptime,
            total_commands: self.global_metrics.total_commands.load(Ordering::Relaxed),
            total_errors: self.global_metrics.total_errors.load(Ordering::Relaxed),
            total_warnings: self.global_metrics.total_warnings.load(Ordering::Relaxed),
            error_rate: self.calculate_error_rate(),
            health_status,
            error_summary: error_stats,
            performance_summary: performance_stats,
        }
    }

    /// Obtiene reporte detallado de errores
    pub async fn get_error_report(&self, hours: Option<u32>) -> error_tracker::ErrorReport {
        let timeframe = hours.map(|h| Duration::from_secs(h as u64 * 3600));
        self.error_tracker.generate_report(timeframe).await
    }

    /// Limpia datos antiguos de monitoreo
    #[allow(dead_code)]
    pub async fn cleanup_old_data(&self) {
        info!("Iniciando limpieza de datos de monitoreo");
        
        let cleaned_errors = self.error_tracker.cleanup_old_entries().await;
        let cleaned_performance = self.performance_monitor.cleanup_old_entries().await;
        
        info!("Limpieza completada: {} errores, {} registros de performance",
              cleaned_errors, cleaned_performance);
    }

    /// Configura alertas personalizadas
    #[allow(dead_code)]
    pub fn configure_alerts(&mut self, error_threshold: u32, performance_threshold: Duration) {
        self.config.error_threshold = error_threshold;
        info!("Alertas configuradas: {} errores, {:?} performance",
              error_threshold, performance_threshold);
    }

    // Métodos privados

    fn start_monitoring_tasks(&self) {
        // Tarea de reporte de métricas
        self.start_metrics_reporting();
        
        // Tarea de limpieza automática
        self.start_cleanup_task();
        
        // Tarea de health checks periódicos
        if self.config.health_checks {
            self.start_health_check_task();
        }
    }

    fn start_metrics_reporting(&self) {
        let metrics = self.global_metrics.clone();
        let interval_duration = self.config.metrics_interval;
        
        tokio::spawn(async move {
            let mut interval = interval(interval_duration);
            
            loop {
                interval.tick().await;
                
                let commands = metrics.total_commands.load(Ordering::Relaxed);
                let errors = metrics.total_errors.load(Ordering::Relaxed);
                let warnings = metrics.total_warnings.load(Ordering::Relaxed);
                let uptime = metrics.start_time.elapsed();
                
                info!("Métricas del sistema - Comandos: {}, Errores: {}, Warnings: {}, Uptime: {:?}",
                      commands, errors, warnings, uptime);
                
                *metrics.last_metrics_report.write().await = Utc::now();
            }
        });
    }

    fn start_cleanup_task(&self) {
        let error_tracker = self.error_tracker.clone();
        let performance_monitor = self.performance_monitor.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(3600)); // Cada hora
            
            loop {
                interval.tick().await;
                
                let _ = error_tracker.cleanup_old_entries().await;
                let _ = performance_monitor.cleanup_old_entries().await;
            }
        });
    }

    fn start_health_check_task(&self) {
        let health_checker = self.health_checker.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(300)); // Cada 5 minutos
            
            loop {
                interval.tick().await;
                
                let status = health_checker.perform_full_check().await;
                
                match status {
                    HealthStatus::Critical => {
                        error!("Health check crítico - sistema en estado crítico");
                    }
                    HealthStatus::Warning => {
                        warn!("Health check warning - problemas detectados");
                    }
                    HealthStatus::Healthy => {
                        debug!("Health check OK");
                    }
                    HealthStatus::Unknown => {
                        debug!("Health check desconocido");
                    }
                }
            }
        });
    }

    async fn check_error_threshold(&self, category: &str) {
        let recent_errors = self.error_tracker.get_recent_error_count(category, Duration::from_secs(300)).await;
        
        if recent_errors >= self.config.error_threshold {
            error!("ALERTA: {} errores de tipo '{}' en los últimos 5 minutos", recent_errors, category);
            
            // Aquí se podría implementar notificaciones adicionales
            // como webhooks, emails, etc.
        }
    }

    fn calculate_error_rate(&self) -> f64 {
        let total_commands = self.global_metrics.total_commands.load(Ordering::Relaxed);
        let total_errors = self.global_metrics.total_errors.load(Ordering::Relaxed);
        
        if total_commands == 0 {
            0.0
        } else {
            (total_errors as f64 / total_commands as f64) * 100.0
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub uptime: Duration,
    pub total_commands: u64,
    pub total_errors: u64,
    pub total_warnings: u64,
    pub error_rate: f64,
    pub health_status: HealthStatus,
    pub error_summary: error_tracker::ErrorSummary,
    pub performance_summary: performance_monitor::PerformanceSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

/// Trait para objetos que pueden ser monitoreados
#[allow(dead_code)]
pub trait Monitorable {
    async fn log_operation(&self, operation: &str, duration: Duration);
    async fn log_error(&self, error: &str, context: Option<HashMap<String, String>>);
    async fn get_health_status(&self) -> HealthStatus;
}

/// Macro para logging fácil con contexto
#[macro_export]
macro_rules! monitor_operation {
    ($monitor:expr, $operation:expr, $code:block) => {{
        let start = std::time::Instant::now();
        let result = $code;
        let duration = start.elapsed();
        
        $monitor.log_performance($operation, duration, None).await;
        
        result
    }};
    
    ($monitor:expr, $operation:expr, $context:expr, $code:block) => {{
        let start = std::time::Instant::now();
        let result = $code;
        let duration = start.elapsed();
        
        $monitor.log_performance($operation, duration, Some($context)).await;
        
        result
    }};
}

/// Macro para logging de errores con contexto automático
#[macro_export]
macro_rules! log_error {
    ($monitor:expr, $category:expr, $error:expr) => {
        $monitor.log_error($category, &$error.to_string(), None).await;
    };
    
    ($monitor:expr, $category:expr, $error:expr, $context:expr) => {
        $monitor.log_error($category, &$error.to_string(), Some($context)).await;
    };
}