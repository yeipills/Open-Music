use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Duration,
};
use tokio::sync::RwLock;
use tracing::debug;

/// Tracker detallado de errores con categorización y análisis
#[derive(Debug)]
pub struct ErrorTracker {
    /// Errores registrados por categoría
    errors_by_category: Arc<RwLock<HashMap<String, VecDeque<ErrorEntry>>>>,
    
    /// Warnings registrados
    warnings: Arc<RwLock<VecDeque<WarningEntry>>>,
    
    /// Estadísticas de errores
    stats: Arc<RwLock<ErrorStats>>,
    
    /// Configuración
    retention_period: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEntry {
    pub timestamp: DateTime<Utc>,
    pub message: String,
    pub context: Option<HashMap<String, String>>,
    pub count: u32, // Para errores repetidos
    pub first_occurrence: DateTime<Utc>,
    pub last_occurrence: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarningEntry {
    pub timestamp: DateTime<Utc>,
    pub category: String,
    pub message: String,
    pub context: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorStats {
    pub total_errors: u64,
    pub total_warnings: u64,
    pub errors_by_category: HashMap<String, u64>,
    pub most_common_error: Option<String>,
    pub error_trend: ErrorTrend,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ErrorTrend {
    Increasing,
    Stable,
    Decreasing,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSummary {
    pub total_errors: u64,
    pub total_warnings: u64,
    pub recent_errors: u64, // Últimas 24 horas
    pub error_categories: Vec<CategorySummary>,
    pub trend: ErrorTrend,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySummary {
    pub category: String,
    pub count: u64,
    pub recent_count: u64, // Últimas 24 horas
    pub most_common_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorReport {
    pub timeframe: Duration,
    pub total_errors: u64,
    pub categories: Vec<DetailedCategoryReport>,
    pub timeline: Vec<TimelineEntry>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedCategoryReport {
    pub category: String,
    pub total_count: u64,
    pub unique_errors: u64,
    pub most_frequent: Vec<ErrorFrequency>,
    pub trend: ErrorTrend,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorFrequency {
    pub message: String,
    pub count: u32,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub timestamp: DateTime<Utc>,
    pub error_count: u32,
    pub warning_count: u32,
    pub categories: Vec<String>,
}

impl ErrorTracker {
    pub fn new(retention_period: Duration) -> Self {
        Self {
            errors_by_category: Arc::new(RwLock::new(HashMap::new())),
            warnings: Arc::new(RwLock::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(ErrorStats {
                total_errors: 0,
                total_warnings: 0,
                errors_by_category: HashMap::new(),
                most_common_error: None,
                error_trend: ErrorTrend::Unknown,
                last_updated: Utc::now(),
            })),
            retention_period,
        }
    }

    /// Registra un error con posible deduplicación
    #[allow(dead_code)]
    pub async fn record_error(&self, category: &str, message: &str, context: Option<HashMap<String, String>>) {
        let now = Utc::now();
        let mut errors = self.errors_by_category.write().await;
        let category_errors = errors.entry(category.to_string()).or_insert_with(VecDeque::new);

        // Buscar si ya existe este error
        if let Some(existing) = category_errors.iter_mut().find(|e| e.message == message) {
            // Error duplicado, incrementar contador
            existing.count += 1;
            existing.last_occurrence = now;
            debug!("Error duplicado detectado: {} (count: {})", message, existing.count);
        } else {
            // Nuevo error
            let error_entry = ErrorEntry {
                timestamp: now,
                message: message.to_string(),
                context,
                count: 1,
                first_occurrence: now,
                last_occurrence: now,
            };
            
            category_errors.push_back(error_entry);
            debug!("Nuevo error registrado en {}: {}", category, message);
        }

        // Actualizar estadísticas
        self.update_stats().await;
    }

    /// Registra un warning
    #[allow(dead_code)]
    pub async fn record_warning(&self, category: &str, message: &str, context: Option<HashMap<String, String>>) {
        let warning = WarningEntry {
            timestamp: Utc::now(),
            category: category.to_string(),
            message: message.to_string(),
            context,
        };

        let mut warnings = self.warnings.write().await;
        warnings.push_back(warning);

        // Limitar tamaño de warnings
        while warnings.len() > 10000 {
            warnings.pop_front();
        }

        self.update_stats().await;
    }

    /// Obtiene conteo de errores recientes para una categoría
    #[allow(dead_code)]
    pub async fn get_recent_error_count(&self, category: &str, timeframe: Duration) -> u32 {
        let cutoff_time = Utc::now() - chrono::Duration::from_std(timeframe).unwrap();
        let errors = self.errors_by_category.read().await;
        
        if let Some(category_errors) = errors.get(category) {
            category_errors
                .iter()
                .filter(|error| error.last_occurrence > cutoff_time)
                .map(|error| error.count)
                .sum()
        } else {
            0
        }
    }

    /// Obtiene resumen de errores
    pub async fn get_summary(&self) -> ErrorSummary {
        let stats = self.stats.read().await;
        let now = Utc::now();
        let last_24h = now - chrono::Duration::hours(24);

        // Calcular errores recientes
        let errors = self.errors_by_category.read().await;
        let recent_errors = errors
            .values()
            .flat_map(|category_errors| category_errors.iter())
            .filter(|error| error.last_occurrence > last_24h)
            .map(|error| error.count as u64)
            .sum();

        // Generar resumen por categoría
        let mut error_categories = Vec::new();
        for (category, count) in &stats.errors_by_category {
            let recent_count = if let Some(category_errors) = errors.get(category) {
                category_errors
                    .iter()
                    .filter(|error| error.last_occurrence > last_24h)
                    .map(|error| error.count as u64)
                    .sum()
            } else {
                0
            };

            let most_common_message = if let Some(category_errors) = errors.get(category) {
                category_errors
                    .iter()
                    .max_by_key(|error| error.count)
                    .map(|error| error.message.clone())
                    .unwrap_or_else(|| "No errors".to_string())
            } else {
                "No errors".to_string()
            };

            error_categories.push(CategorySummary {
                category: category.clone(),
                count: *count,
                recent_count,
                most_common_message,
            });
        }

        // Ordenar por count descendente
        error_categories.sort_by(|a, b| b.count.cmp(&a.count));

        ErrorSummary {
            total_errors: stats.total_errors,
            total_warnings: stats.total_warnings,
            recent_errors,
            error_categories,
            trend: stats.error_trend,
        }
    }

    /// Genera reporte detallado de errores
    pub async fn generate_report(&self, timeframe: Option<Duration>) -> ErrorReport {
        let timeframe = timeframe.unwrap_or(Duration::from_secs(24 * 3600)); // 24 horas por defecto
        let cutoff_time = Utc::now() - chrono::Duration::from_std(timeframe).unwrap();
        
        let errors = self.errors_by_category.read().await;
        let mut categories = Vec::new();
        let mut total_errors = 0u64;
        let mut timeline_data: HashMap<DateTime<Utc>, (u32, u32)> = HashMap::new();

        for (category_name, category_errors) in errors.iter() {
            let relevant_errors: Vec<_> = category_errors
                .iter()
                .filter(|error| error.first_occurrence > cutoff_time)
                .collect();

            if relevant_errors.is_empty() {
                continue;
            }

            let category_total: u32 = relevant_errors.iter().map(|e| e.count).sum();
            total_errors += category_total as u64;

            // Crear frecuencias
            let mut frequencies: Vec<ErrorFrequency> = relevant_errors
                .iter()
                .map(|error| ErrorFrequency {
                    message: error.message.clone(),
                    count: error.count,
                    first_seen: error.first_occurrence,
                    last_seen: error.last_occurrence,
                })
                .collect();

            frequencies.sort_by(|a, b| b.count.cmp(&a.count));

            // Calcular trend (simplificado)
            let trend = self.calculate_category_trend(category_name, timeframe).await;

            categories.push(DetailedCategoryReport {
                category: category_name.clone(),
                total_count: category_total as u64,
                unique_errors: relevant_errors.len() as u64,
                most_frequent: frequencies.into_iter().take(10).collect(),
                trend,
            });

            // Agregar a timeline
            for error in relevant_errors {
                let hour_key = error.first_occurrence
                    .with_minute(0)
                    .unwrap()
                    .with_second(0)
                    .unwrap()
                    .with_nanosecond(0)
                    .unwrap();
                
                let entry = timeline_data.entry(hour_key).or_insert((0, 0));
                entry.0 += error.count;
            }
        }

        // Procesar warnings para timeline
        let warnings = self.warnings.read().await;
        for warning in warnings.iter() {
            if warning.timestamp > cutoff_time {
                let hour_key = warning.timestamp
                    .with_minute(0)
                    .unwrap()
                    .with_second(0)
                    .unwrap()
                    .with_nanosecond(0)
                    .unwrap();
                
                let entry = timeline_data.entry(hour_key).or_insert((0, 0));
                entry.1 += 1;
            }
        }

        // Crear timeline ordenado
        let mut timeline: Vec<TimelineEntry> = timeline_data
            .into_iter()
            .map(|(timestamp, (error_count, warning_count))| TimelineEntry {
                timestamp,
                error_count,
                warning_count,
                categories: vec![], // Simplificado por ahora
            })
            .collect();

        timeline.sort_by_key(|entry| entry.timestamp);

        // Generar recomendaciones
        let recommendations = self.generate_recommendations(&categories);

        ErrorReport {
            timeframe,
            total_errors,
            categories,
            timeline,
            recommendations,
        }
    }

    /// Limpia entradas antiguas
    pub async fn cleanup_old_entries(&self) -> u32 {
        let cutoff_time = Utc::now() - chrono::Duration::from_std(self.retention_period).unwrap();
        let mut total_cleaned = 0u32;

        // Limpiar errores
        let mut errors = self.errors_by_category.write().await;
        for category_errors in errors.values_mut() {
            let before_len = category_errors.len();
            category_errors.retain(|error| error.first_occurrence > cutoff_time);
            total_cleaned += (before_len - category_errors.len()) as u32;
        }

        // Remover categorías vacías
        errors.retain(|_, category_errors| !category_errors.is_empty());

        // Limpiar warnings
        let mut warnings = self.warnings.write().await;
        let before_warnings = warnings.len();
        warnings.retain(|warning| warning.timestamp > cutoff_time);
        total_cleaned += (before_warnings - warnings.len()) as u32;

        if total_cleaned > 0 {
            debug!("Limpiadas {} entradas antiguas de errores", total_cleaned);
            self.update_stats().await;
        }

        total_cleaned
    }

    // Métodos privados

    async fn update_stats(&self) {
        let mut stats = self.stats.write().await;
        let errors = self.errors_by_category.read().await;
        let warnings = self.warnings.read().await;

        // Contar errores totales
        let mut total_errors = 0u64;
        let mut errors_by_category = HashMap::new();
        let mut most_common_error: Option<(String, u32)> = None;

        for (category, category_errors) in errors.iter() {
            let category_total: u32 = category_errors.iter().map(|e| e.count).sum();
            total_errors += category_total as u64;
            errors_by_category.insert(category.clone(), category_total as u64);

            // Encontrar error más común globalmente
            for error in category_errors.iter() {
                if let Some((_, current_max)) = &most_common_error {
                    if error.count > *current_max {
                        most_common_error = Some((error.message.clone(), error.count));
                    }
                } else {
                    most_common_error = Some((error.message.clone(), error.count));
                }
            }
        }

        // Calcular trend (simplificado)
        let trend = if stats.total_errors == 0 {
            ErrorTrend::Unknown
        } else if total_errors > stats.total_errors {
            ErrorTrend::Increasing
        } else if total_errors < stats.total_errors {
            ErrorTrend::Decreasing
        } else {
            ErrorTrend::Stable
        };

        stats.total_errors = total_errors;
        stats.total_warnings = warnings.len() as u64;
        stats.errors_by_category = errors_by_category;
        stats.most_common_error = most_common_error.map(|(msg, _)| msg);
        stats.error_trend = trend;
        stats.last_updated = Utc::now();
    }

    async fn calculate_category_trend(&self, category: &str, timeframe: Duration) -> ErrorTrend {
        let errors = self.errors_by_category.read().await;
        
        if let Some(category_errors) = errors.get(category) {
            let now = Utc::now();
            let half_timeframe = timeframe / 2;
            
            let recent_cutoff = now - chrono::Duration::from_std(half_timeframe).unwrap();
            let older_cutoff = now - chrono::Duration::from_std(timeframe).unwrap();

            let recent_count: u32 = category_errors
                .iter()
                .filter(|e| e.last_occurrence > recent_cutoff)
                .map(|e| e.count)
                .sum();

            let older_count: u32 = category_errors
                .iter()
                .filter(|e| e.last_occurrence > older_cutoff && e.last_occurrence <= recent_cutoff)
                .map(|e| e.count)
                .sum();

            if recent_count > older_count * 2 {
                ErrorTrend::Increasing
            } else if older_count > recent_count * 2 {
                ErrorTrend::Decreasing
            } else {
                ErrorTrend::Stable
            }
        } else {
            ErrorTrend::Unknown
        }
    }

    fn generate_recommendations(&self, categories: &[DetailedCategoryReport]) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Análisis de categorías con más errores
        if let Some(top_category) = categories.first() {
            if top_category.total_count > 50 {
                recommendations.push(format!(
                    "La categoría '{}' tiene {} errores. Considere revisar la lógica relacionada.",
                    top_category.category, top_category.total_count
                ));
            }
        }

        // Análisis de trends
        let increasing_categories: Vec<_> = categories
            .iter()
            .filter(|cat| matches!(cat.trend, ErrorTrend::Increasing))
            .collect();

        if !increasing_categories.is_empty() {
            recommendations.push(format!(
                "Las siguientes categorías muestran errores en aumento: {}",
                increasing_categories.iter()
                    .map(|cat| cat.category.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        // Análisis de errores únicos
        for category in categories.iter().take(3) {
            if category.unique_errors > 20 {
                recommendations.push(format!(
                    "La categoría '{}' tiene {} tipos diferentes de errores. Considere consolidar el manejo de errores.",
                    category.category, category.unique_errors
                ));
            }
        }

        if recommendations.is_empty() {
            recommendations.push("No se detectaron patrones preocupantes en los errores.".to_string());
        }

        recommendations
    }
}