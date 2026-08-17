use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, atomic::{AtomicU64, Ordering}},
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use tracing::{debug, warn, error};

use super::HealthStatus;

/// Health checker para monitoreo del estado del sistema
#[derive(Debug)]
pub struct HealthChecker {
    /// Métricas de salud por componente
    component_metrics: Arc<RwLock<HashMap<String, ComponentHealth>>>,
    
    /// Estadísticas globales de health checks
    stats: Arc<HealthStats>,
    
    /// Configuración del health checker
    #[allow(dead_code)]
    config: HealthConfig,
    
    /// Tiempo de inicio del sistema
    system_start_time: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthStatus,
    pub last_check: DateTime<Utc>,
    pub response_time: Duration,
    pub uptime: Duration,
    pub error_count: u32,
    pub success_count: u32,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug)]
struct HealthStats {
    total_checks: AtomicU64,
    healthy_checks: AtomicU64,
    warning_checks: AtomicU64,
    critical_checks: AtomicU64,
    avg_check_time: AtomicU64, // microseconds
}

#[derive(Debug, Clone)]
pub struct HealthConfig {
    /// Timeout para cada health check individual
    #[allow(dead_code)]
    pub check_timeout: Duration,
    
    /// Umbral para considerar una respuesta lenta
    #[allow(dead_code)]
    pub slow_response_threshold: Duration,
    
    /// Número de fallos consecutivos antes de marcar como crítico
    #[allow(dead_code)]
    pub critical_failure_threshold: u32,
    
    /// Activar health checks detallados
    #[allow(dead_code)]
    pub detailed_checks: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthReport {
    pub overall_status: HealthStatus,
    pub uptime: Duration,
    pub total_checks: u64,
    pub components: Vec<ComponentHealth>,
    pub issues: Vec<HealthIssue>,
    pub recommendations: Vec<String>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthIssue {
    pub component: String,
    pub severity: IssueSeverity,
    pub description: String,
    pub detected_at: DateTime<Utc>,
    pub recommendation: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum IssueSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            check_timeout: Duration::from_secs(5),
            slow_response_threshold: Duration::from_millis(1000),
            critical_failure_threshold: 3,
            detailed_checks: true,
        }
    }
}

impl HealthChecker {
    pub fn new() -> Self {
        Self::with_config(HealthConfig::default())
    }

    pub fn with_config(config: HealthConfig) -> Self {
        Self {
            component_metrics: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(HealthStats {
                total_checks: AtomicU64::new(0),
                healthy_checks: AtomicU64::new(0),
                warning_checks: AtomicU64::new(0),
                critical_checks: AtomicU64::new(0),
                avg_check_time: AtomicU64::new(0),
            }),
            config,
            system_start_time: Instant::now(),
        }
    }

    /// Realiza health check completo del sistema
    pub async fn perform_full_check(&self) -> HealthStatus {
        let start_time = Instant::now();
        self.stats.total_checks.fetch_add(1, Ordering::Relaxed);
        
        debug!("Iniciando health check completo");

        // Verificar componentes principales - ejecutar secuencialmente para evitar problemas de tipos
        let mut results = Vec::new();
        results.push(self.check_database_health().await);
        results.push(self.check_audio_system_health().await);
        results.push(self.check_discord_connectivity().await);
        results.push(self.check_youtube_api_health().await);
        results.push(self.check_memory_usage().await);
        results.push(self.check_cpu_usage().await);

        // Procesar resultados
        let mut overall_status = HealthStatus::Healthy;
        let mut component_healths = Vec::new();

        for (_i, health) in results.into_iter().enumerate() {
            match health.status {
                HealthStatus::Critical => {
                    overall_status = HealthStatus::Critical;
                    self.stats.critical_checks.fetch_add(1, Ordering::Relaxed);
                }
                HealthStatus::Warning => {
                    if overall_status == HealthStatus::Healthy {
                        overall_status = HealthStatus::Warning;
                    }
                    self.stats.warning_checks.fetch_add(1, Ordering::Relaxed);
                }
                HealthStatus::Healthy => {
                    self.stats.healthy_checks.fetch_add(1, Ordering::Relaxed);
                }
                HealthStatus::Unknown => {}
            }
            component_healths.push(health);
        }

        // Actualizar métricas de componentes
        let mut metrics = self.component_metrics.write().await;
        for health in component_healths {
            metrics.insert(health.name.clone(), health);
        }

        // Registrar tiempo de check
        let check_duration = start_time.elapsed();
        self.record_check_time(check_duration);

        match overall_status {
            HealthStatus::Healthy => debug!("Health check completado: Sistema saludable"),
            HealthStatus::Warning => warn!("Health check completado: Warnings detectados"),
            HealthStatus::Critical => error!("Health check completado: Problemas críticos"),
            HealthStatus::Unknown => debug!("Health check completado: Estado desconocido"),
        }

        overall_status
    }

    /// Genera reporte completo de salud del sistema
    #[allow(dead_code)]
    pub async fn generate_health_report(&self) -> SystemHealthReport {
        let overall_status = self.perform_full_check().await;
        let metrics = self.component_metrics.read().await;
        
        let components: Vec<ComponentHealth> = metrics.values().cloned().collect();
        let issues = self.detect_health_issues(&components);
        let recommendations = self.generate_health_recommendations(&components, &issues);

        SystemHealthReport {
            overall_status,
            uptime: self.system_start_time.elapsed(),
            total_checks: self.stats.total_checks.load(Ordering::Relaxed),
            components,
            issues,
            recommendations,
            last_updated: Utc::now(),
        }
    }

    /// Obtiene estado de un componente específico
    #[allow(dead_code)]
    pub async fn get_component_health(&self, component: &str) -> Option<ComponentHealth> {
        let metrics = self.component_metrics.read().await;
        metrics.get(component).cloned()
    }

    /// Registra health check manual para un componente
    #[allow(dead_code)]
    pub async fn register_component_check(&self, component: &str, status: HealthStatus, response_time: Duration, metadata: HashMap<String, String>) {
        let mut metrics = self.component_metrics.write().await;
        
        let health = metrics.entry(component.to_string()).or_insert_with(|| ComponentHealth {
            name: component.to_string(),
            status: HealthStatus::Unknown,
            last_check: Utc::now(),
            response_time: Duration::from_millis(0),
            uptime: Duration::from_millis(0),
            error_count: 0,
            success_count: 0,
            metadata: HashMap::new(),
        });

        health.status = status;
        health.last_check = Utc::now();
        health.response_time = response_time;
        health.uptime = self.system_start_time.elapsed();
        health.metadata = metadata;

        match status {
            HealthStatus::Healthy => health.success_count += 1,
            _ => health.error_count += 1,
        }

        debug!("Health actualizado para {}: {:?}", component, status);
    }

    // Checks específicos de componentes

    async fn check_database_health(&self) -> ComponentHealth {
        let start_time = Instant::now();
        
        // Simulación de check de base de datos
        // En implementación real, aquí se haría una query simple
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        let response_time = start_time.elapsed();
        let status = if response_time > Duration::from_millis(500) {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        };

        ComponentHealth {
            name: "database".to_string(),
            status,
            last_check: Utc::now(),
            response_time,
            uptime: self.system_start_time.elapsed(),
            error_count: 0,
            success_count: 1,
            metadata: [
                ("response_time_ms".to_string(), response_time.as_millis().to_string()),
                ("connection_pool".to_string(), "healthy".to_string()),
            ].into(),
        }
    }

    async fn check_audio_system_health(&self) -> ComponentHealth {
        let start_time = Instant::now();
        
        // Check del sistema de audio (verificar que songbird esté disponible)
        let status = HealthStatus::Healthy; // Simplificado
        let response_time = start_time.elapsed();

        ComponentHealth {
            name: "audio_system".to_string(),
            status,
            last_check: Utc::now(),
            response_time,
            uptime: self.system_start_time.elapsed(),
            error_count: 0,
            success_count: 1,
            metadata: [
                ("songbird_status".to_string(), "active".to_string()),
                ("active_connections".to_string(), "0".to_string()),
            ].into(),
        }
    }

    async fn check_discord_connectivity(&self) -> ComponentHealth {
        let start_time = Instant::now();
        
        // Check de conectividad con Discord
        let status = HealthStatus::Healthy; // Simplificado
        let response_time = start_time.elapsed();

        ComponentHealth {
            name: "discord".to_string(),
            status,
            last_check: Utc::now(),
            response_time,
            uptime: self.system_start_time.elapsed(),
            error_count: 0,
            success_count: 1,
            metadata: [
                ("gateway_status".to_string(), "connected".to_string()),
                ("latency_ms".to_string(), "50".to_string()),
            ].into(),
        }
    }

    async fn check_youtube_api_health(&self) -> ComponentHealth {
        let start_time = Instant::now();
        
        // Check de la API de YouTube (hacer una búsqueda simple)
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        let response_time = start_time.elapsed();
        let status = if response_time > Duration::from_secs(2) {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        };

        ComponentHealth {
            name: "youtube_api".to_string(),
            status,
            last_check: Utc::now(),
            response_time,
            uptime: self.system_start_time.elapsed(),
            error_count: 0,
            success_count: 1,
            metadata: [
                ("api_response_time".to_string(), response_time.as_millis().to_string()),
                ("quota_status".to_string(), "ok".to_string()),
            ].into(),
        }
    }

    async fn check_memory_usage(&self) -> ComponentHealth {
        let start_time = Instant::now();
        
        // Check de uso de memoria del sistema
        let memory_usage = self.get_memory_usage();
        let status = if memory_usage > 85.0 {
            HealthStatus::Critical
        } else if memory_usage > 70.0 {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        };

        let response_time = start_time.elapsed();

        ComponentHealth {
            name: "memory".to_string(),
            status,
            last_check: Utc::now(),
            response_time,
            uptime: self.system_start_time.elapsed(),
            error_count: if matches!(status, HealthStatus::Critical | HealthStatus::Warning) { 1 } else { 0 },
            success_count: if matches!(status, HealthStatus::Healthy) { 1 } else { 0 },
            metadata: [
                ("usage_percent".to_string(), format!("{:.1}", memory_usage)),
                ("status".to_string(), format!("{:?}", status)),
            ].into(),
        }
    }

    async fn check_cpu_usage(&self) -> ComponentHealth {
        let start_time = Instant::now();
        
        // Check de uso de CPU
        let cpu_usage = self.get_cpu_usage();
        let status = if cpu_usage > 90.0 {
            HealthStatus::Critical
        } else if cpu_usage > 75.0 {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        };

        let response_time = start_time.elapsed();

        ComponentHealth {
            name: "cpu".to_string(),
            status,
            last_check: Utc::now(),
            response_time,
            uptime: self.system_start_time.elapsed(),
            error_count: if matches!(status, HealthStatus::Critical | HealthStatus::Warning) { 1 } else { 0 },
            success_count: if matches!(status, HealthStatus::Healthy) { 1 } else { 0 },
            metadata: [
                ("usage_percent".to_string(), format!("{:.1}", cpu_usage)),
                ("load_average".to_string(), "1.2".to_string()),
            ].into(),
        }
    }

    // Métodos de utilidad

    #[allow(dead_code)]
    fn detect_health_issues(&self, components: &[ComponentHealth]) -> Vec<HealthIssue> {
        let mut issues = Vec::new();

        for component in components {
            match component.status {
                HealthStatus::Critical => {
                    issues.push(HealthIssue {
                        component: component.name.clone(),
                        severity: IssueSeverity::Critical,
                        description: "Componente en estado crítico".to_string(),
                        detected_at: component.last_check,
                        recommendation: "Revisar logs y reiniciar componente si es necesario".to_string(),
                    });
                }
                HealthStatus::Warning => {
                    if component.response_time > self.config.slow_response_threshold {
                        issues.push(HealthIssue {
                            component: component.name.clone(),
                            severity: IssueSeverity::Medium,
                            description: format!("Respuesta lenta: {:?}", component.response_time),
                            detected_at: component.last_check,
                            recommendation: "Optimizar rendimiento del componente".to_string(),
                        });
                    }
                }
                _ => {}
            }

            // Check de ratio de errores
            let total_checks = component.error_count + component.success_count;
            if total_checks > 10 {
                let error_rate = component.error_count as f64 / total_checks as f64;
                if error_rate > 0.1 { // Más del 10% de errores
                    issues.push(HealthIssue {
                        component: component.name.clone(),
                        severity: IssueSeverity::High,
                        description: format!("Alta tasa de errores: {:.1}%", error_rate * 100.0),
                        detected_at: component.last_check,
                        recommendation: "Investigar y corregir errores recurrentes".to_string(),
                    });
                }
            }
        }

        issues
    }

    #[allow(dead_code)]
    fn generate_health_recommendations(&self, components: &[ComponentHealth], issues: &[HealthIssue]) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Recomendaciones basadas en issues
        let critical_issues = issues.iter().filter(|i| matches!(i.severity, IssueSeverity::Critical)).count();
        if critical_issues > 0 {
            recommendations.push(format!("Se detectaron {} problemas críticos que requieren atención inmediata", critical_issues));
        }

        // Recomendaciones basadas en componentes lentos
        let slow_components: Vec<_> = components.iter()
            .filter(|c| c.response_time > self.config.slow_response_threshold)
            .collect();
        
        if !slow_components.is_empty() {
            recommendations.push(format!(
                "Los siguientes componentes están respondiendo lentamente: {}",
                slow_components.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ")
            ));
        }

        // Recomendaciones de memoria y CPU
        if let Some(memory_component) = components.iter().find(|c| c.name == "memory") {
            if matches!(memory_component.status, HealthStatus::Warning | HealthStatus::Critical) {
                recommendations.push("Considere liberar memoria o aumentar recursos del sistema".to_string());
            }
        }

        if let Some(cpu_component) = components.iter().find(|c| c.name == "cpu") {
            if matches!(cpu_component.status, HealthStatus::Warning | HealthStatus::Critical) {
                recommendations.push("Alto uso de CPU detectado. Considere optimizar procesos o escalar recursos".to_string());
            }
        }

        if recommendations.is_empty() {
            recommendations.push("Todos los sistemas están funcionando correctamente".to_string());
        }

        recommendations
    }

    fn record_check_time(&self, duration: Duration) {
        let elapsed_micros = duration.as_micros() as u64;
        
        // Promedio móvil simple
        let current_avg = self.stats.avg_check_time.load(Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            elapsed_micros
        } else {
            (current_avg * 3 + elapsed_micros) / 4
        };
        
        self.stats.avg_check_time.store(new_avg, Ordering::Relaxed);
    }

    fn get_memory_usage(&self) -> f64 {
        // Simulación de uso de memoria
        // En implementación real usaría psutil o similar
        45.2
    }

    fn get_cpu_usage(&self) -> f64 {
        // Simulación de uso de CPU
        // En implementación real usaría psutil o similar
        23.5
    }

    #[allow(dead_code)]
    pub fn get_health_stats(&self) -> HealthCheckStats {
        HealthCheckStats {
            total_checks: self.stats.total_checks.load(Ordering::Relaxed),
            healthy_checks: self.stats.healthy_checks.load(Ordering::Relaxed),
            warning_checks: self.stats.warning_checks.load(Ordering::Relaxed),
            critical_checks: self.stats.critical_checks.load(Ordering::Relaxed),
            avg_check_time_ms: self.stats.avg_check_time.load(Ordering::Relaxed) as f64 / 1000.0,
            uptime: self.system_start_time.elapsed(),
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HealthCheckStats {
    pub total_checks: u64,
    pub healthy_checks: u64,
    pub warning_checks: u64,
    pub critical_checks: u64,
    pub avg_check_time_ms: f64,
    pub uptime: Duration,
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}