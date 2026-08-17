use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Duration,
};
use tokio::sync::RwLock;
use tracing::debug;

/// Monitor de rendimiento para operaciones del bot
#[derive(Debug)]
pub struct PerformanceMonitor {
    /// Registros de operaciones por tipo
    operations: Arc<RwLock<HashMap<String, VecDeque<OperationRecord>>>>,
    
    /// Estadísticas de comandos
    command_stats: Arc<RwLock<HashMap<String, CommandStats>>>,
    
    /// Métricas de rendimiento global
    global_metrics: Arc<RwLock<GlobalPerformanceMetrics>>,
    
    /// Configuración
    config: PerformanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationRecord {
    pub timestamp: DateTime<Utc>,
    pub operation: String,
    pub duration: Duration,
    pub metadata: Option<HashMap<String, String>>,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandStats {
    pub total_executions: u64,
    pub total_duration: Duration,
    pub avg_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
    pub success_rate: f64,
    pub last_execution: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalPerformanceMetrics {
    pub total_operations: u64,
    pub avg_operation_time: Duration,
    pub slowest_operations: Vec<SlowOperation>,
    pub performance_trend: PerformanceTrend,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowOperation {
    pub operation: String,
    pub duration: Duration,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PerformanceTrend {
    Improving,
    Stable,
    Degrading,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    /// Número máximo de registros por operación
    #[allow(dead_code)]
    pub max_records_per_operation: usize,
    /// Habilitar tracking de metadata
    #[allow(dead_code)]
    pub track_metadata: bool,
    pub slow_operation_threshold: Duration,
    pub retention_period: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSummary {
    pub total_operations: u64,
    pub avg_response_time: Duration,
    pub slow_operations_count: u64,
    pub fastest_operation: Option<String>,
    pub slowest_operation: Option<String>,
    pub command_performance: Vec<CommandPerformance>,
    pub trend: PerformanceTrend,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandPerformance {
    pub command: String,
    pub executions: u64,
    pub avg_duration: Duration,
    pub success_rate: f64,
    pub trend: PerformanceTrend,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub timeframe: Duration,
    pub operation_analysis: Vec<OperationAnalysis>,
    pub performance_bottlenecks: Vec<PerformanceBottleneck>,
    pub recommendations: Vec<String>,
    pub metrics_over_time: Vec<TimeMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationAnalysis {
    pub operation: String,
    pub total_executions: u64,
    pub avg_duration: Duration,
    pub p95_duration: Duration,
    pub p99_duration: Duration,
    pub success_rate: f64,
    pub trend: PerformanceTrend,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBottleneck {
    pub operation: String,
    pub issue_type: BottleneckType,
    pub severity: Severity,
    pub description: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BottleneckType {
    SlowAverage,
    HighVariability,
    LowSuccessRate,
    MemoryLeak,
    FrequentErrors,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeMetrics {
    pub timestamp: DateTime<Utc>,
    pub avg_duration: Duration,
    pub operation_count: u64,
    pub slow_operations: u64,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_records_per_operation: 1000,
            slow_operation_threshold: Duration::from_secs(5),
            retention_period: Duration::from_secs(24 * 3600), // 24 horas
            track_metadata: true,
        }
    }
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self::with_config(PerformanceConfig::default())
    }

    pub fn with_config(config: PerformanceConfig) -> Self {
        Self {
            operations: Arc::new(RwLock::new(HashMap::new())),
            command_stats: Arc::new(RwLock::new(HashMap::new())),
            global_metrics: Arc::new(RwLock::new(GlobalPerformanceMetrics {
                total_operations: 0,
                avg_operation_time: Duration::from_millis(0),
                slowest_operations: Vec::new(),
                performance_trend: PerformanceTrend::Unknown,
                last_updated: Utc::now(),
            })),
            config,
        }
    }

    /// Registra una operación completada
    #[allow(dead_code)]
    pub async fn record_operation(&self, operation: &str, duration: Duration, metadata: Option<HashMap<String, String>>) {
        let record = OperationRecord {
            timestamp: Utc::now(),
            operation: operation.to_string(),
            duration,
            metadata,
            success: true,
        };

        let mut operations = self.operations.write().await;
        let operation_records = operations.entry(operation.to_string()).or_insert_with(VecDeque::new);
        
        operation_records.push_back(record.clone());
        
        // Limitar número de registros
        while operation_records.len() > self.config.max_records_per_operation {
            operation_records.pop_front();
        }

        drop(operations);

        // Verificar si es una operación lenta
        if duration > self.config.slow_operation_threshold {
            self.record_slow_operation(operation, duration, record.metadata).await;
        }

        // Actualizar métricas globales
        self.update_global_metrics().await;
        
        debug!("Operación registrada: {} - {:?}", operation, duration);
    }

    /// Registra una operación fallida
    #[allow(dead_code)]
    pub async fn record_failed_operation(&self, operation: &str, duration: Duration, metadata: Option<HashMap<String, String>>) {
        let record = OperationRecord {
            timestamp: Utc::now(),
            operation: operation.to_string(),
            duration,
            metadata,
            success: false,
        };

        let mut operations = self.operations.write().await;
        let operation_records = operations.entry(operation.to_string()).or_insert_with(VecDeque::new);
        operation_records.push_back(record);

        while operation_records.len() > self.config.max_records_per_operation {
            operation_records.pop_front();
        }

        drop(operations);
        self.update_global_metrics().await;
    }

    /// Registra estadísticas de comando
    #[allow(dead_code)]
    pub async fn record_command(&self, command: &str) {
        let mut stats = self.command_stats.write().await;
        let command_stat = stats.entry(command.to_string()).or_insert(CommandStats {
            total_executions: 0,
            total_duration: Duration::from_millis(0),
            avg_duration: Duration::from_millis(0),
            min_duration: Duration::from_secs(u64::MAX),
            max_duration: Duration::from_millis(0),
            success_rate: 100.0,
            last_execution: Utc::now(),
        });

        command_stat.total_executions += 1;
        command_stat.last_execution = Utc::now();
    }

    /// Obtiene resumen de rendimiento
    pub async fn get_summary(&self) -> PerformanceSummary {
        let operations = self.operations.read().await;
        let command_stats = self.command_stats.read().await;
        let global_metrics = self.global_metrics.read().await;

        let total_operations = operations.values().map(|records| records.len() as u64).sum();
        
        // Calcular operación más rápida y más lenta
        let mut fastest_op: Option<(String, Duration)> = None;
        let mut slowest_op: Option<(String, Duration)> = None;
        let mut slow_count = 0u64;

        for (op_name, records) in operations.iter() {
            for record in records.iter() {
                if record.duration > self.config.slow_operation_threshold {
                    slow_count += 1;
                }

                if let Some((_, fastest_duration)) = &fastest_op {
                    if record.duration < *fastest_duration {
                        fastest_op = Some((op_name.clone(), record.duration));
                    }
                } else {
                    fastest_op = Some((op_name.clone(), record.duration));
                }

                if let Some((_, slowest_duration)) = &slowest_op {
                    if record.duration > *slowest_duration {
                        slowest_op = Some((op_name.clone(), record.duration));
                    }
                } else {
                    slowest_op = Some((op_name.clone(), record.duration));
                }
            }
        }

        // Generar performance de comandos
        let mut command_performance: Vec<CommandPerformance> = command_stats
            .iter()
            .map(|(command, stats)| CommandPerformance {
                command: command.clone(),
                executions: stats.total_executions,
                avg_duration: stats.avg_duration,
                success_rate: stats.success_rate,
                trend: PerformanceTrend::Stable, // Simplificado
            })
            .collect();

        command_performance.sort_by(|a, b| b.executions.cmp(&a.executions));

        PerformanceSummary {
            total_operations,
            avg_response_time: global_metrics.avg_operation_time,
            slow_operations_count: slow_count,
            fastest_operation: fastest_op.map(|(name, _)| name),
            slowest_operation: slowest_op.map(|(name, _)| name),
            command_performance: command_performance.into_iter().take(10).collect(),
            trend: global_metrics.performance_trend,
        }
    }

    /// Genera reporte detallado de rendimiento
    #[allow(dead_code)]
    pub async fn generate_report(&self, timeframe: Option<Duration>) -> PerformanceReport {
        let timeframe = timeframe.unwrap_or(Duration::from_secs(24 * 3600));
        let cutoff_time = Utc::now() - chrono::Duration::from_std(timeframe).unwrap();
        
        let operations = self.operations.read().await;
        let mut operation_analyses = Vec::new();
        let mut bottlenecks = Vec::new();

        // Analizar cada tipo de operación
        for (op_name, records) in operations.iter() {
            let relevant_records: Vec<_> = records
                .iter()
                .filter(|record| record.timestamp > cutoff_time)
                .collect();

            if relevant_records.is_empty() {
                continue;
            }

            let analysis = self.analyze_operation(op_name, &relevant_records);
            
            // Detectar cuellos de botella
            let bottleneck = self.detect_bottlenecks(op_name, &analysis);
            if let Some(bottleneck) = bottleneck {
                bottlenecks.push(bottleneck);
            }

            operation_analyses.push(analysis);
        }

        // Generar timeline de métricas (por hora)
        let metrics_timeline = self.generate_metrics_timeline(&operations, timeframe).await;

        // Generar recomendaciones
        let recommendations = self.generate_performance_recommendations(&operation_analyses, &bottlenecks);

        // Ordenar análisis por duración promedio (descendente)
        operation_analyses.sort_by(|a, b| b.avg_duration.cmp(&a.avg_duration));

        PerformanceReport {
            timeframe,
            operation_analysis: operation_analyses,
            performance_bottlenecks: bottlenecks,
            recommendations,
            metrics_over_time: metrics_timeline,
        }
    }

    /// Limpia registros antiguos
    pub async fn cleanup_old_entries(&self) -> u32 {
        let cutoff_time = Utc::now() - chrono::Duration::from_std(self.config.retention_period).unwrap();
        let mut total_cleaned = 0u32;

        let mut operations = self.operations.write().await;
        for records in operations.values_mut() {
            let before_len = records.len();
            records.retain(|record| record.timestamp > cutoff_time);
            total_cleaned += (before_len - records.len()) as u32;
        }

        // Remover operaciones sin registros
        operations.retain(|_, records| !records.is_empty());

        if total_cleaned > 0 {
            debug!("Limpiados {} registros de performance antiguos", total_cleaned);
            self.update_global_metrics().await;
        }

        total_cleaned
    }

    // Métodos privados

    async fn record_slow_operation(&self, operation: &str, duration: Duration, metadata: Option<HashMap<String, String>>) {
        let slow_op = SlowOperation {
            operation: operation.to_string(),
            duration,
            timestamp: Utc::now(),
            metadata,
        };

        let mut global_metrics = self.global_metrics.write().await;
        global_metrics.slowest_operations.push(slow_op);

        // Mantener solo las 50 operaciones más lentas
        global_metrics.slowest_operations.sort_by(|a, b| b.duration.cmp(&a.duration));
        global_metrics.slowest_operations.truncate(50);
    }

    async fn update_global_metrics(&self) {
        let operations = self.operations.read().await;
        let mut global_metrics = self.global_metrics.write().await;

        let total_operations: u64 = operations.values().map(|records| records.len() as u64).sum();
        
        if total_operations > 0 {
            let total_duration: Duration = operations
                .values()
                .flat_map(|records| records.iter())
                .map(|record| record.duration)
                .sum();

            global_metrics.avg_operation_time = total_duration / total_operations as u32;
        }

        global_metrics.total_operations = total_operations;
        global_metrics.last_updated = Utc::now();

        // Calcular trend simplificado
        // (En una implementación real, esto sería más sofisticado)
        if total_operations > 100 {
            global_metrics.performance_trend = PerformanceTrend::Stable;
        }
    }

    #[allow(dead_code)]
    fn analyze_operation(&self, op_name: &str, records: &[&OperationRecord]) -> OperationAnalysis {
        let mut durations: Vec<Duration> = records.iter().map(|r| r.duration).collect();
        durations.sort();

        let total_executions = records.len() as u64;
        let avg_duration = durations.iter().sum::<Duration>() / durations.len() as u32;
        
        let p95_index = (durations.len() as f64 * 0.95) as usize;
        let p99_index = (durations.len() as f64 * 0.99) as usize;
        
        let p95_duration = durations.get(p95_index).copied().unwrap_or(Duration::from_millis(0));
        let p99_duration = durations.get(p99_index).copied().unwrap_or(Duration::from_millis(0));

        let success_count = records.iter().filter(|r| r.success).count();
        let success_rate = (success_count as f64 / total_executions as f64) * 100.0;

        OperationAnalysis {
            operation: op_name.to_string(),
            total_executions,
            avg_duration,
            p95_duration,
            p99_duration,
            success_rate,
            trend: PerformanceTrend::Stable, // Simplificado
        }
    }

    #[allow(dead_code)]
    fn detect_bottlenecks(&self, op_name: &str, analysis: &OperationAnalysis) -> Option<PerformanceBottleneck> {
        // Detectar operación lenta
        if analysis.avg_duration > Duration::from_secs(3) {
            return Some(PerformanceBottleneck {
                operation: op_name.to_string(),
                issue_type: BottleneckType::SlowAverage,
                severity: if analysis.avg_duration > Duration::from_secs(10) {
                    Severity::High
                } else {
                    Severity::Medium
                },
                description: format!("La operación tiene una duración promedio de {:?}", analysis.avg_duration),
                recommendation: "Considere optimizar la lógica de esta operación".to_string(),
            });
        }

        // Detectar alta variabilidad
        let variability_ratio = analysis.p99_duration.as_millis() as f64 / analysis.avg_duration.as_millis() as f64;
        if variability_ratio > 3.0 {
            return Some(PerformanceBottleneck {
                operation: op_name.to_string(),
                issue_type: BottleneckType::HighVariability,
                severity: Severity::Medium,
                description: "La operación muestra alta variabilidad en tiempos de ejecución".to_string(),
                recommendation: "Investigue las causas de la variabilidad en el rendimiento".to_string(),
            });
        }

        // Detectar baja tasa de éxito
        if analysis.success_rate < 90.0 {
            return Some(PerformanceBottleneck {
                operation: op_name.to_string(),
                issue_type: BottleneckType::LowSuccessRate,
                severity: if analysis.success_rate < 50.0 {
                    Severity::Critical
                } else {
                    Severity::High
                },
                description: format!("La operación tiene una tasa de éxito de {:.1}%", analysis.success_rate),
                recommendation: "Revise el manejo de errores y la lógica de esta operación".to_string(),
            });
        }

        None
    }

    #[allow(dead_code)]
    async fn generate_metrics_timeline(&self, operations: &HashMap<String, VecDeque<OperationRecord>>, timeframe: Duration) -> Vec<TimeMetrics> {
        let mut timeline = Vec::new();
        let cutoff_time = Utc::now() - chrono::Duration::from_std(timeframe).unwrap();
        
        // Agrupar por horas
        let mut hourly_data: HashMap<DateTime<Utc>, (Vec<Duration>, u64, u64)> = HashMap::new();

        for records in operations.values() {
            for record in records.iter() {
                if record.timestamp > cutoff_time {
                    let hour_key = record.timestamp
                        .with_minute(0).unwrap()
                        .with_second(0).unwrap()
                        .with_nanosecond(0).unwrap();
                    
                    let entry = hourly_data.entry(hour_key).or_insert((Vec::new(), 0, 0));
                    entry.0.push(record.duration);
                    entry.1 += 1;
                    
                    if record.duration > self.config.slow_operation_threshold {
                        entry.2 += 1;
                    }
                }
            }
        }

        for (timestamp, (durations, count, slow_count)) in hourly_data {
            let avg_duration = if !durations.is_empty() {
                durations.iter().sum::<Duration>() / durations.len() as u32
            } else {
                Duration::from_millis(0)
            };

            timeline.push(TimeMetrics {
                timestamp,
                avg_duration,
                operation_count: count,
                slow_operations: slow_count,
            });
        }

        timeline.sort_by_key(|entry| entry.timestamp);
        timeline
    }

    #[allow(dead_code)]
    fn generate_performance_recommendations(&self, analyses: &[OperationAnalysis], bottlenecks: &[PerformanceBottleneck]) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Recomendaciones basadas en cuellos de botella
        let critical_bottlenecks = bottlenecks.iter().filter(|b| matches!(b.severity, Severity::Critical | Severity::High)).count();
        
        if critical_bottlenecks > 0 {
            recommendations.push(format!(
                "Se detectaron {} cuellos de botella críticos que requieren atención inmediata.",
                critical_bottlenecks
            ));
        }

        // Recomendaciones basadas en operaciones lentas
        let slow_operations: Vec<_> = analyses.iter()
            .filter(|a| a.avg_duration > Duration::from_secs(2))
            .collect();

        if !slow_operations.is_empty() {
            recommendations.push(format!(
                "Las siguientes operaciones son lentas: {}",
                slow_operations.iter()
                    .map(|a| a.operation.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        // Recomendaciones basadas en tasas de éxito
        let unreliable_operations: Vec<_> = analyses.iter()
            .filter(|a| a.success_rate < 95.0)
            .collect();

        if !unreliable_operations.is_empty() {
            recommendations.push(format!(
                "Las siguientes operaciones tienen baja confiabilidad: {}",
                unreliable_operations.iter()
                    .map(|a| format!("{} ({:.1}%)", a.operation, a.success_rate))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        if recommendations.is_empty() {
            recommendations.push("El rendimiento del sistema está dentro de parámetros normales.".to_string());
        }

        recommendations
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}