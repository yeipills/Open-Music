#[cfg(test)]
mod tests {
    use crate::monitoring::*;
    use std::time::Duration;

    #[test]
    fn test_monitoring_config_default() {
        let config = MonitoringConfig::default();
        assert_eq!(config.sample_interval, Duration::from_secs(30));
        assert_eq!(config.retention_period, Duration::from_secs(3600));
        assert!(!config.detailed_metrics);
    }

    #[test]
    fn test_monitoring_system_creation() {
        let config = MonitoringConfig::default();
        let system = MonitoringSystem::new(config);

        let metrics = system.metrics();
        assert_eq!(metrics.commands_executed, 0);
        assert_eq!(metrics.errors_count, 0);
        assert_eq!(metrics.tracks_played, 0);
        assert_eq!(metrics.guilds_active, 0);
    }

    #[test]
    fn test_monitoring_system_record_command() {
        let config = MonitoringConfig::default();
        let system = MonitoringSystem::new(config);

        system.record_command();
        system.record_command();
        system.record_command();

        let metrics = system.metrics();
        assert_eq!(metrics.commands_executed, 3);
    }

    #[test]
    fn test_monitoring_system_record_error() {
        let config = MonitoringConfig::default();
        let system = MonitoringSystem::new(config);

        system.record_error();
        system.record_error();

        let metrics = system.metrics();
        assert_eq!(metrics.errors_count, 2);
    }

    #[test]
    fn test_monitoring_system_record_track() {
        let config = MonitoringConfig::default();
        let system = MonitoringSystem::new(config);

        system.record_track_played();
        system.record_track_played();
        system.record_track_played();
        system.record_track_played();

        let metrics = system.metrics();
        assert_eq!(metrics.tracks_played, 4);
    }

    #[test]
    fn test_monitoring_system_update_guilds() {
        let config = MonitoringConfig::default();
        let system = MonitoringSystem::new(config);

        system.update_active_guilds(5);

        let metrics = system.metrics();
        assert_eq!(metrics.guilds_active, 5);

        system.update_active_guilds(10);
        let metrics = system.metrics();
        assert_eq!(metrics.guilds_active, 10);
    }

    #[test]
    fn test_metrics_error_rate() {
        let metrics = Metrics {
            uptime: Duration::from_secs(60),
            commands_executed: 100,
            errors_count: 10,
            tracks_played: 50,
            guilds_active: 5,
        };

        assert_eq!(metrics.error_rate(), 10.0);
    }

    #[test]
    fn test_metrics_error_rate_no_commands() {
        let metrics = Metrics {
            uptime: Duration::from_secs(60),
            commands_executed: 0,
            errors_count: 0,
            tracks_played: 0,
            guilds_active: 0,
        };

        assert_eq!(metrics.error_rate(), 0.0);
    }

    #[test]
    fn test_metrics_commands_per_minute() {
        let metrics = Metrics {
            uptime: Duration::from_secs(120), // 2 minutos
            commands_executed: 60,
            errors_count: 0,
            tracks_played: 0,
            guilds_active: 0,
        };

        assert_eq!(metrics.commands_per_minute(), 30.0);
    }

    #[tokio::test]
    async fn test_health_check_healthy() {
        let config = MonitoringConfig::default();
        let system = MonitoringSystem::new(config);

        // Esperar un poco
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Con poco uptime, debe estar Unknown o Healthy (sin errores)
        let status = system.perform_health_check().await;
        // Cambiado a verificar que no es Critical o Warning
        assert!(status == HealthStatus::Unknown || status == HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_health_check_warning() {
        let config = MonitoringConfig::default();
        let system = MonitoringSystem::new(config);

        // Esperar a que el sistema tenga uptime
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Generar comandos y errores para tasa de error > 10%
        for _ in 0..100 {
            system.record_command();
        }
        for _ in 0..15 {
            system.record_error();
        }

        let status = system.perform_health_check().await;
        assert_eq!(status, HealthStatus::Warning);
    }

    #[tokio::test]
    async fn test_system_metrics() {
        let config = MonitoringConfig::default();
        let system = MonitoringSystem::new(config);

        system.record_command();
        system.record_command();
        system.record_error();

        let metrics = system.get_system_metrics().await;

        assert_eq!(metrics.total_commands, 2);
        assert_eq!(metrics.total_errors, 1);
        assert!(metrics.thread_count > 0);
        assert!(metrics.error_rate > 0.0);
    }

    #[tokio::test]
    async fn test_error_report() {
        let config = MonitoringConfig::default();
        let system = MonitoringSystem::new(config);

        system.record_error();
        system.record_error();
        system.record_error();

        let report = system.get_error_report(Some(24)).await;

        assert_eq!(report.total_errors, 3);
        assert_eq!(report.categories.len(), 3);
        assert!(report.categories.iter().all(|c| c.count > 0 || report.total_errors == 0));
    }

    #[test]
    fn test_health_status_enum() {
        assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
        assert_ne!(HealthStatus::Healthy, HealthStatus::Warning);
        assert_ne!(HealthStatus::Healthy, HealthStatus::Critical);
        assert_ne!(HealthStatus::Healthy, HealthStatus::Unknown);
    }
}
