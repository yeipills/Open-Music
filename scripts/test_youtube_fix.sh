#!/bin/bash

# Script de diagnóstico para problemas de YouTube/SSAP
# Creado por Claude Code - Julio 2025

echo "=== DIAGNÓSTICO DE YOUTUBE/SSAP ISSUES ==="
echo "Fecha: $(date)"
echo ""

# 1. Verificar versión de yt-dlp
echo "Verificando yt-dlp..."
if command -v yt-dlp &> /dev/null; then
    echo "yt-dlp encontrado: $(yt-dlp --version)"
else
    echo "yt-dlp no encontrado"
    exit 1
fi

# 2. Verificar ffmpeg
echo ""
echo "Verificando ffmpeg..."
if command -v ffmpeg &> /dev/null; then
    echo "ffmpeg encontrado: $(ffmpeg -version | head -1)"
else
    echo "ffmpeg no encontrado"
    exit 1
fi

# 3. Test básico de YouTube
echo ""
echo "Probando extracción básica de YouTube..."
TEST_URL="https://www.youtube.com/watch?v=dQw4w9WgXcQ"  # Rick Roll como test

# Test 1: Verificar acceso básico
echo "Test 1: Simulación básica..."
if yt-dlp --simulate --quiet --no-warnings "$TEST_URL" &> /dev/null; then
    echo "Acceso básico OK"
else
    echo "Acceso básico falló"
fi

# Test 2: Cliente Android
echo "Test 2: Cliente Android..."
if yt-dlp --simulate --quiet --no-warnings \
    --extractor-args "youtube:player_client=android" "$TEST_URL" &> /dev/null; then
    echo "Cliente Android OK"
else
    echo "Cliente Android falló"
fi

# Test 3: Cliente TV Embed
echo "Test 3: Cliente TV Embed..."
if yt-dlp --simulate --quiet --no-warnings \
    --extractor-args "youtube:player_client=tv_embed" "$TEST_URL" &> /dev/null; then
    echo "Cliente TV Embed OK"
else
    echo "Cliente TV Embed falló"
fi

# Test 4: Detectar SSAP
echo ""
echo "Detectando experimento SSAP..."
SSAP_OUTPUT=$(yt-dlp --simulate --no-warnings "$TEST_URL" 2>&1)
if echo "$SSAP_OUTPUT" | grep -q "SSAP\|server-side ads\|Signature extraction failed"; then
    echo "SSAP detectado en la sesión actual"
    echo "    Aplicar estrategias anti-SSAP..."
else
    echo "No se detectó SSAP"
fi

# Test 5: Verificar instancias de Invidious
echo ""
echo "Verificando instancias de Invidious..."
INVIDIOUS_INSTANCES=(
    "https://yewtu.be"
    "https://inv.nadeko.net"
    "https://invidious.nerdvpn.de"
)

for instance in "${INVIDIOUS_INSTANCES[@]}"; do
    if curl -s --max-time 5 "$instance/api/v1/videos/dQw4w9WgXcQ" > /dev/null; then
        echo "$instance - OK"
    else
        echo "$instance - No responde"
    fi
done

# 6. Recomendaciones
echo ""
echo "=== RECOMENDACIONES ==="
echo "1. Mantener yt-dlp actualizado: sudo apt update && sudo apt upgrade yt-dlp"
echo "2. Si persisten errores SSAP, usar VPN o cambiar IP"
echo "3. El bot usará Invidious como fallback automático"
echo "4. Considerar rotar entre diferentes user-agents"

echo ""
echo "=== PRÓXIMOS PASOS ==="
echo "- Compilar bot con nuevas mejoras anti-SSAP"
echo "- Monitorear logs para detectar patrones de error"
echo "- Configurar alertas de monitoring para errores SSAP"

echo ""
echo "Diagnóstico completado"