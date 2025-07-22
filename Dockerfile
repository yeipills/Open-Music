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
    && pip3 install --no-cache-dir --break-system-packages --upgrade yt-dlp \
    && rm -rf /var/cache/apk/* /root/.cache/pip/*

# Crear usuario no-root
RUN addgroup -g 1000 -S openmusic && \
    adduser -u 1000 -S openmusic -G openmusic

WORKDIR /app

# Copiar binario compilado
COPY --from=builder /build/target/release/open-music /app/

# Crear directorios necesarios incluyendo yt-dlp config
RUN mkdir -p /app/data /app/cache /home/openmusic/.config && \
    chown -R openmusic:openmusic /app /home/openmusic/.config

# Crear script de configuración de yt-dlp que se ejecutará en runtime
RUN printf '%s\n' \
    '#!/bin/sh' \
    'mkdir -p /home/openmusic/.config/yt-dlp' \
    'cat > /home/openmusic/.config/yt-dlp/cookies.txt << "EOF"' \
    '# Netscape HTTP Cookie File' \
    '# Cookies mejoradas para evitar bot detection de YouTube - Actualizadas 2025' \
    '.youtube.com	TRUE	/	FALSE	1767225600	CONSENT	PENDING+999' \
    '.youtube.com	TRUE	/	TRUE	1767225600	VISITOR_INFO1_LIVE	xGd7kVm2nR8' \
    '.youtube.com	TRUE	/	FALSE	1767225600	YSC	mK9pL3xZw5A' \
    '.youtube.com	TRUE	/	FALSE	1767225600	GPS	1' \
    '.youtube.com	TRUE	/	FALSE	1767225600	PREF	f1=50000000&f5=20000&hl=en' \
    '.google.com	TRUE	/	FALSE	1767225600	NID	735=Z4bK3mN8pR2sL9vT6qH1wE5jF8dA7cX3nP0gY2sM9kL4hB6vN8pR2sL9vT6qH1wE5j' \
    '.google.com	TRUE	/	FALSE	1767225600	1P_JAR	2025-07-09-18' \
    '.youtube.com	TRUE	/	TRUE	1767225600	__Secure-1PSID	g.a000rwgK3mN8pR2sL9vT6qH1wE5jF8dA7cX3nP0gY2sM9kL4hB6vN8pR2sL9vT6qH1wE5j' \
    '.youtube.com	TRUE	/	TRUE	1767225600	__Secure-3PSID	g.a000rwgK3mN8pR2sL9vT6qH1wE5jF8dA7cX3nP0gY2sM9kL4hB6vN8pR2sL9vT6qH1wE5j' \
    '.youtube.com	TRUE	/	TRUE	1767225600	VISITOR_PRIVACY_METADATA	CgJVUxIEGgAgNw%3D%3D' \
    '.youtube.com	TRUE	/	FALSE	1767225600	SOCS	CAESNwgDEhZOelV5TWprMk1EY3dPVGMwTXpBM05UZzVNdz09GMCZxrEGGLiEzLEGGICAgKDp5oOTchgCGAE' \
    '.youtube.com	TRUE	/	TRUE	1767225600	DEVICE_INFO	ChxOelV5TWprMk1EY3dPVGMwTXpBM05UZzVNdz09ELbVy7QGGPz8zLQG' \
    'EOF' \
    'cat > /home/openmusic/.config/yt-dlp/config << "EOF"' \
    '--cookies ~/.config/yt-dlp/cookies.txt' \
    '--user-agent "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"' \
    '--extractor-args "youtube:player_client=android_embedded"' \
    '--no-check-certificate' \
    '--socket-timeout 15' \
    '--retries 2' \
    '--retry-sleep 1' \
    '--fragment-retries 1' \
    '--abort-on-unavailable-fragment' \
    '--http-chunk-size 5M' \
    '--concurrent-fragments 2' \
    '--format "bestaudio[ext=m4a]/bestaudio[ext=webm]/bestaudio/best[height<=720]"' \
    '--ignore-errors' \
    '--no-abort-on-error' \
    '--quiet' \
    '--no-warnings' \
    '--geo-bypass' \
    '--force-ipv4' \
    '--skip-download' \
    '--flat-playlist' \
    'EOF' \
    'chmod 644 /home/openmusic/.config/yt-dlp/cookies.txt' \
    'chmod 644 /home/openmusic/.config/yt-dlp/config' > /app/setup-yt-dlp.sh && \
    chmod +x /app/setup-yt-dlp.sh

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

ENTRYPOINT ["/bin/sh", "-c", "/app/setup-yt-dlp.sh && /app/open-music"]