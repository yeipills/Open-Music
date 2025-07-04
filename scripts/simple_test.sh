#!/bin/bash

# Script simple para probar yt-dlp directamente
echo "🧪 Prueba directa de yt-dlp"

TEST_URL="https://www.youtube.com/watch?v=MldGX_mbS-o"
COOKIES_FILE="$HOME/.config/yt-dlp/cookies.txt"

echo "URL de prueba: $TEST_URL"
echo ""

# Test 1: Básico
echo "Test 1: Básico"
yt-dlp --simulate --get-title "$TEST_URL"
echo ""

# Test 2: Con cookies
if [ -f "$COOKIES_FILE" ]; then
    echo "Test 2: Con cookies"
    yt-dlp --cookies "$COOKIES_FILE" --simulate --get-title "$TEST_URL"
else
    echo "Test 2: Saltado (no hay cookies)"
fi
echo ""

# Test 3: Cliente Android
echo "Test 3: Cliente Android"
yt-dlp --extractor-args 'youtube:player_client=android' --simulate --get-title "$TEST_URL"
echo ""

# Test 4: TV Embed
echo "Test 4: TV Embed"
yt-dlp --extractor-args 'youtube:player_client=tv_embed' --simulate --get-title "$TEST_URL"
echo ""

echo "✅ Pruebas completadas"