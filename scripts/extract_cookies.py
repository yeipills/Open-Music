#!/usr/bin/env python3
"""
Script para extraer cookies de YouTube desde el navegador
Requiere: pip install browser-cookie3
"""

import json
import sys
import os
from datetime import datetime, timezone

try:
    import browser_cookie3
except ImportError:
    print("❌ ERROR: browser_cookie3 no está instalado")
    print("💡 Instalar con: pip3 install browser_cookie3")
    sys.exit(1)

def extract_youtube_cookies():
    """Extrae cookies de YouTube desde navegadores instalados"""
    cookies_data = []
    browsers = {
        'Chrome': browser_cookie3.chrome,
        'Firefox': browser_cookie3.firefox,
        'Edge': browser_cookie3.edge,
        'Safari': browser_cookie3.safari if hasattr(browser_cookie3, 'safari') else None
    }
    
    print("🔍 Buscando cookies de YouTube en navegadores...")
    
    for browser_name, browser_func in browsers.items():
        if browser_func is None:
            continue
            
        try:
            print(f"   📂 Verificando {browser_name}...")
            cj = browser_func(domain_name='youtube.com')
            
            for cookie in cj:
                if 'youtube.com' in cookie.domain or 'google.com' in cookie.domain:
                    # Convertir timestamp de expiración
                    expires = int(cookie.expires) if cookie.expires else 2147483647
                    
                    # Formato Netscape
                    secure = "TRUE" if cookie.secure else "FALSE"
                    http_only = "TRUE" if cookie.domain.startswith('.') else "FALSE"
                    
                    cookie_line = f"{cookie.domain}\t{http_only}\t{cookie.path}\t{secure}\t{expires}\t{cookie.name}\t{cookie.value}"
                    cookies_data.append(cookie_line)
                    
            if cookies_data:
                print(f"   ✅ {len(cookies_data)} cookies encontradas en {browser_name}")
                break
                
        except Exception as e:
            print(f"   ⚠️  {browser_name}: {str(e)}")
            continue
    
    return cookies_data

def save_cookies_file(cookies_data, output_path):
    """Guarda las cookies en formato Netscape"""
    with open(output_path, 'w') as f:
        f.write("# Netscape HTTP Cookie File\n")
        f.write(f"# Generated on {datetime.now(timezone.utc).strftime('%Y-%m-%d %H:%M:%S UTC')}\n")
        f.write("# This file contains the HTTP cookies for YouTube\n")
        f.write("#\n")
        
        for cookie in cookies_data:
            f.write(cookie + '\n')

def main():
    print("🍪 EXTRACTOR DE COOKIES DE YOUTUBE")
    print("=" * 40)
    
    # Extraer cookies
    cookies = extract_youtube_cookies()
    
    if not cookies:
        print("❌ No se encontraron cookies de YouTube")
        print("💡 Asegúrate de:")
        print("   1. Haber iniciado sesión en YouTube en tu navegador")
        print("   2. Tener permisos para leer cookies del navegador")
        print("   3. Cerrar el navegador antes de ejecutar este script")
        sys.exit(1)
    
    # Guardar en el directorio config
    config_dir = "../config"
    os.makedirs(config_dir, exist_ok=True)
    
    output_path = os.path.join(config_dir, "cookies.txt")
    save_cookies_file(cookies, output_path)
    
    print(f"✅ Cookies guardadas en: {output_path}")
    print(f"📊 Total de cookies: {len(cookies)}")
    
    # Verificar que las cookies importantes están presentes
    cookie_content = '\n'.join(cookies)
    important_cookies = ['CONSENT', 'VISITOR_INFO1_LIVE', 'YSC', '__Secure-1PSID', '__Secure-3PSID']
    
    found_important = []
    for cookie_name in important_cookies:
        if cookie_name in cookie_content:
            found_important.append(cookie_name)
    
    print(f"🔑 Cookies importantes encontradas: {len(found_important)}/{len(important_cookies)}")
    
    if len(found_important) >= 3:
        print("✅ Suficientes cookies para autenticación")
    else:
        print("⚠️  Pocas cookies importantes - podrían no funcionar")
        print("💡 Intenta:")
        print("   1. Visitar youtube.com y reproducir algunos videos")
        print("   2. Cerrar y abrir el navegador")
        print("   3. Ejecutar este script nuevamente")
    
    print("\n📋 SIGUIENTE PASO:")
    print("   Ejecutar: docker compose restart")

if __name__ == "__main__":
    main()