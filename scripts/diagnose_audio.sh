#!/bin/bash
# Script de diagnóstico para problemas de audio - Open Music Bot

echo "DIAGNÓSTICO DE AUDIO - Open Music Bot"
echo "======================================="

echo ""
echo "1. VERIFICANDO DEPENDENCIAS..."
echo "-----------------------------------"

# Verificar yt-dlp
if command -v yt-dlp &> /dev/null; then
    echo "yt-dlp: $(yt-dlp --version)"
else
    echo "yt-dlp: NO ENCONTRADO"
fi

# Verificar ffmpeg
if command -v ffmpeg &> /dev/null; then
    echo "ffmpeg: $(ffmpeg -version | head -1)"
else
    echo "ffmpeg: NO ENCONTRADO"
fi

# Verificar opus
if ldconfig -p | grep -q opus; then
    echo "libopus: ENCONTRADO"
else
    echo "libopus: NO ENCONTRADO"
fi

echo ""
echo "2. VERIFICANDO CONTENEDOR..."
echo "------------------------------"

# Verificar si el contenedor está corriendo
if docker ps | grep -q "open-music-bot"; then
    echo "Contenedor: CORRIENDO"
    
    # Obtener logs recientes
    echo ""
    echo "Logs recientes (últimas 50 líneas):"
    docker logs --tail 50 open-music-bot
    
else
    echo "Contenedor: NO CORRIENDO"
fi

echo ""
echo "3. VERIFICANDO CONFIGURACIÓN..."
echo "---------------------------------"

# Verificar variables de entorno
echo "Variables de entorno configuradas:"
if [ -f .env ]; then
    echo "Archivo .env: EXISTE"
    grep -E "^(DISCORD_TOKEN|APPLICATION_ID|DEFAULT_VOLUME|OPUS_BITRATE)" .env | sed 's/=.*/=***/' || echo "Variables de Discord no encontradas"
else
    echo "Archivo .env: NO EXISTE"
fi

echo ""
echo "4. VERIFICANDO DIRECTORIOS..."
echo "-------------------------------"

# Verificar directorios de datos
for dir in "./data" "./cache" "./config"; do
    if [ -d "$dir" ]; then
        echo "$dir: EXISTE ($(ls -la $dir | wc -l) archivos)"
    else
        echo "$dir: NO EXISTE"
    fi
done

echo ""
echo "5. PRUEBA RÁPIDA DE YT-DLP..."
echo "------------------------------"

# Prueba rápida de extracción
echo "Probando extracción de audio de video de prueba..."
TEST_URL="https://www.youtube.com/watch?v=dQw4w9WgXcQ"
if command -v yt-dlp &> /dev/null; then
    if yt-dlp --quiet --no-warnings --simulate --format "bestaudio" "$TEST_URL" &> /dev/null; then
        echo "yt-dlp: FUNCIONA CORRECTAMENTE"
    else
        echo "yt-dlp: ERROR EN EXTRACCIÓN"
        echo "Ejecutando con verbose para debug:"
        yt-dlp --verbose --simulate --format "bestaudio" "$TEST_URL" | head -20
    fi
else
    echo "yt-dlp no disponible para prueba"
fi

echo ""
echo "6. HEALTH CHECK DEL CONTENEDOR..."
echo "----------------------------------"

if docker ps | grep -q "open-music-bot"; then
    HEALTH=$(docker inspect --format='{{.State.Health.Status}}' open-music-bot 2>/dev/null || echo "no-healthcheck")
    echo "Estado de salud: $HEALTH"
    
    if [ "$HEALTH" = "healthy" ]; then
        echo "Contenedor: SALUDABLE"
    elif [ "$HEALTH" = "unhealthy" ]; then
        echo "Contenedor: NO SALUDABLE"
    else
        echo "Sin health check configurado"
    fi
fi

echo ""
echo "RECOMENDACIONES:"
echo "=================="
echo "1. Si yt-dlp falla: Actualizar con 'pip3 install --upgrade yt-dlp'"
echo "2. Si ffmpeg falla: Instalar con 'apt-get install ffmpeg'"
echo "3. Si el contenedor no inicia: Verificar variables en .env"
echo "4. Si hay errores de permisos: Verificar docker-compose.yml"
echo "5. Para logs detallados: 'docker logs -f open-music-bot'"

echo ""
echo "Diagnóstico completado."