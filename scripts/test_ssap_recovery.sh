#!/bin/bash

# Script para probar la recuperación de errores SSAP
# Creado por Claude Code - Julio 2025

echo "🧪 === PRUEBA DE RECUPERACIÓN SSAP ===" 
echo "Fecha: $(date)"
echo ""

TEST_URLS=(
    "https://www.youtube.com/watch?v=MldGX_mbS-o"  # MILO J - M.A.I (el que falló antes)
    "https://www.youtube.com/watch?v=dQw4w9WgXcQ"  # Rick Roll (test de control)
    "https://www.youtube.com/watch?v=kJQP7kiw5Fk"  # Luis Fonsi - Despacito
)

COOKIES_FILE="$HOME/.config/yt-dlp/cookies.txt"

echo "🔧 Configuración actual:"
echo "yt-dlp version: $(yt-dlp --version)"
echo "Cookies file: $COOKIES_FILE"
if [ -f "$COOKIES_FILE" ]; then
    echo "✅ Cookies disponibles ($(wc -l < "$COOKIES_FILE") líneas)"
else
    echo "❌ No hay cookies disponibles"
fi
echo ""

for i in "${!TEST_URLS[@]}"; do
    url="${TEST_URLS[$i]}"
    echo "🎵 === PRUEBA $((i+1)): $(basename "$url") ==="
    
    # Configurar las mismas opciones que usa el bot
    echo "Estrategia 1: Configuración principal..."
    cmd="yt-dlp"
    if [ -f "$COOKIES_FILE" ]; then
        cmd="$cmd --cookies '$COOKIES_FILE'"
    fi
    cmd="$cmd --user-agent 'Mozilla/5.0 (Linux; Android 11; SM-A515F) AppleWebKit/537.36'"
    cmd="$cmd --extractor-args 'youtube:player_client=android,tv_embed,web'"
    cmd="$cmd --extractor-args 'youtube:player_js_variant=main'"
    cmd="$cmd --extractor-args 'youtube:skip=dash,hls'"
    cmd="$cmd --no-check-certificate --socket-timeout 30 --retries 3"
    cmd="$cmd --retry-sleep 1 --fragment-retries 3"
    cmd="$cmd --http-chunk-size 5M --concurrent-fragments 1"
    cmd="$cmd --format 'bestaudio[ext=m4a]/bestaudio[ext=webm]/bestaudio'"
    cmd="$cmd --ignore-errors --no-abort-on-error --simulate --get-title '$url'"
    
    if timeout 20s eval $cmd &> /tmp/test_output_1; then
        title=$(cat /tmp/test_output_1)
        echo "✅ Éxito: $title"
        success=true
    else
        echo "❌ Falló estrategia 1"
        cat /tmp/test_output_1 | head -3
        success=false
    fi
    
    if [ "$success" = false ]; then
        echo "Estrategia 2: Cliente Android solamente..."
        cmd2="yt-dlp"
        if [ -f "$COOKIES_FILE" ]; then
            cmd2="$cmd2 --cookies '$COOKIES_FILE'"
        fi
        cmd2="$cmd2 --user-agent 'Mozilla/5.0 (compatible; Googlebot/2.1)'"
        cmd2="$cmd2 --extractor-args 'youtube:player_client=android'"
        cmd2="$cmd2 --format 'bestaudio' --simulate --get-title '$url'"
        
        if timeout 20s eval $cmd2 &> /tmp/test_output_2; then
            title=$(cat /tmp/test_output_2)
            echo "✅ Éxito con fallback: $title"
            success=true
        else
            echo "❌ Falló estrategia 2"
            head -3 /tmp/test_output_2
        fi
    fi
    
    if [ "$success" = false ]; then
        echo "Estrategia 3: TV Embed..."
        cmd3="yt-dlp"
        if [ -f "$COOKIES_FILE" ]; then
            cmd3="$cmd3 --cookies '$COOKIES_FILE'"
        fi
        cmd3="$cmd3 --user-agent 'Mozilla/5.0 (iPad; CPU OS 14_0 like Mac OS X)'"
        cmd3="$cmd3 --extractor-args 'youtube:player_client=tv_embed'"
        cmd3="$cmd3 --format 'bestaudio[ext=webm]/bestaudio' --simulate --get-title '$url'"
        
        if timeout 20s eval $cmd3 &> /tmp/test_output_3; then
            title=$(cat /tmp/test_output_3)
            echo "✅ Éxito con TV Embed: $title"
            success=true
        else
            echo "❌ Falló estrategia 3"
            head -3 /tmp/test_output_3
        fi
    fi
    
    if [ "$success" = false ]; then
        echo "🔄 Todas las estrategias yt-dlp fallaron, se usaría Invidious"
    fi
    
    echo ""
done

echo "🔍 === ANÁLISIS DE ERRORES ==="

# Buscar patrones de errores SSAP
for file in /tmp/test_output_*; do
    if [ -f "$file" ]; then
        if grep -qi "ssap\|server-side ads\|signature extraction failed\|some web client https formats have been skipped\|requested format is not available" "$file"; then
            echo "⚠️  Detectado error SSAP en $file:"
            grep -i "ssap\|server-side ads\|signature extraction failed\|some web client https formats have been skipped\|requested format is not available" "$file"
        fi
    fi
done

echo ""
echo "💡 === RECOMENDACIONES ==="
echo "1. Si persisten errores SSAP, considerar usar VPN o cambiar de IP"
echo "2. Obtener cookies reales de YouTube puede mejorar significativamente el éxito"
echo "3. El bot debería usar Invidious como fallback automático"
echo "4. Considerar implementar rotación de user-agents"

echo ""
echo "✅ Prueba de recuperación SSAP completada"

# Limpiar archivos temporales
rm -f /tmp/test_output_*