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

# Crear directorios necesarios incluyendo yt-dlp config
RUN mkdir -p /app/data /app/cache /home/openmusic/.config/yt-dlp && \
    chown -R openmusic:openmusic /app /home/openmusic/.config

# Configurar cookies básicas para evitar bot detection
RUN printf '%s\n' \
    '# Netscape HTTP Cookie File' \
    '# Cookies básicas para evitar bot detection de YouTube' \
    '.youtube.com	TRUE	/	FALSE	1735689600	CONSENT	PENDING+999' \
    '.youtube.com	TRUE	/	FALSE	1735689600	VISITOR_INFO1_LIVE	fPQ4jCL6EiE' \
    '.youtube.com	TRUE	/	FALSE	1735689600	YSC	DjI2cygHYg4' \
    '.youtube.com	TRUE	/	FALSE	1735689600	GPS	1' \
    '.youtube.com	TRUE	/	FALSE	1735689600	PREF	f1=50000000&f5=20000' \
    '.google.com	TRUE	/	FALSE	1735689600	NID	511=example_basic_value' \
    '.google.com	TRUE	/	FALSE	1735689600	1P_JAR	2025-01-01-00' \
    '.youtube.com	TRUE	/	FALSE	1735689600	__Secure-1PSID	basic_session_value' \
    '.youtube.com	TRUE	/	FALSE	1735689600	SOCS	CAESEwgDEgk0NzE3NzExMjAaAmVzIAEaBgiA_LyaBg' > /home/openmusic/.config/yt-dlp/cookies.txt

# Configurar archivo de configuración de yt-dlp
RUN printf '%s\n' \
    '--cookies ~/.config/yt-dlp/cookies.txt' \
    '--user-agent "Mozilla/5.0 (Linux; Android 11; SM-A515F) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36"' \
    '--extractor-args "youtube:player_client=android_embedded,android_creator,tv_embed"' \
    '--extractor-args "youtube:player_js_variant=main"' \
    '--extractor-args "youtube:skip=dash,hls"' \
    '--no-check-certificate' \
    '--socket-timeout 30' \
    '--retries 3' \
    '--retry-sleep 1' \
    '--fragment-retries 3' \
    '--http-chunk-size 5M' \
    '--concurrent-fragments 1' \
    '--format "bestaudio[ext=m4a]/bestaudio[ext=webm]/bestaudio/best[height<=720]/best"' \
    '--ignore-errors' \
    '--no-abort-on-error' \
    '--quiet' \
    '--no-warnings' > /home/openmusic/.config/yt-dlp/config

# Ajustar permisos
RUN chown -R openmusic:openmusic /home/openmusic/.config && \
    chmod 600 /home/openmusic/.config/yt-dlp/cookies.txt && \
    chmod 644 /home/openmusic/.config/yt-dlp/config

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