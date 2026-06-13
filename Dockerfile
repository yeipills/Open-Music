# Etapa de construcción (Debian para compatibilidad glibc)
FROM rust:1.85-bookworm AS builder

# Instalar dependencias de compilación
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    libopus-dev \
    cmake \
    make \
    g++ \
    git \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copiar archivos de dependencias primero para aprovechar caché
COPY Cargo.toml Cargo.lock ./

# Crear estructura dummy para compilar dependencias
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    echo "" > src/lib.rs && \
    cargo build --release && \
    rm -rf src target/release/deps/open_music-* target/release/open-music*

# Copiar código fuente real
COPY src ./src

# Compilar la aplicación real
RUN touch src/main.rs src/sources/ytdlp_optimized.rs && \
    cargo build --release && \
    strip target/release/open-music

# Etapa de runtime (Debian para compatibilidad con deno)
FROM debian:bookworm-slim

# Instalar dependencias de runtime + deno para yt-dlp
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libopus0 \
    ffmpeg \
    python3 \
    procps \
    curl \
    unzip \
    gosu \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Instalar yt-dlp directamente de GitHub (versión más reciente)
RUN curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp \
    && chmod a+rx /usr/local/bin/yt-dlp \
    && yt-dlp --version

# Instalar deno (requerido por yt-dlp 2025+ para YouTube)
RUN curl -fsSL https://github.com/denoland/deno/releases/latest/download/deno-x86_64-unknown-linux-gnu.zip -o /tmp/deno.zip \
    && unzip /tmp/deno.zip -d /usr/local/bin \
    && chmod +x /usr/local/bin/deno \
    && rm /tmp/deno.zip \
    && deno --version

# Agregar deno al PATH
ENV PATH="/usr/local/bin:${PATH}"

# Crear usuario no-root
RUN groupadd -g 1000 openmusic && \
    useradd -u 1000 -g openmusic -m openmusic

WORKDIR /app

# Copiar binario compilado
COPY --from=builder /build/target/release/open-music /app/

# Crear directorios necesarios
RUN mkdir -p /app/data /app/cache && \
    chown -R openmusic:openmusic /app

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
    '--format "bestaudio[acodec=opus]/bestaudio[ext=webm]/bestaudio/best"' \
    '--ignore-errors' \
    '--no-abort-on-error' \
    '--quiet' \
    '--no-warnings' \
    '--geo-bypass' \
    '--force-ipv4' \
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
    'exec gosu openmusic /app/open-music' > /app/start.sh && \
    chmod +x /app/start.sh

ENTRYPOINT ["/app/start.sh"]