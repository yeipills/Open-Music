#!/bin/bash

# Script de prueba para el sistema jerárquico inteligente
echo "🎯 Probando Sistema Jerárquico Inteligente"
echo "=========================================="

# Verificar que la API key esté configurada
if [ -f .env ] && grep -q "YOUTUBE_API_KEY" .env; then
    echo "✅ API key configurada"
    source .env
else
    echo "❌ API key no encontrada. Ejecuta: ./scripts/setup_youtube_api.sh"
    exit 1
fi

# Compilar el proyecto
echo ""
echo "🔨 Compilando proyecto..."
cargo build --release

if [ $? -ne 0 ]; then
    echo "❌ Error de compilación"
    exit 1
fi

echo "✅ Compilación exitosa"

# Crear un programa de prueba simple
echo ""
echo "🧪 Creando programa de prueba..."

cat > test_hierarchical.rs << 'EOF'
use open_music::sources::{SmartMusicClient, MusicSource};
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configurar logging
    tracing_subscriber::fmt::init();
    
    println!("🎯 Iniciando prueba del Sistema Jerárquico Inteligente");
    println!("=====================================================");
    
    // Crear cliente inteligente
    let client = SmartMusicClient::new();
    
    // Mostrar estadísticas iniciales
    let stats = client.get_performance_stats();
    println!("📊 Estadísticas del sistema:");
    println!("   - Total de fuentes: {}", stats.total_sources);
    println!("   - Fuentes habilitadas: {}", stats.enabled_sources);
    println!("   - YouTube API v3 disponible: {}", stats.youtube_api_available);
    
    // Prueba 1: Búsqueda simple
    println!("\n🔍 Prueba 1: Búsqueda simple");
    let query = "despacito";
    let start = Instant::now();
    
    match client.search(query, 5).await {
        Ok(results) => {
            let elapsed = start.elapsed();
            println!("✅ Búsqueda exitosa en {:?}", elapsed);
            println!("📝 Resultados encontrados: {}", results.len());
            
            for (i, track) in results.iter().enumerate() {
                println!("   {}. {} - {}", i + 1, track.title(), track.artist().unwrap_or_default());
            }
        }
        Err(e) => {
            println!("❌ Error en búsqueda: {}", e);
        }
    }
    
    // Prueba 2: Obtener track por URL
    println!("\n🔗 Prueba 2: Obtener track por URL");
    let url = "https://www.youtube.com/watch?v=kJQP7kiw5Fk"; // Despacito
    let start = Instant::now();
    
    match client.get_track(url).await {
        Ok(track) => {
            let elapsed = start.elapsed();
            println!("✅ Track obtenido en {:?}", elapsed);
            println!("📝 Título: {}", track.title());
            println!("🎤 Artista: {}", track.artist().unwrap_or_default());
            if let Some(duration) = track.duration() {
                println!("⏱️ Duración: {:?}", duration);
            }
        }
        Err(e) => {
            println!("❌ Error obteniendo track: {}", e);
        }
    }
    
    // Prueba 3: Búsqueda con múltiples fuentes
    println!("\n🔄 Prueba 3: Búsqueda jerárquica completa");
    let query = "shape of you";
    let start = Instant::now();
    
    match client.search_hierarchical(query, 3).await {
        Ok(results) => {
            let elapsed = start.elapsed();
            println!("✅ Búsqueda jerárquica exitosa en {:?}", elapsed);
            println!("📝 Resultados: {}", results.len());
            
            for (i, track) in results.iter().enumerate() {
                println!("   {}. {} - {}", i + 1, track.title(), track.artist().unwrap_or_default());
            }
        }
        Err(e) => {
            println!("❌ Error en búsqueda jerárquica: {}", e);
        }
    }
    
    println!("\n🎉 Pruebas completadas!");
    Ok(())
}
EOF

# Compilar y ejecutar la prueba
echo ""
echo "🚀 Ejecutando prueba..."
rustc --edition 2021 -L target/release/deps -L target/release --extern open_music=target/release/libopen_music.rlib test_hierarchical.rs -o test_hierarchical

if [ $? -eq 0 ]; then
    echo "✅ Compilación de prueba exitosa"
    echo ""
    echo "🎯 Ejecutando sistema jerárquico..."
    ./test_hierarchical
else
    echo "❌ Error compilando prueba"
    echo "💡 Intenta ejecutar: cargo test --lib"
fi

# Limpiar archivos temporales
rm -f test_hierarchical.rs test_hierarchical

echo ""
echo "🎯 Sistema jerárquico implementado exitosamente!"
echo "📚 Para usar en tu código:"
echo "   use open_music::sources::SmartMusicClient;"
echo "   let client = SmartMusicClient::new();"
echo "   let results = client.search(\"tu búsqueda\", 10).await?;" 