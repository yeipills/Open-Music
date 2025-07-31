# 🤝 Guía de Contribución - Open Music Bot

¡Gracias por tu interés en contribuir a Open Music Bot! Este documento te guiará para hacer contribuciones efectivas al proyecto.

## 📋 Tabla de Contenidos

1. [🚀 Inicio Rápido](#-inicio-rápido)
2. [🛠️ Configuración del Entorno](#️-configuración-del-entorno)
3. [📝 Proceso de Contribución](#-proceso-de-contribución)
4. [🎯 Tipos de Contribuciones](#-tipos-de-contribuciones)
5. [📋 Estándares de Código](#-estándares-de-código)
6. [🧪 Testing](#-testing)
7. [📚 Documentación](#-documentación)
8. [🔄 Proceso de Review](#-proceso-de-review)
9. [🏷️ Versionado](#️-versionado)
10. [📞 Obtener Ayuda](#-obtener-ayuda)

---

## 🚀 Inicio Rápido

### ⚡ Contribución Rápida (< 5 minutos)

```bash
# 1. Fork y clonar
git clone https://github.com/tu-usuario/open-music-bot.git
cd open-music-bot

# 2. Crear rama
git checkout -b feature/mi-mejora

# 3. Hacer cambios y verificar
cargo fmt && cargo clippy && cargo test

# 4. Commit y push
git add .
git commit -m "feat: descripción clara del cambio"
git push origin feature/mi-mejora

# 5. Crear Pull Request
```

---

## 🛠️ Configuración del Entorno

### 📋 Prerequisitos

**Desarrollo Local:**
- **Rust** 1.82+ ([rustup.rs](https://rustup.rs/))
- **Git** 2.30+
- **yt-dlp** latest (`pip3 install --upgrade yt-dlp`)

**Dependencias del Sistema (Ubuntu/Debian):**
```bash
sudo apt update && sudo apt install -y \
    build-essential cmake pkg-config \
    libssl-dev libopus-dev python3-pip
```

### 🐳 Opción Docker (Recomendado)

```bash
# Setup completo con Docker
cp .env.example .env
# Configurar DISCORD_TOKEN en .env
docker-compose up -d --build
```

### 🔧 Configuración de IDE

**VS Code (Recomendado):**
```json
// .vscode/settings.json
{
    "rust-analyzer.cargo.features": ["all"],
    "rust-analyzer.checkOnSave.command": "clippy",
    "editor.formatOnSave": true
}
```

**Extensiones recomendadas:**
- `rust-lang.rust-analyzer`
- `vadimcn.vscode-lldb`
- `serayuzgur.crates`

---

## 📝 Proceso de Contribución

### 🔄 Workflow Completo

1. **Fork del repositorio**
   ```bash
   # En GitHub: Click "Fork"
   git clone https://github.com/TU-USUARIO/open-music-bot.git
   cd open-music-bot
   git remote add upstream https://github.com/ORIGINAL-OWNER/open-music-bot.git
   ```

2. **Crear rama de feature**
   ```bash
   git checkout -b feature/nombre-descriptivo
   # o
   git checkout -b fix/issue-numero
   # o  
   git checkout -b docs/mejora-documentacion
   ```

3. **Desarrollo**
   ```bash
   # Hacer cambios
   # Seguir estándares de código (ver más abajo)
   
   # Verificar calidad
   cargo fmt
   cargo clippy
   cargo test
   ```

4. **Commit**
   ```bash
   git add .
   git commit -m "tipo(scope): descripción clara
   
   - Detalle 1
   - Detalle 2
   
   Fixes #123"
   ```

5. **Mantener actualizado**
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

6. **Push y PR**
   ```bash
   git push origin feature/nombre-descriptivo
   # Crear PR en GitHub
   ```

### 📋 Checklist Pre-commit

- [ ] `cargo fmt` - Código formateado
- [ ] `cargo clippy` - Sin warnings
- [ ] `cargo test` - Tests pasan
- [ ] `cargo audit` - Sin vulnerabilidades
- [ ] Documentación actualizada
- [ ] Tests para nuevas funcionalidades
- [ ] CHANGELOG.md actualizado (si aplica)

---

## 🎯 Tipos de Contribuciones

### 🐛 **Bug Fixes**
```bash
git checkout -b fix/audio-playback-issue

# Ejemplo de commit
git commit -m "fix(audio): corrige reproducción entrecortada en bitrate alto

- Ajusta buffer size para OPUS_BITRATE > 256kbps
- Mejora manejo de memoria en audio pipeline
- Añade test para bitrates extremos

Fixes #145"
```

### ✨ **Nuevas Features**
```bash
git checkout -b feature/spotify-integration

# Desarrollo con tests
cargo test --test spotify_integration

# Commit ejemplo
git commit -m "feat(sources): añade integración con Spotify API

- Implementa SpotifySource struct
- Añade metadata fetching desde Spotify
- Mantiene compatibilidad con YouTube como fallback
- Incluye rate limiting para API calls

Closes #67"
```

### 📚 **Documentación**
```bash
git checkout -b docs/api-examples

git commit -m "docs: añade ejemplos de uso de API en README

- Ejemplos para comandos slash más comunes
- Guía de configuración avanzada
- Screenshots de interfaz Discord

Improves #89"
```

### ⚡ **Performance**
```bash
git checkout -b perf/cache-optimization

git commit -m "perf(cache): optimiza LRU cache con TTL inteligente

- Reduce memory usage 30% promedio
- Implementa adaptive TTL basado en frecuencia de uso
- Añade métricas de cache hit/miss

Benchmark:
- Before: 150MB avg, 60% hit rate
- After: 105MB avg, 85% hit rate"
```

### 🔧 **Refactoring**
```bash
git checkout -b refactor/audio-module

git commit -m "refactor(audio): reorganiza módulo para mejor mantenimiento

- Separa AudioPlayer de AudioQueue
- Extrae common traits para sources
- Mejora error handling consistency
- Mantiene API pública sin cambios

No breaking changes"
```

---

## 📋 Estándares de Código

### 🦀 **Rust Guidelines**

**Formato y Estilo:**
```rust
// ✅ Bueno: Nombres descriptivos
pub struct AudioTrackMetadata {
    pub title: String,
    pub duration: Duration,
    pub source_url: String,
}

// ✅ Bueno: Error handling con context
pub async fn fetch_audio_metadata(url: &str) -> anyhow::Result<AudioTrackMetadata> {
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("Failed to fetch: {}", url))?;
    // ...
    Ok(metadata)
}

// ✅ Bueno: Logging estructurado
tracing::info!(
    track = %metadata.title,
    duration_secs = metadata.duration.as_secs(),
    "Successfully loaded audio track"
);
```

**Async Patterns:**
```rust
// ✅ Bueno: Spawn para background tasks
tokio::spawn(async move {
    if let Err(e) = cleanup_old_cache_files().await {
        tracing::warn!("Cache cleanup failed: {:?}", e);
    }
});

// ✅ Bueno: Timeout para operaciones externas
let result = tokio::time::timeout(
    Duration::from_secs(10),
    download_audio(url)
).await??;
```

### 📝 **Commit Messages**

**Formato:**
```
tipo(scope): descripción corta en minúsculas

Explicación más detallada si es necesario.
Puede tener múltiples párrafos.

- Lista de cambios
- Otro cambio importante

Fixes #123
Closes #456
```

**Tipos válidos:**
- `feat`: Nueva funcionalidad
- `fix`: Bug fix
- `docs`: Solo documentación
- `style`: Formatting, no cambios de lógica
- `refactor`: Reestructura código sin cambiar funcionalidad
- `perf`: Mejoras de performance
- `test`: Añade o mejora tests
- `chore`: Mantenimiento, dependencias

---

## 🧪 Testing

### 🔬 **Estrategia de Testing**

**Unit Tests:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_audio_queue_add_remove() {
        let mut queue = AudioQueue::new(100);
        let track = create_test_track("Test Song");
        
        queue.add(track.clone()).await?;
        assert_eq!(queue.len(), 1);
        
        let popped = queue.pop().await?;
        assert_eq!(popped.title, "Test Song");
    }
}
```

**Integration Tests:**
```bash
# Ejecutar tests específicos
cargo test audio_queue
cargo test --test integration

# Tests con output detallado
cargo test -- --nocapture

# Tests en modo release (performance)
cargo test --release
```

### 📊 **Coverage Goals**
- **Unit Tests**: 80%+ coverage mínimo
- **Integration Tests**: Todos los comandos slash
- **Performance Tests**: Benchmarks para cambios críticos

---

## 📚 Documentación

### 📝 **Qué Documentar**

**Código:**
```rust
/// Fetch audio metadata from various sources with fallback support.
/// 
/// # Arguments
/// * `url` - The source URL (YouTube, direct audio, etc.)
/// * `timeout` - Maximum time to wait for response
/// 
/// # Returns
/// * `Ok(AudioTrackMetadata)` - Successfully parsed metadata
/// * `Err(anyhow::Error)` - Network error, parsing error, or timeout
/// 
/// # Examples
/// ```rust
/// let metadata = fetch_audio_metadata("https://youtube.com/watch?v=...", Duration::from_secs(10)).await?;
/// println!("Track: {} ({})", metadata.title, metadata.duration);
/// ```
pub async fn fetch_audio_metadata(url: &str, timeout: Duration) -> anyhow::Result<AudioTrackMetadata> {
    // ...
}
```

**README Updates:**
- Nuevas features en sección "Funcionalidades"
- Nuevos comandos en tabla de comandos
- Cambios en instalación o configuración

### 📖 **Archivos de Documentación**
- `README.md` - Información general y quick start
- `DEVELOPMENT.md` - Guía para desarrolladores
- `TROUBLESHOOTING.md` - Solución de problemas
- Este `CONTRIBUTING.md` - Guía de contribución

---

## 🔄 Proceso de Review

### 👥 **Pull Request Guidelines**

**Template de PR:**
```markdown
## 📋 Descripción
Breve descripción de los cambios realizados.

## 🎯 Tipo de Cambio
- [ ] Bug fix (cambio que corrige un issue)
- [ ] Nueva feature (cambio que añade funcionalidad)
- [ ] Breaking change (fix o feature que causa incompatibilidad)
- [ ] Documentación

## 🧪 Testing
- [ ] Tests unitarios pasan
- [ ] Tests de integración pasan
- [ ] Probado manualmente con Discord

## 📋 Checklist
- [ ] Código formateado (`cargo fmt`)
- [ ] Sin warnings (`cargo clippy`)
- [ ] Documentación actualizada
- [ ] Tests añadidos/actualizados
```

### ⏱️ **Timeline Esperado**
- **Primera Review**: 2-3 días hábiles
- **Feedback Response**: 1-2 días
- **Merge**: 1-2 días después de aprobación

### 🎯 **Criterios de Aprobación**
- [ ] Código limpio y bien documentado
- [ ] Tests pasan sin issues
- [ ] Performance no se degrada
- [ ] Compatible con features existentes
- [ ] Documentación actualizada

---

## 🏷️ Versionado

Seguimos [Semantic Versioning](https://semver.org/):

- **MAJOR** (1.x.x): Breaking changes
- **MINOR** (x.1.x): Nuevas features (backward compatible)
- **PATCH** (x.x.1): Bug fixes

**Ejemplos:**
- `1.0.0` → `1.0.1`: Bug fix en audio playback
- `1.0.1` → `1.1.0`: Nuevo comando `/lyrics`
- `1.1.0` → `2.0.0`: Cambio en API de comandos

---

## 📞 Obtener Ayuda

### 💬 **Canales de Comunicación**

**Para Dudas Técnicas:**
- Crear issue con label `question`
- Revisar [DEVELOPMENT.md](DEVELOPMENT.md) para patrones
- Consultar [TROUBLESHOOTING.md](TROUBLESHOOTING.md)

**Para Bugs:**
- Crear issue con template de bug report
- Incluir logs relevantes
- Pasos para reproducir

**Para Feature Requests:**
- Crear issue con template de feature request
- Explicar use case y beneficios
- Mockups o ejemplos si es UI

### 🆘 **Template de Issue**

```markdown
**Describe el bug**
Descripción clara del problema.

**Para reproducir**
1. Ejecutar comando '...'
2. Ver error

**Comportamiento esperado**
Lo que debería pasar.

**Screenshots/Logs**
```
[logs aquí]
```

**Entorno:**
- OS: [Ubuntu 22.04]
- Rust: [1.82.0] 
- Docker: [si aplica]
```

---

## 🎉 Reconocimientos

### 🏆 **Tipos de Contribuciones Valoradas**

- 🐛 **Bug Hunters**: Encuentran y reportan bugs
- ✨ **Feature Developers**: Implementan nuevas funcionalidades
- 📚 **Documentation Writers**: Mejoran documentación
- 🔧 **Performance Optimizers**: Mejoran velocidad/memoria
- 🧪 **Test Writers**: Añaden cobertura de testing
- 🎨 **UX Improvers**: Mejoran experiencia de usuario
- 🌍 **Community Helpers**: Ayudan a otros usuarios

### 📜 **Código de Conducta**

- **Sé respetuoso**: Trata a todos con respeto profesional
- **Sé constructivo**: Critica código, no personas
- **Sé paciente**: Recuerda que todos estamos aprendiendo
- **Sé colaborativo**: Trabaja en equipo hacia objetivos comunes

---

## 🚀 ¡Empezar a Contribuir!

1. **Lee la documentación**: README.md y DEVELOPMENT.md
2. **Fork el repositorio**: Haz tu copia personal
3. **Encuentra un issue**: Label "good first issue" para empezar
4. **¡Haz tu primer PR!**: Siguiendo esta guía

**¡Gracias por hacer Open Music Bot mejor para toda la comunidad! 🎵🤖**

---

*¿Encontraste un error en esta guía? ¡Contribuye arreglándolo! 😄*