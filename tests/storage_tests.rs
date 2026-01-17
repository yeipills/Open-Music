//! Tests for storage module

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use std::time::Duration;

    /// Simplified ServerConfig for testing serialization
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestServerConfig {
        guild_id: u64,
        default_volume: f32,
        max_queue_per_user: usize,
        dj_role: Option<u64>,
        announcement_channel: Option<u64>,
        prefix: String,
        auto_disconnect_timeout: u64,
    }

    impl Default for TestServerConfig {
        fn default() -> Self {
            Self {
                guild_id: 0,
                default_volume: 0.7,
                max_queue_per_user: 10,
                dj_role: None,
                announcement_channel: None,
                prefix: "!".to_string(),
                auto_disconnect_timeout: 600,
            }
        }
    }

    #[test]
    fn test_server_config_default() {
        let config = TestServerConfig::default();
        
        assert_eq!(config.default_volume, 0.7);
        assert_eq!(config.max_queue_per_user, 10);
        assert!(config.dj_role.is_none());
        assert!(config.announcement_channel.is_none());
        assert_eq!(config.prefix, "!");
        assert_eq!(config.auto_disconnect_timeout, 600);
    }

    #[test]
    fn test_server_config_serialization() {
        let config = TestServerConfig {
            guild_id: 123456789,
            default_volume: 0.8,
            max_queue_per_user: 15,
            dj_role: Some(987654321),
            announcement_channel: None,
            prefix: "/".to_string(),
            auto_disconnect_timeout: 300,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: TestServerConfig = serde_json::from_str(&json).unwrap();
        
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_server_config_json_format() {
        let config = TestServerConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        
        assert!(json.contains("guild_id"));
        assert!(json.contains("default_volume"));
        assert!(json.contains("max_queue_per_user"));
    }

    /// Playlist track for testing
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestPlaylistTrack {
        id: String,
        title: String,
        artist: String,
        duration_secs: u64,
        url: String,
        added_by: u64,
    }

    #[test]
    fn test_playlist_track_serialization() {
        let track = TestPlaylistTrack {
            id: "abc123".to_string(),
            title: "Test Song".to_string(),
            artist: "Test Artist".to_string(),
            duration_secs: 180,
            url: "https://example.com/track".to_string(),
            added_by: 123456789,
        };

        let json = serde_json::to_string(&track).unwrap();
        let deserialized: TestPlaylistTrack = serde_json::from_str(&json).unwrap();
        
        assert_eq!(track, deserialized);
    }

    #[test]
    fn test_duration_conversion() {
        let duration_secs: u64 = 185; // 3:05
        let duration = Duration::from_secs(duration_secs);
        
        assert_eq!(duration.as_secs(), 185);
        
        let minutes = duration.as_secs() / 60;
        let seconds = duration.as_secs() % 60;
        
        assert_eq!(minutes, 3);
        assert_eq!(seconds, 5);
    }

    #[test]
    fn test_volume_normalization() {
        // Test volume clamping logic
        let test_values = vec![
            (0, 0.0),
            (50, 0.5),
            (100, 1.0),
            (150, 1.5),
            (200, 2.0),
            (250, 2.0), // Should clamp to 2.0
        ];

        for (input, expected) in test_values {
            let normalized = (input as f32 / 100.0).clamp(0.0, 2.0);
            assert_eq!(normalized, expected, "Input {} should normalize to {}", input, expected);
        }
    }

    #[test]
    fn test_queue_size_limits() {
        let max_queue_size: usize = 1000;
        let current_size: usize = 999;
        
        assert!(current_size < max_queue_size);
        assert!(current_size + 1 <= max_queue_size);
        assert!(current_size + 2 > max_queue_size);
    }

    /// Test JSON file path generation
    #[test]
    fn test_file_path_generation() {
        let data_dir = std::path::PathBuf::from("/app/data");
        let guild_id: u64 = 123456789;
        
        let server_path = data_dir.join("servers").join(format!("{}.json", guild_id));
        
        assert_eq!(
            server_path.to_string_lossy(),
            "/app/data/servers/123456789.json"
        );
    }

    #[test]
    fn test_playlist_file_path() {
        let data_dir = std::path::PathBuf::from("/app/data");
        let user_id: u64 = 987654321;
        let playlist_name = "my_playlist";
        
        let playlist_path = data_dir
            .join("playlists")
            .join(user_id.to_string())
            .join(format!("{}.json", playlist_name));
        
        assert_eq!(
            playlist_path.to_string_lossy(),
            "/app/data/playlists/987654321/my_playlist.json"
        );
    }
}
