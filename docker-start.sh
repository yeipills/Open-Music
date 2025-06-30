#!/bin/bash

# =====================================
# 🎵 Open Music Bot - Docker Start Script
# =====================================
# Production-ready startup script for Docker containers
# Handles initialization, health checks, and graceful shutdown

set -euo pipefail  # Exit on error, undefined vars, pipe failures

# =====================================
# 🔧 Configuration
# =====================================

# Colors for output
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly BLUE='\033[0;34m'
readonly NC='\033[0m' # No Color

# Script configuration
readonly SCRIPT_NAME="$(basename "$0")"
readonly LOG_PREFIX="[${SCRIPT_NAME}]"

# Application paths
readonly APP_BINARY="/app/open-music"
readonly DATA_DIR="${DATA_DIR:-/app/data}"
readonly CACHE_DIR="${CACHE_DIR:-/app/cache}"

# =====================================
# 📊 Logging Functions
# =====================================

log_info() {
    echo -e "${GREEN}${LOG_PREFIX} INFO:${NC} $*" >&2
}

log_warn() {
    echo -e "${YELLOW}${LOG_PREFIX} WARN:${NC} $*" >&2
}

log_error() {
    echo -e "${RED}${LOG_PREFIX} ERROR:${NC} $*" >&2
}

log_debug() {
    if [[ "${DEBUG:-false}" == "true" ]]; then
        echo -e "${BLUE}${LOG_PREFIX} DEBUG:${NC} $*" >&2
    fi
}

# =====================================
# 🔍 Health Check Functions
# =====================================

check_binary() {
    if [[ ! -f "$APP_BINARY" ]]; then
        log_error "Application binary not found: $APP_BINARY"
        return 1
    fi
    
    if [[ ! -x "$APP_BINARY" ]]; then
        log_error "Application binary is not executable: $APP_BINARY"
        return 1
    fi
    
    log_info "✅ Application binary found and executable"
    return 0
}

check_dependencies() {
    local missing_deps=()
    
    # Required system dependencies
    local deps=("yt-dlp" "ffmpeg")
    
    for dep in "${deps[@]}"; do
        if ! command -v "$dep" &> /dev/null; then
            missing_deps+=("$dep")
        fi
    done
    
    if [[ ${#missing_deps[@]} -gt 0 ]]; then
        log_error "Missing dependencies: ${missing_deps[*]}"
        log_error "Please ensure all dependencies are installed in the container"
        return 1
    fi
    
    log_info "✅ All dependencies found"
    return 0
}

check_environment() {
    local required_vars=("DISCORD_TOKEN" "APPLICATION_ID")
    local missing_vars=()
    
    for var in "${required_vars[@]}"; do
        if [[ -z "${!var:-}" ]]; then
            missing_vars+=("$var")
        fi
    done
    
    if [[ ${#missing_vars[@]} -gt 0 ]]; then
        log_error "Missing required environment variables: ${missing_vars[*]}"
        log_error "Please check your .env file or Docker environment settings"
        return 1
    fi
    
    log_info "✅ Required environment variables set"
    return 0
}

check_directories() {
    local dirs=("$DATA_DIR" "$CACHE_DIR")
    
    for dir in "${dirs[@]}"; do
        if [[ ! -d "$dir" ]]; then
            log_warn "Directory does not exist, creating: $dir"
            mkdir -p "$dir" || {
                log_error "Failed to create directory: $dir"
                return 1
            }
        fi
        
        if [[ ! -w "$dir" ]]; then
            log_error "Directory is not writable: $dir"
            return 1
        fi
    done
    
    log_info "✅ Data directories accessible"
    return 0
}

check_network() {
    log_info "🌐 Checking network connectivity..."
    
    # Check Discord API connectivity
    if ! curl -s --max-time 10 "https://discord.com/api/v10/gateway" > /dev/null; then
        log_error "Cannot reach Discord API"
        return 1
    fi
    
    # Check YouTube connectivity (for yt-dlp)
    if ! curl -s --max-time 10 "https://www.youtube.com" > /dev/null; then
        log_warn "Cannot reach YouTube (yt-dlp may not work)"
        # Don't fail on YouTube connectivity issues
    fi
    
    log_info "✅ Network connectivity verified"
    return 0
}

run_health_check() {
    log_info "🏥 Running comprehensive health check..."
    
    local checks=(
        "check_binary"
        "check_dependencies" 
        "check_environment"
        "check_directories"
        "check_network"
    )
    
    for check in "${checks[@]}"; do
        if ! "$check"; then
            log_error "Health check failed: $check"
            return 1
        fi
    done
    
    log_info "✅ All health checks passed"
    return 0
}

# =====================================
# 🚀 Application Management
# =====================================

setup_signal_handlers() {
    # Trap signals for graceful shutdown
    trap 'handle_shutdown SIGTERM' TERM
    trap 'handle_shutdown SIGINT' INT
    trap 'handle_shutdown SIGQUIT' QUIT
}

handle_shutdown() {
    local signal="$1"
    log_info "📡 Received $signal signal, initiating graceful shutdown..."
    
    if [[ -n "${APP_PID:-}" ]]; then
        log_info "🛑 Stopping application (PID: $APP_PID)..."
        kill -TERM "$APP_PID" 2>/dev/null || true
        
        # Wait for graceful shutdown
        local timeout=30
        while [[ $timeout -gt 0 ]] && kill -0 "$APP_PID" 2>/dev/null; do
            sleep 1
            ((timeout--))
        done
        
        # Force kill if still running
        if kill -0 "$APP_PID" 2>/dev/null; then
            log_warn "⚠️ Application didn't stop gracefully, force killing..."
            kill -KILL "$APP_PID" 2>/dev/null || true
        fi
        
        log_info "✅ Application stopped"
    fi
    
    cleanup_resources
    log_info "👋 Shutdown complete"
    exit 0
}

cleanup_resources() {
    log_info "🧹 Cleaning up resources..."
    
    # Clean temporary files
    find /tmp -name "openmusic-*" -type f -mtime +1 -delete 2>/dev/null || true
    
    # Clean old cache files if cache is too large
    if [[ -d "$CACHE_DIR" ]]; then
        local cache_size
        cache_size=$(du -sm "$CACHE_DIR" 2>/dev/null | cut -f1 || echo "0")
        
        if [[ $cache_size -gt 1000 ]]; then  # > 1GB
            log_warn "Cache size is ${cache_size}MB, cleaning old files..."
            find "$CACHE_DIR" -type f -mtime +7 -delete 2>/dev/null || true
        fi
    fi
    
    log_debug "Resource cleanup complete"
}

start_application() {
    log_info "🚀 Starting Open Music Bot..."
    
    # Set runtime environment
    export RUST_LOG="${RUST_LOG:-info,open_music=debug}"
    export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"
    
    # Log configuration
    log_info "📋 Configuration:"
    log_info "  - Data directory: $DATA_DIR"
    log_info "  - Cache directory: $CACHE_DIR"
    log_info "  - Log level: ${RUST_LOG}"
    log_info "  - Default volume: ${DEFAULT_VOLUME:-0.5}"
    log_info "  - Cache size: ${CACHE_SIZE:-100}"
    log_info "  - Worker threads: ${WORKER_THREADS:-auto}"
    log_info "  - Equalizer enabled: ${ENABLE_EQUALIZER:-true}"
    log_info "  - Application ID: ${APPLICATION_ID:-<not set>}"
    
    # Start the application in the background
    "$APP_BINARY" &
    APP_PID=$!
    
    log_info "✅ Application started (PID: $APP_PID)"
    
    # Wait for the application to exit
    wait "$APP_PID"
    local exit_code=$?
    
    if [[ $exit_code -eq 0 ]]; then
        log_info "✅ Application exited cleanly"
    else
        log_error "❌ Application exited with code: $exit_code"
    fi
    
    return $exit_code
}

# =====================================
# 🎯 Main Function
# =====================================

main() {
    log_info "🎵 Open Music Bot Docker Start Script"
    log_info "======================================"
    
    # Parse command line arguments
    case "${1:-start}" in
        "health-check"|"healthcheck")
            if run_health_check; then
                echo "OK"
                exit 0
            else
                echo "FAILED"
                exit 1
            fi
            ;;
        "start"|"")
            # Setup signal handlers
            setup_signal_handlers
            
            # Run health check
            if ! run_health_check; then
                log_error "🚨 Pre-flight health check failed"
                exit 1
            fi
            
            # Start application
            start_application
            exit $?
            ;;
        "version"|"--version")
            if [[ -x "$APP_BINARY" ]]; then
                "$APP_BINARY" --version || echo "Version information not available"
            else
                echo "Application binary not found"
                exit 1
            fi
            ;;
        "help"|"--help")
            cat << EOF
Open Music Bot Docker Start Script

Usage: $SCRIPT_NAME [COMMAND]

Commands:
  start, <empty>    Start the application (default)
  health-check      Run health check and exit
  version           Show version information
  help              Show this help message

Environment Variables:
  DISCORD_TOKEN     Discord bot token (required)
  APPLICATION_ID    Discord application ID (required)
  DATA_DIR          Data directory path (default: /app/data)
  CACHE_DIR         Cache directory path (default: /app/cache)
  RUST_LOG          Logging level (default: info,open_music=debug)
  DEBUG             Enable debug logging (default: false)

Examples:
  $SCRIPT_NAME                    # Start the bot
  $SCRIPT_NAME health-check       # Check system health
  $SCRIPT_NAME version            # Show version info
EOF
            ;;
        *)
            log_error "Unknown command: $1"
            log_error "Use '$SCRIPT_NAME help' for usage information"
            exit 1
            ;;
    esac
}

# =====================================
# 🏁 Script Entry Point
# =====================================

# Only run main if script is executed directly (not sourced)
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi