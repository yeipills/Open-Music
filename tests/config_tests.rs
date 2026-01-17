//! Tests for configuration module

#[cfg(test)]
mod tests {

    #[test]
    fn test_config_default_values() {
        // Test that default config has sensible values
        let config = open_music::config::Config::default();
        
        assert_eq!(config.default_volume, 0.5);
        assert_eq!(config.opus_bitrate, 96000);
        assert_eq!(config.frame_size, 960);
        assert_eq!(config.cache_size, 100);
        assert_eq!(config.max_queue_size, 1000);
        assert_eq!(config.max_song_duration, 7200);
        assert!(config.enable_equalizer);
        assert!(!config.enable_autoplay);
    }

    #[test]
    fn test_config_validation_valid() {
        let mut config = open_music::config::Config::default();
        config.default_volume = 1.0;
        config.opus_bitrate = 128000;
        config.cache_size = 50;
        
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_volume_too_high() {
        let mut config = open_music::config::Config::default();
        config.default_volume = 3.0; // Above 2.0 limit
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("volume"));
    }

    #[test]
    fn test_config_validation_volume_negative() {
        let mut config = open_music::config::Config::default();
        config.default_volume = -0.5;
        
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_config_validation_bitrate_too_high() {
        let mut config = open_music::config::Config::default();
        config.opus_bitrate = 600000; // Above 510000 limit
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("bitrate"));
    }

    #[test]
    fn test_config_validation_bitrate_too_low() {
        let mut config = open_music::config::Config::default();
        config.opus_bitrate = 1000; // Below 8000 minimum
        
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_config_validation_zero_cache() {
        let mut config = open_music::config::Config::default();
        config.cache_size = 0;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cache size"));
    }

    #[test]
    fn test_config_validation_zero_queue() {
        let mut config = open_music::config::Config::default();
        config.max_queue_size = 0;
        
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_config_summary_format() {
        let config = open_music::config::Config::default();
        let summary = config.summary();
        
        assert!(summary.contains("Config Summary"));
        assert!(summary.contains("Audio"));
        assert!(summary.contains("Cache"));
        assert!(summary.contains("Limits"));
    }
}
