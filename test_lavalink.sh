#!/bin/bash

# Script para probar la funcionalidad de Lavalink

echo "🔍 Verificando servicios Docker..."
docker compose ps

echo ""
echo "🎼 Probando conectividad con Lavalink..."
curl -s "http://localhost:2333/version" && echo " ✅ Lavalink conectando"

echo ""
echo "🔍 Probando búsqueda con Lavalink API..."
curl -H "Authorization: youshallnotpass" \
     "http://localhost:2333/v4/loadtracks?identifier=ytsearch:milo%20j" \
     | jq '.loadType' 2>/dev/null && echo " ✅ API de búsqueda funcional"

echo ""
echo "📊 Estado de los logs del bot (últimas 10 líneas)..."
docker compose logs open-music --tail=10

echo ""
echo "✅ Test completado. Si Lavalink muestra resultados, el sistema debería funcionar correctamente."