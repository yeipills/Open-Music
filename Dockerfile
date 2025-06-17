# Etapa de construcción
FROM rust:1.81-alpine AS builder

# Instalar dependencias de compilación
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev \
    opus-dev \
    cmake \
    make \
    g++ \
    git

WORKDIR /build

# Copiar archivos de dependencias primero para aprovechar caché
COPY Cargo.toml Cargo.lock ./

# Crear estructura dummy para compilar dependencias
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copiar código fuente real
COPY src ./src
COPY migrations ./migrations

# Compilar la aplicación real
RUN touch src/main.rs && \
    cargo build --release && \
    strip target/release/open-music

# Etapa de runtime ultra-ligera
FROM alpine:3.20

# Instalar solo dependencias de runtime
RUN apk add --no-cache \
    ca-certificates \
    opus \
    ffmpeg \
    python3 \
    py3-pip \
    && pip3 install --no-cache-dir yt-dlp \
    && rm -rf /var/cache/apk/*

# Crear usuario no-root
RUN addgroup -g 1000 -S openmusic && \
    adduser -u 1000 -S openmusic -G openmusic

WORKDIR /app

# Copiar binario compilado
COPY --from=builder /build/target/release/open-music /app/
COPY --from=builder /build/migrations /app/migrations

# Crear directorios necesarios
RUN mkdir -p /app/data /app/cache && \
    chown -R openmusic:openmusic /app

USER openmusic

# Variables de entorno para optimización
ENV RUST_LOG=info \
    RUST_BACKTRACE=1 \
    CACHE_DIR=/app/cache \
    DATA_DIR=/app/data

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/app/open-music", "--health-check"]

ENTRYPOINT ["/app/open-music"]