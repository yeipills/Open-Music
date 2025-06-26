# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is Open Music Bot - a high-performance Discord music bot built in Rust with modern architecture. The bot supports multiple audio sources (YouTube, Spotify, SoundCloud, direct URLs), advanced audio processing (equalizer, effects), and a robust queue system.

## Architecture

### Core Components

- **Audio System** (`src/audio/`): Audio player, queue management, effects processing, and equalizer
- **Bot Framework** (`src/bot/`): Discord commands, event handlers, and voice connection management
- **Source Integration** (`src/sources/`): YouTube, Spotify, SoundCloud, and direct URL handlers
- **Cache System** (`src/cache/`): LRU cache with TTL for metadata and audio data
- **Storage** (`src/storage.rs`): JSON-based persistent storage for configurations and settings
- **UI Components** (`src/ui/`): Discord embed and button builders

### Key Technologies

- **Serenity 0.12.4** + **Songbird 0.5.0** for Discord integration and voice
- **Symphonia** for audio decoding (replaces FFmpeg dependency)
- **yt-dlp** for YouTube audio extraction
- **Tokio** async runtime with optimized configuration
- **DashMap** for concurrent data structures

## Development Commands

### Build and Run
```bash
cargo build --release    # Production build with optimizations
cargo run                # Development run
```

### Testing and Quality
```bash
cargo test               # Run unit tests
cargo test --test integration  # Run integration tests
cargo clippy            # Linting
cargo fmt               # Code formatting
```

### Docker Development
```bash
docker-compose up -d     # Run in container
docker-compose logs -f   # View logs
```

## Configuration

### Environment Setup
Copy `.env.example` to `.env` and configure:
- `DISCORD_TOKEN`: Bot token (required)
- `APPLICATION_ID`: Discord application ID (required)
- `GUILD_ID`: Guild ID for development (optional)

### Audio Configuration
- `OPUS_BITRATE`: Audio quality (default: 128000)
- `DEFAULT_VOLUME`: Initial volume (default: 0.5)
- `MAX_QUEUE_SIZE`: Queue limit (default: 1000)

## Code Patterns

### Error Handling
- Use `anyhow::Result<T>` for functions that can fail
- Log errors with appropriate severity levels
- Return descriptive error messages to users

### Async Operations
- All Discord and audio operations are async
- Use `tokio::spawn` for background tasks
- Proper mutex handling for shared state

### Voice Connection Management
- Voice handlers stored in `DashMap<GuildId, Arc<Mutex<Call>>>`
- Auto-disconnect when bot is alone in channel
- Graceful cleanup on disconnection

### Audio Processing
- Symphonia for decoding (no FFmpeg dependency)
- Opus encoding for Discord voice
- Basic effects applied in real-time pipeline

## Important Implementation Details

### Command Registration
- Commands can be registered globally or per-guild
- Guild-specific registration for development (faster updates)
- Global registration for production deployment

### Queue System
- Supports shuffle, repeat modes, and seeking
- Concurrent-safe operations with proper locking
- Automatic cleanup of expired entries

### Cache Strategy
- LRU cache for metadata and audio data
- TTL-based expiration for memory management
- Separate caches for different data types

### Source Integration
- YouTube: Primary source using yt-dlp
- Direct URLs: Support for various audio formats

## Performance Considerations

- Rust 2024 edition with aggressive optimizations
- Memory usage typically 50-100MB runtime
- Supports 100+ concurrent guilds
- Audio latency < 100ms typical

## Troubleshooting

### Common Issues
- Missing system dependencies: `cmake`, `libopus-dev`, `libssl-dev`
- yt-dlp outdated: Update to latest version
- Discord permissions: Bot needs Voice permissions
- Compilation: Requires Rust 1.85+

### Debugging
- Set `RUST_LOG=debug` for verbose logging
- Check Docker health checks for container issues
- Monitor memory usage with built-in metrics