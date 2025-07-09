#!/bin/bash

# Script para configurar la API key de YouTube
echo "🔑 Configurando API key de YouTube..."

# API key proporcionada
API_KEY="AIzaSyDIHRKKkDT2wX1EMep9onWWViK1dB2yZGk"

# Verificar si existe .env
if [ ! -f .env ]; then
    echo "📝 Creando archivo .env..."
    touch .env
fi

# Agregar o actualizar la variable de entorno
if grep -q "YOUTUBE_API_KEY" .env; then
    echo "🔄 Actualizando YOUTUBE_API_KEY existente..."
    sed -i "s/YOUTUBE_API_KEY=.*/YOUTUBE_API_KEY=$API_KEY/" .env
else
    echo "➕ Agregando YOUTUBE_API_KEY..."
    echo "YOUTUBE_API_KEY=$API_KEY" >> .env
fi

echo "✅ API key configurada exitosamente!"
echo "🔍 Verificando configuración..."
grep "YOUTUBE_API_KEY" .env

echo ""
echo "🎯 Ahora puedes usar el sistema jerárquico inteligente con:"
echo "   - YouTube API v3 (más rápido y confiable)"
echo "   - Invidious (fallback sin cookies)"
echo "   - YouTube Fast (scraping optimizado)"
echo "   - YouTube Enhanced (yt-dlp con reintentos)"
echo "   - YouTube RSS (último recurso)" 