# YouTube Authentication Fix Guide

## Current Status ✅
The bot now has a **4-tier fallback system**:
1. **YouTube Fast** (yt-dlp with optimized parameters)
2. **YouTube Standard** (yt-dlp with detailed search)  
3. **Invidious API** (8 working instances with direct scraping fallback)
4. **RSS Feeds** (YouTube RSS channels as last resort)

## Long-term Solution: Real Cookies

To completely solve the YouTube authentication issue, you can add real browser cookies:

### Option 1: Export from Browser
```bash
# Install browser-cookie3
pip3 install browser-cookie3

# Extract cookies from your browser
python3 -c "
import browser_cookie3
cookies = browser_cookie3.chrome(domain_name='youtube.com')
with open('/app/data/youtube_cookies.txt', 'w') as f:
    for cookie in cookies:
        f.write(f'{cookie.domain}\t{cookie.name}\t{cookie.value}\n')
"
```

### Option 2: Manual Cookie Export
1. Go to youtube.com in browser
2. Open Developer Tools (F12)
3. Go to Application/Storage > Cookies
4. Export cookies to file format:
```
# youtube_cookies.txt
.youtube.com	CONSENT	YES+...
.youtube.com	VISITOR_INFO1_LIVE	...
.youtube.com	YSC	...
```

### Option 3: Update Docker with Cookies
Add to your `docker-compose.yml`:
```yaml
services:
  open-music-bot:
    volumes:
      - ./youtube_cookies.txt:/app/data/youtube_cookies.txt:ro
    environment:
      - YTDLP_OPTS=--cookies /app/data/youtube_cookies.txt --user-agent 'Mozilla/5.0...'
```

## Current Working Instances
Updated Invidious instances (working as of 2025):
- ✅ yewtu.be (Germany)
- ✅ inv.nadeko.net (Chile) 
- ✅ invidious.nerdvpn.de (Ukraine)
- ✅ invidious.protokolla.fi (Finland)
- ✅ invidious.privacydev.net (US)
- ✅ vid.puffyan.us (US)
- ✅ invidious.weblibre.org (France)
- ✅ inv.bp.projectsegfau.lt (Lithuania)

## Testing the New System
Try these commands:
```
/play milo j
/play The Strokes - Ode To The Mets
/search Bad Bunny
```

The bot will now:
1. Try YouTube Fast first
2. Fall back to YouTube Standard
3. Fall back to Invidious (8 instances)
4. Fall back to YouTube RSS
5. Use Invidious for playback if yt-dlp fails

## Logs to Monitor
Watch for these log messages:
- `🔄 Búsqueda rápida falló, usando método estándar...`
- `🔄 YouTube falló, usando Invidious...`
- `🔄 Todas las instancias de Invidious fallaron, intentando scraping directo...`
- `✅ Input creado con Invidious para: [track]`