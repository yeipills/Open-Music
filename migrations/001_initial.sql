-- Migración inicial para Open Music Bot

-- Tabla de configuración por servidor
CREATE TABLE IF NOT EXISTS guild_settings (
    guild_id INTEGER PRIMARY KEY,
    default_volume REAL DEFAULT 0.5,
    announce_songs BOOLEAN DEFAULT TRUE,
    dj_role_id INTEGER,
    max_song_duration INTEGER DEFAULT 3600,
    allow_duplicates BOOLEAN DEFAULT TRUE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Tabla de presets de ecualizador personalizados
CREATE TABLE IF NOT EXISTS equalizer_presets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    band_32 REAL DEFAULT 0.0,
    band_64 REAL DEFAULT 0.0,
    band_125 REAL DEFAULT 0.0,
    band_250 REAL DEFAULT 0.0,
    band_500 REAL DEFAULT 0.0,
    band_1k REAL DEFAULT 0.0,
    band_2k REAL DEFAULT 0.0,
    band_4k REAL DEFAULT 0.0,
    band_8k REAL DEFAULT 0.0,
    band_16k REAL DEFAULT 0.0,
    created_by INTEGER NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(guild_id, name),
    FOREIGN KEY (guild_id) REFERENCES guild_settings(guild_id)
);

-- Tabla de historial de reproducción
CREATE TABLE IF NOT EXISTS playback_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    track_title TEXT NOT NULL,
    track_artist TEXT,
    track_duration INTEGER,
    source_type TEXT NOT NULL,
    played_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (guild_id) REFERENCES guild_settings(guild_id)
);

-- Tabla de playlists guardadas
CREATE TABLE IF NOT EXISTS playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    is_public BOOLEAN DEFAULT FALSE,
    created_by INTEGER NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(guild_id, name),
    FOREIGN KEY (guild_id) REFERENCES guild_settings(guild_id)
);

-- Tabla de canciones en playlists
CREATE TABLE IF NOT EXISTS playlist_tracks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL,
    position INTEGER NOT NULL,
    track_title TEXT NOT NULL,
    track_artist TEXT,
    track_url TEXT,
    track_duration INTEGER,
    source_type TEXT NOT NULL,
    added_by INTEGER NOT NULL,
    added_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE
);

-- Tabla de usuarios bloqueados (por servidor)
CREATE TABLE IF NOT EXISTS blocked_users (
    guild_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    reason TEXT,
    blocked_by INTEGER NOT NULL,
    blocked_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (guild_id, user_id),
    FOREIGN KEY (guild_id) REFERENCES guild_settings(guild_id)
);

-- Tabla de estadísticas de uso
CREATE TABLE IF NOT EXISTS usage_stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guild_id INTEGER NOT NULL,
    date DATE NOT NULL,
    total_songs_played INTEGER DEFAULT 0,
    total_duration_seconds INTEGER DEFAULT 0,
    unique_users INTEGER DEFAULT 0,
    most_played_source TEXT,
    peak_concurrent_listeners INTEGER DEFAULT 0,
    UNIQUE(guild_id, date),
    FOREIGN KEY (guild_id) REFERENCES guild_settings(guild_id)
);

-- Índices para mejorar rendimiento
CREATE INDEX idx_playback_history_guild ON playback_history(guild_id);
CREATE INDEX idx_playback_history_user ON playback_history(user_id);
CREATE INDEX idx_playback_history_date ON playback_history(played_at);
CREATE INDEX idx_playlist_tracks_playlist ON playlist_tracks(playlist_id);
CREATE INDEX idx_usage_stats_date ON usage_stats(date);

-- Trigger para actualizar updated_at
CREATE TRIGGER update_guild_settings_timestamp 
AFTER UPDATE ON guild_settings
BEGIN
    UPDATE guild_settings 
    SET updated_at = CURRENT_TIMESTAMP 
    WHERE guild_id = NEW.guild_id;
END;

CREATE TRIGGER update_playlists_timestamp 
AFTER UPDATE ON playlists
BEGIN
    UPDATE playlists 
    SET updated_at = CURRENT_TIMESTAMP 
    WHERE id = NEW.id;
END;