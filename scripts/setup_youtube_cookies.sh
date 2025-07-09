#!/bin/bash

# Script para configurar cookies de YouTube para yt-dlp
# Creado por Claude Code - Julio 2025

echo "🍪 === CONFIGURACIÓN DE COOKIES PARA YOUTUBE ===" 
echo "Fecha: $(date)"
echo ""

COOKIES_DIR="$HOME/.config/yt-dlp"
COOKIES_FILE="$COOKIES_DIR/cookies.txt"

# Crear directorio si no existe
mkdir -p "$COOKIES_DIR"

echo "📁 Directorio de cookies: $COOKIES_DIR"

# Verificar si ya existen cookies
if [ -f "$COOKIES_FILE" ]; then
    echo "✅ Archivo de cookies encontrado: $COOKIES_FILE"
    echo "📊 Tamaño del archivo: $(du -h "$COOKIES_FILE" | cut -f1)"
    echo "📅 Última modificación: $(stat -c %y "$COOKIES_FILE")"
else
    echo "❌ No se encontró archivo de cookies"
    echo ""
    echo "💡 === INSTRUCCIONES PARA GENERAR COOKIES ==="
    echo "1. Instalar extensión del navegador para exportar cookies:"
    echo "   - Chrome/Edge: 'Get cookies.txt LOCALLY'"
    echo "   - Firefox: 'cookies.txt'"
    echo ""
    echo "2. Ir a YouTube.com en tu navegador"
    echo "3. Iniciar sesión (opcional, pero recomendado)"
    echo "4. Usar la extensión para exportar cookies a:"
    echo "   $COOKIES_FILE"
    echo ""
    echo "5. Ejecutar este script nuevamente para verificar"
    
    # Crear archivo de cookies básico para evitar bot detection
    cat > "$COOKIES_FILE" << 'EOF'
# Netscape HTTP Cookie File
# Cookies básicas para evitar bot detection de YouTube

# Cookies esenciales para evitar detección
.youtube.com	TRUE	/	FALSE	1735689600	CONSENT	PENDING+999
.youtube.com	TRUE	/	FALSE	1735689600	VISITOR_INFO1_LIVE	fPQ4jCL6EiE
.youtube.com	TRUE	/	FALSE	1735689600	YSC	DjI2cygHYg4
.youtube.com	TRUE	/	FALSE	1735689600	GPS	1
.youtube.com	TRUE	/	FALSE	1735689600	PREF	f1=50000000&f5=20000

# Google cookies para compatibilidad
.google.com	TRUE	/	FALSE	1735689600	NID	511=example_basic_value
.google.com	TRUE	/	FALSE	1735689600	1P_JAR	2025-01-01-00

# Configuración adicional anti-bot
.youtube.com	TRUE	/	FALSE	1735689600	__Secure-1PSID	basic_session_value
.youtube.com	TRUE	/	FALSE	1735689600	SOCS	CAESEwgDEgk0NzE3NzExMjAaAmVzIAEaBgiA_LyaBg
EOF
    
    echo ""
    echo "📝 Archivo de cookies de ejemplo creado en: $COOKIES_FILE"
    echo "⚠️  IMPORTANTE: Reemplazar con cookies reales de tu navegador"
fi

echo ""
echo "🧪 === PROBANDO CONFIGURACIÓN DE COOKIES ==="

# Test con cookies
echo "Test 1: yt-dlp con cookies..."
if yt-dlp --cookies "$COOKIES_FILE" --simulate --quiet --no-warnings \
    "https://www.youtube.com/watch?v=dQw4w9WgXcQ" &> /dev/null; then
    echo "✅ Test con cookies exitoso"
else
    echo "❌ Test con cookies falló"
fi

# Test sin cookies
echo "Test 2: yt-dlp sin cookies..."
if yt-dlp --simulate --quiet --no-warnings \
    "https://www.youtube.com/watch?v=dQw4w9WgXcQ" &> /dev/null; then
    echo "✅ Test sin cookies exitoso"
else
    echo "❌ Test sin cookies falló"
fi

echo ""
echo "🔧 === CONFIGURACIÓN DE YT-DLP ==="
echo "Para usar cookies automáticamente, agregar a variables de entorno:"
echo "export YTDLP_OPTS=\"--cookies $COOKIES_FILE \$YTDLP_OPTS\""
echo ""
echo "O modificar el código para incluir: --cookies '$COOKIES_FILE'"

echo ""
echo "✅ Configuración de cookies completada"