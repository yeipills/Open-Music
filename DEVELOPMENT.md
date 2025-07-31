# 🛠️ Development Guide
**Open Music Bot - Discord Music Bot in Rust**

*Comprehensive development documentation for contributors and maintainers*

---

## 📋 Project Overview

**Open Music Bot** is a high-performance Discord music bot built with modern Rust architecture. Features include YouTube integration, direct URL support, advanced audio processing with equalizer, and a robust queue management system.

### 🎯 Key Features
- **Audio Processing**: Real-time equalizer with 8 presets
- **Multiple Sources**: YouTube (yt-dlp) + Direct URLs
- **Queue Management**: Advanced controls with shuffle/repeat
- **Performance**: <100MB RAM, supports 100+ concurrent servers
- **Modern Stack**: Serenity 0.12.4 + Songbird 0.5.0

## 🏗️ Architecture

### 📦 Core Components

| Module | Location | Purpose | Key Files |
|--------|----------|---------|----------|
| **🎵 Audio System** | `src/audio/` | Player, queue, effects, equalizer | `player.rs`, `queue.rs`, `effects.rs` |
| **🤖 Bot Framework** | `src/bot/` | Commands, events, voice connections | `commands.rs`, `handlers.rs`, `events.rs` |
| **📡 Source Integration** | `src/sources/` | YouTube and URL handlers | `youtube.rs`, `direct_url.rs` |
| **💾 Cache System** | `src/cache/` | LRU cache with TTL | `lru_cache.rs`, `optimized_cache.rs` |
| **🗄️ Storage** | `src/storage.rs` | JSON persistent storage | Configuration, settings |
| **🎨 UI Components** | `src/ui/` | Discord embeds and buttons | `embeds.rs`, `buttons.rs` |
| **📊 Monitoring** | `src/monitoring/` | Performance tracking | `performance_monitor.rs` |

### 🔧 Technology Stack

| Component | Technology | Version | Purpose |
|-----------|------------|---------|--------|
| **Discord API** | Serenity | 0.12.4 | Bot framework |
| **Voice/Audio** | Songbird | 0.5.0 | Voice connections |
| **Audio Decode** | Symphonia | 0.5.4 | Format support (no FFmpeg) |
| **YouTube DL** | yt-dlp | Latest | Video extraction |
| **Async Runtime** | Tokio | 1.45 | High-performance async |
| **Concurrency** | DashMap | 6.1 | Concurrent data structures |
| **Serialization** | Serde | 1.0 | JSON handling |

## 🚀 Development Commands

### 🔨 Build & Run
```bash
# Development
cargo build                    # Debug build
cargo run                      # Run with debug info
cargo run -- --health-check   # Health check mode

# Production
cargo build --release          # Optimized build
STRIP=true cargo build --release  # Stripped binary (~15MB)

# Cross-compilation
cargo build --target x86_64-unknown-linux-musl --release
```

### 🧪 Testing & Quality
```bash
# Testing
cargo test                     # Unit tests
cargo test --test integration  # Integration tests  
cargo test -- --nocapture      # Tests with output
cargo test --release           # Optimized test run

# Code Quality
cargo clippy                   # Linter
cargo clippy -- -D warnings    # Treat warnings as errors
cargo fmt                      # Format code
cargo fmt -- --check           # Verify formatting

# Analysis
cargo audit                    # Security audit
cargo outdated                 # Check for updates
cargo tree                     # Dependency tree
```

### 🐳 Docker Development
```bash
# Container Management
docker-compose up -d           # Start services
docker-compose logs -f         # Follow logs
docker-compose restart         # Restart services
docker-compose down            # Stop and remove

# Development Workflow
docker-compose up --build      # Rebuild and start
docker-compose exec open-music sh  # Access container
docker stats open-music-bot    # Resource usage

# Cleanup
docker system prune            # Clean unused resources
docker builder prune           # Clean build cache
```

## ⚙️ Configuration

### 🌍 Environment Setup

**1. Copy environment template**
```bash
cp .env.example .env
```

**2. Configure required variables**
```env
# Discord (Required)
DISCORD_TOKEN=your_bot_token_here
APPLICATION_ID=your_application_id_here

# Development (Optional)
GUILD_ID=your_test_server_id  # For faster command updates
```

### 🎵 Audio Configuration

| Variable | Default | Range | Description |
|----------|---------|-------|-------------|
| `DEFAULT_VOLUME` | 0.5 | 0.0-2.0 | Starting volume (50%) |
| `OPUS_BITRATE` | 128000 | 64000-510000 | Audio quality (128kbps) |
| `FRAME_SIZE` | 960 | 120-2880 | Frame size (20ms @ 48kHz) |
| `MAX_SONG_DURATION` | 7200 | 1-43200 | Max song length (2 hours) |

### 🚀 Performance Tuning

| Variable | Default | Description |
|----------|---------|-------------|
| `CACHE_SIZE` | 100 | Metadata cache entries |
| `AUDIO_CACHE_SIZE` | 50 | Audio file cache entries |
| `MAX_QUEUE_SIZE` | 1000 | Maximum queue length |
| `WORKER_THREADS` | Auto | Worker thread count |
| `RATE_LIMIT_PER_USER` | 20 | Commands/minute per user |

### 🔧 Development-Specific Settings

```env
# Detailed Logging
RUST_LOG=debug,serenity=info,songbird=debug
RUST_BACKTRACE=full

# Development Paths (if not using Docker)
DATA_DIR=./data
CACHE_DIR=./cache

# Feature Flags
ENABLE_EQUALIZER=true
ENABLE_AUTOPLAY=false
```

## 📝 Code Patterns & Guidelines

### 🚨 Error Handling

```rust
// ✅ Good: Use anyhow::Result for fallible operations
pub async fn play_song(url: &str) -> anyhow::Result<()> {
    let source = extract_audio(url)
        .await
        .with_context(|| format!("Failed to extract: {}", url))?;
    Ok(())
}

// ✅ Good: Log errors with context
if let Err(e) = play_song(&url).await {
    tracing::error!("Playback failed: {:?}", e);
    interaction.reply("❌ Could not play song").await?;
}
```

### ⚡ Async Operations

```rust
// ✅ Good: Spawn background tasks for non-blocking operations
tokio::spawn(async move {
    if let Err(e) = download_audio(url).await {
        tracing::warn!("Background download failed: {:?}", e);
    }
});

// ✅ Good: Proper mutex handling
let mut queue = self.queue.lock().await;
queue.add_song(song);
drop(queue); // Explicit early release
```

### 🎤 Voice Connection Management

```rust
// Voice connections are stored per guild
type VoiceConnections = DashMap<GuildId, Arc<Mutex<Call>>>;

// ✅ Auto-disconnect pattern
if call.current_channel().is_none() {
    tracing::info!("Bot alone in channel, disconnecting");
    let _ = call.leave().await;
    voice_connections.remove(&guild_id);
}
```

### 🎵 Audio Processing Pipeline

```rust
// Symphonia decode -> Opus encode -> Discord
let decoded = symphonia_decoder.decode(packet)?;
let opus_frame = opus_encoder.encode(&decoded)?;
voice_connection.send_audio(opus_frame).await?;

// Real-time equalizer application
let eq_output = equalizer.process(&audio_samples, &preset);
```

## 🔧 Implementation Details

### 📝 Command Registration

```rust
// Development: Guild-specific (fast updates)
if let Some(guild_id) = config.guild_id {
    GuildId(guild_id).set_application_commands(&ctx, commands).await?;
}

// Production: Global (slower propagation)
Command::set_global_application_commands(&ctx, commands).await?;
```

**Registration Timing:**
- Guild commands: ~1 second propagation
- Global commands: ~1 hour propagation
- Use guild commands for development/testing

### 🎵 Queue System Architecture

```rust
pub struct MusicQueue {
    tracks: VecDeque<Track>,
    current: Option<Track>,
    repeat_mode: RepeatMode,
    shuffle: bool,
    history: VecDeque<Track>,
}

// Thread-safe operations
impl MusicQueue {
    pub async fn add(&mut self, track: Track) -> Result<()> {
        if self.tracks.len() >= self.max_size {
            return Err(anyhow!("Queue is full"));
        }
        self.tracks.push_back(track);
        Ok(())
    }
}
```

### 💾 Cache Strategy

```rust
// Multi-layered caching approach
pub struct MusicCache {
    metadata: LruCache<String, TrackMetadata>,    // Song info
    audio_data: LruCache<String, Vec<u8>>,       // Decoded audio
    thumbnails: LruCache<String, Vec<u8>>,       // Album artwork
}

// TTL-based expiration
if let Some(cached) = cache.get_with_expiry(&key) {
    if !cached.is_expired() {
        return Ok(cached.data);
    }
}
```

### 📡 Source Integration

**YouTube Integration (yt-dlp)**
```rust
let output = Command::new("yt-dlp")
    .args(&["-f", "bestaudio", "--get-url", url])
    .output()
    .await?;
    
let audio_url = String::from_utf8(output.stdout)?
    .trim()
    .to_string();
```

**Direct URL Support**
```rust
// Supported formats via Symphonia
const SUPPORTED_FORMATS: &[&str] = &[
    "mp3", "wav", "flac", "ogg", "aac", "m4a"
];
```

## 📊 Performance Considerations

### 🎯 Performance Targets

| Metric | Target | Typical | Notes |
|--------|--------|---------|-------|
| **Memory Usage** | <100MB | 50-80MB | Includes cache |
| **CPU Usage (Idle)** | <5% | 1-2% | Single core |
| **CPU Usage (Playing)** | <20% | 10-15% | Per concurrent stream |
| **Audio Latency** | <100ms | 50-80ms | End-to-end |
| **Command Response** | <500ms | 100-200ms | Slash commands |
| **Concurrent Guilds** | 100+ | 50+ | Production tested |

### ⚡ Optimization Strategies

**Cargo Profile Optimizations**
```toml
[profile.release]
opt-level = 3           # Maximum optimization
lto = true             # Link-time optimization
codegen-units = 1      # Single codegen unit
strip = true           # Strip debug symbols
panic = "abort"        # Smaller binary size
```

**Memory Management**
```rust
// Use bounded channels to prevent memory leaks
let (tx, rx) = flume::bounded(100);

// Pool expensive resources
static OPUS_ENCODER: Lazy<OpusEncoder> = Lazy::new(|| {
    OpusEncoder::new(48000, Channels::Stereo, Application::Audio)
        .expect("Failed to create Opus encoder")
});
```

**Async Performance**
```rust
// Use spawn_blocking for CPU-intensive tasks
let decoded = tokio::task::spawn_blocking(move || {
    symphonia_decode(audio_data)
}).await??;

// Batch operations when possible
let results = futures::future::join_all(
    urls.iter().map(|url| extract_metadata(url))
).await;
```

## 🔍 Troubleshooting & Debugging

### ❌ Common Issues

| Issue | Cause | Solution |
|-------|-------|----------|
| **Compilation fails** | Missing system deps | `apt install cmake libopus-dev libssl-dev pkg-config` |
| **yt-dlp not working** | Outdated version | `pip3 install -U yt-dlp` |
| **No audio playback** | Missing voice perms | Check bot permissions in Discord |
| **High memory usage** | Cache too large | Reduce `CACHE_SIZE` and `AUDIO_CACHE_SIZE` |
| **Slow responses** | Limited resources | Increase Docker memory limits |
| **Connection timeouts** | Network issues | Check firewall/proxy settings |

### 🐛 Debugging Tools

**Logging Configuration**
```bash
# Detailed logging
export RUST_LOG="debug,serenity=info,songbird=debug,hyper=warn"
export RUST_BACKTRACE=full

# Component-specific debugging
export RUST_LOG="open_music::audio=trace,open_music::bot=debug"
```

**Performance Monitoring**
```bash
# Memory profiling
valgrind --tool=memcheck ./target/release/open-music

# CPU profiling  
perf record -g ./target/release/open-music
perf report

# Real-time monitoring
top -p $(pgrep open-music)
htop -p $(pgrep open-music)
```

**Health Checks**
```bash
# Built-in health check
./target/release/open-music --health-check

# Docker container health
docker-compose exec open-music /app/open-music --health-check

# Dependency verification
yt-dlp --version
ffmpeg -version
opus_demo --help
```

### 📋 Development Checklist

**Before Committing:**
- [ ] `cargo fmt` - Code formatting
- [ ] `cargo clippy` - Linter warnings
- [ ] `cargo test` - All tests pass
- [ ] `cargo audit` - Security vulnerabilities
- [ ] Update documentation if applicable
- [ ] Test with sample Discord server

**Before Release:**
- [ ] Update version in `Cargo.toml`
- [ ] Build release binary: `cargo build --release`
- [ ] Test Docker build: `docker-compose build`
- [ ] Update documentation
- [ ] Create git tag: `git tag v1.x.x`

---

## 📚 Additional Resources

- **[Serenity Documentation](https://docs.rs/serenity/)**
- **[Songbird Guide](https://github.com/serenity-rs/songbird)**
- **[Discord Developer Portal](https://discord.com/developers/applications)**
- **[Rust Async Book](https://rust-lang.github.io/async-book/)**
- **[Tokio Tutorial](https://tokio.rs/tokio/tutorial)**

**Project Links:**
- Issues: Crear issues en el repositorio para bugs y mejoras
- Discussions: Usar issues para discusiones técnicas
- Documentation: Ver README.md y archivos .md del proyecto