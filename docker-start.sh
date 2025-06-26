#!/bin/bash

# Script de inicio optimizado para Docker
set -e

echo "🎵 Iniciando Open Music Bot..."

# Verificar variables de entorno requeridas
if [ -z "$DISCORD_TOKEN" ]; then
    echo "❌ Error: DISCORD_TOKEN no está configurado"
    exit 1
fi

if [ -z "$APPLICATION_ID" ]; then
    echo "❌ Error: APPLICATION_ID no está configurado"
    exit 1
fi

# Crear directorios si no existen
mkdir -p /app/data /app/cache

# Mostrar configuración
echo "📊 Configuración:"
echo "  - Volume por defecto: ${DEFAULT_VOLUME:-0.5}"
echo "  - Cache size: ${CACHE_SIZE:-100}"
echo "  - Threads: ${WORKER_THREADS:-auto}"
echo "  - Ecualizador: ${ENABLE_EQUALIZER:-true}"

# Verificar conectividad de red
echo "🌐 Verificando conectividad..."
timeout 10 ping -c 1 discord.com > /dev/null 2>&1 || {
    echo "⚠️  Advertencia: No se puede conectar a Discord"
}

echo "🚀 Lanzando bot..."
exec /app/open-music "$@"