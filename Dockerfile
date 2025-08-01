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
    su-exec \
    curl \
    && pip3 install --no-cache-dir --break-system-packages --upgrade yt-dlp \
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

# Crear script de configuración de yt-dlp que se ejecutará en runtime
RUN printf '%s\n' \
    '#!/bin/sh' \
    'set -e' \
    '# Crear directorio con permisos correctos' \
    'mkdir -p /home/openmusic/.config/yt-dlp' \
    'chown -R openmusic:openmusic /home/openmusic/.config' \
    '# Solo crear archivos si no existen (para evitar sobrescribir volúmenes montados)' \
    'if [ ! -f /home/openmusic/.config/yt-dlp/cookies.txt ]; then' \
    '  cat > /home/openmusic/.config/yt-dlp/cookies.txt << "EOF"' \
    '# Netscape HTTP Cookie File' \
    '# Cookies anti-bot detection mejoradas - 2025-07-31' \
    '.youtube.com	TRUE	/	FALSE	1767225600	CONSENT	YES+cb.20210328-17-p0.en+FX+667' \
    '.youtube.com	TRUE	/	TRUE	1767225600	VISITOR_INFO1_LIVE	Zf3Qk8mP9R2' \
    '.youtube.com	TRUE	/	FALSE	1767225600	YSC	nB7kL4xW8mD' \
    '.youtube.com	TRUE	/	FALSE	1767225600	GPS	1' \
    '.youtube.com	TRUE	/	FALSE	1767225600	PREF	f1=50000000&f5=20000&hl=en&f6=40000000&f7=100' \
    '.google.com	TRUE	/	FALSE	1767225600	NID	511=M8kP3nR2sL9vT6qH1wE5jF8dA7cX3nP0gY2sM9kL4hB6vN8pR2sL9vT6qH1wE5jX9c' \
    '.google.com	TRUE	/	FALSE	1767225600	1P_JAR	2025-07-31-07' \
    '.youtube.com	TRUE	/	TRUE	1767225600	__Secure-1PSID	g.a000lwgK8mP3nR2sL9vT6qH1wE5jF8dA7cX3oP0gY2sM9kL4hB6vN8pR2sL9vT6qX5j' \
    '.youtube.com	TRUE	/	TRUE	1767225600	__Secure-3PSID	g.a000lwgK8mP3nR2sL9vT6qH1wE5jF8dA7cX3oP0gY2sM9kL4hB6vN8pR2sL9vT6qX5j' \
    '.youtube.com	TRUE	/	TRUE	1767225600	VISITOR_PRIVACY_METADATA	CgJVUxIEGgAgOQ%3D%3D' \
    '.youtube.com	TRUE	/	FALSE	1767225600	SOCS	CAISNwgEEhJOelV5TWprMk1EY3dPVGMwTXpBM05UZzVNdz09GLjZxrEGGPiEzLEGGICAgKCp5oOTchgBGAE' \
    '.youtube.com	TRUE	/	TRUE	1767225600	DEVICE_INFO	ChxOelV5TWprMk1EY3dPVGMwTXpBM05UZzVNdz09ELzVy7QGGLz8zLQG' \
    'EOF' \
    'fi' \
    'if [ ! -f /home/openmusic/.config/yt-dlp/config ]; then' \
    '  cat > /home/openmusic/.config/yt-dlp/config << "EOF"' \
    '--cookies ~/.config/yt-dlp/cookies.txt' \
    '--user-agent "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"' \
    '--extractor-args "youtube:player_client=android_creator,web;player_skip=dash,hls"' \
    '--add-header "Accept:text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"' \
    '--add-header "Accept-Language:en-us,en;q=0.5"' \
    '--add-header "Accept-Encoding:gzip,deflate"' \
    '--add-header "Accept-Charset:ISO-8859-1,utf-8;q=0.7,*;q=0.7"' \
    '--add-header "Connection:keep-alive"' \
    '--no-check-certificate' \
    '--socket-timeout 30' \
    '--retries 5' \
    '--retry-sleep 3' \
    '--fragment-retries 3' \
    '--http-chunk-size 10M' \
    '--concurrent-fragments 1' \
    '--format "bestaudio[ext=m4a]/bestaudio[ext=webm]/bestaudio/best[height<=720]"' \
    '--ignore-errors' \
    '--no-abort-on-error' \
    '--quiet' \
    '--no-warnings' \
    '--geo-bypass' \
    '--force-ipv4' \
    '--skip-download' \
    '--flat-playlist' \
    '--sleep-interval 1' \
    '--max-sleep-interval 3' \
    'EOF' \
    'fi' \
    '# Asegurar permisos correctos en archivos existentes' \
    'chown -R openmusic:openmusic /home/openmusic/.config/yt-dlp' \
    'chmod 644 /home/openmusic/.config/yt-dlp/cookies.txt 2>/dev/null || true' \
    'chmod 644 /home/openmusic/.config/yt-dlp/config 2>/dev/null || true' > /app/setup-yt-dlp.sh && \
    chmod +x /app/setup-yt-dlp.sh

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

# Script de inicio que maneja permisos correctamente
RUN printf '%s\n' \
    '#!/bin/sh' \
    'set -e' \
    '# Crear directorio con permisos correctos' \
    'mkdir -p /home/openmusic/.config/yt-dlp' \
    '# Corregir permisos del volumen montado si es necesario' \
    'chown -R openmusic:openmusic /home/openmusic/.config /app/data /app/cache 2>/dev/null || true' \
    '# Solo crear archivos si no existen' \
    'if [ ! -f /home/openmusic/.config/yt-dlp/cookies.txt ]; then' \
    '  cat > /home/openmusic/.config/yt-dlp/cookies.txt << "EOF"' \
    '# Netscape HTTP Cookie File' \
    '# Cookies anti-bot detection mejoradas - 2025-07-31' \
    '.youtube.com	TRUE	/	FALSE	1767225600	CONSENT	YES+cb.20210328-17-p0.en+FX+667' \
    '.youtube.com	TRUE	/	TRUE	1767225600	VISITOR_INFO1_LIVE	Zf3Qk8mP9R2' \
    '.youtube.com	TRUE	/	FALSE	1767225600	YSC	nB7kL4xW8mD' \
    '.youtube.com	TRUE	/	FALSE	1767225600	GPS	1' \
    '.youtube.com	TRUE	/	FALSE	1767225600	PREF	f1=50000000&f5=20000&hl=en&f6=40000000&f7=100' \
    '.google.com	TRUE	/	FALSE	1767225600	NID	511=M8kP3nR2sL9vT6qH1wE5jF8dA7cX3nP0gY2sM9kL4hB6vN8pR2sL9vT6qH1wE5jX9c' \
    '.google.com	TRUE	/	FALSE	1767225600	1P_JAR	2025-07-31-07' \
    '.youtube.com	TRUE	/	TRUE	1767225600	__Secure-1PSID	g.a000lwgK8mP3nR2sL9vT6qH1wE5jF8dA7cX3oP0gY2sM9kL4hB6vN8pR2sL9vT6qX5j' \
    '.youtube.com	TRUE	/	TRUE	1767225600	__Secure-3PSID	g.a000lwgK8mP3nR2sL9vT6qH1wE5jF8dA7cX3oP0gY2sM9kL4hB6vN8pR2sL9vT6qX5j' \
    '.youtube.com	TRUE	/	TRUE	1767225600	VISITOR_PRIVACY_METADATA	CgJVUxIEGgAgOQ%3D%3D' \
    '.youtube.com	TRUE	/	FALSE	1767225600	SOCS	CAISNwgEEhJOelV5TWprMk1EY3dPVGMwTXpBM05UZzVNdz09GLjZxrEGGPiEzLEGGICAgKCp5oOTchgBGAE' \
    '.youtube.com	TRUE	/	TRUE	1767225600	DEVICE_INFO	ChxOelV5TWprMk1EY3dPVGMwTXpBM05UZzVNdz09ELzVy7QGGLz8zLQG' \
    'EOF' \
    'fi' \
    'if [ ! -f /home/openmusic/.config/yt-dlp/config ]; then' \
    '  cat > /home/openmusic/.config/yt-dlp/config << "EOF"' \
    '--cookies ~/.config/yt-dlp/cookies.txt' \
    '--user-agent "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"' \
    '--extractor-args "youtube:player_client=android_creator,web;player_skip=dash,hls"' \
    '--add-header "Accept:text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"' \
    '--add-header "Accept-Language:en-us,en;q=0.5"' \
    '--add-header "Accept-Encoding:gzip,deflate"' \
    '--add-header "Accept-Charset:ISO-8859-1,utf-8;q=0.7,*;q=0.7"' \
    '--add-header "Connection:keep-alive"' \
    '--no-check-certificate' \
    '--socket-timeout 30' \
    '--retries 5' \
    '--retry-sleep 3' \
    '--fragment-retries 3' \
    '--http-chunk-size 10M' \
    '--concurrent-fragments 1' \
    '--format "bestaudio[ext=m4a]/bestaudio[ext=webm]/bestaudio/best[height<=720]"' \
    '--ignore-errors' \
    '--no-abort-on-error' \
    '--quiet' \
    '--no-warnings' \
    '--geo-bypass' \
    '--force-ipv4' \
    '--skip-download' \
    '--flat-playlist' \
    '--sleep-interval 1' \
    '--max-sleep-interval 3' \
    'EOF' \
    'fi' \
    '# Asegurar permisos correctos en archivos existentes' \
    'chown -R openmusic:openmusic /home/openmusic/.config/yt-dlp 2>/dev/null || true' \
    'chmod 755 /home/openmusic/.config/yt-dlp 2>/dev/null || true' \
    'chmod 644 /home/openmusic/.config/yt-dlp/cookies.txt 2>/dev/null || true' \
    'chmod 644 /home/openmusic/.config/yt-dlp/config 2>/dev/null || true' \
    '# Cambiar al usuario openmusic y ejecutar la aplicación' \
    'exec su-exec openmusic:openmusic /app/open-music' > /app/start.sh && \
    chmod +x /app/start.sh

ENTRYPOINT ["/app/start.sh"]