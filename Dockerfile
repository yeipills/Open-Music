# Etapa de construcción optimizada
FROM rust:1.85-alpine AS builder

# Instalar dependencias de compilación (versión optimizada)
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev \
    opus-dev \
    cmake \
    make \
    g++ \
    git \
    && rm -rf /var/cache/apk/*

WORKDIR /build

# Copiar archivos de dependencias primero para aprovechar caché
COPY Cargo.toml Cargo.lock ./

# Crear estructura dummy para compilar dependencias (linking dinámico)
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    RUSTFLAGS="-C target-feature=-crt-static" PKG_CONFIG_PATH=/usr/lib/pkgconfig cargo build --release && \
    rm -rf src target/release/deps/open_music-* target/release/open-music*

# Copiar código fuente real
COPY src ./src

# Compilar la aplicación real (linking dinámico)
RUN touch src/main.rs && \
    RUSTFLAGS="-C target-feature=-crt-static" PKG_CONFIG_PATH=/usr/lib/pkgconfig cargo build --release && \
    strip target/release/open-music

# Etapa de runtime ultra-ligera
FROM alpine:3.21

# Instalar solo dependencias de runtime (optimizado)
RUN apk add --no-cache \
    ca-certificates \
    opus \
    ffmpeg \
    python3 \
    py3-pip \
    procps \
    && pip3 install --no-cache-dir --break-system-packages yt-dlp \
    && rm -rf /var/cache/apk/* /root/.cache/pip/*

# Crear usuario no-root
RUN addgroup -g 1000 -S openmusic && \
    adduser -u 1000 -S openmusic -G openmusic

WORKDIR /app

# Copiar binario compilado
COPY --from=builder /build/target/release/open-music /app/

# Crear directorios necesarios
RUN mkdir -p /app/data /app/cache && \
    chown -R openmusic:openmusic /app

USER openmusic

# Variables de entorno para optimización y autenticación
ENV RUST_LOG=info \
    RUST_BACKTRACE=1 \
    CACHE_DIR=/app/cache \
    DATA_DIR=/app/data \
    YTDLP_OPTS="--user-agent 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36' --extractor-args 'youtube:player_client=android,web' --no-check-certificate --socket-timeout 30 --retries 3"

EXPOSE 8080
# Health check básico
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD pgrep open-music > /dev/null || exit 1

ENTRYPOINT ["/app/open-music"]