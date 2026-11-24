//! Ajustes avanzados por servicio: agente de usuario (vista móvil), proxy, CSS/JS propio y
//! extensiones de Chrome.
//!
//! Son opciones del *entorno* de WebView2, que se fijan al crear el webview: al cambiarlas
//! hay que volver a crearlo (ver `ServiceHost::recreate`). Como cada servicio tiene su propia
//! carpeta de datos, cada uno puede tener un proxy o extensiones distintos.

use std::path::PathBuf;

use tauri::{AppHandle, Manager, Url};

use crate::db::{Service, ServicePatch};
use crate::error::{AppError, AppResult};

pub const MOBILE_UA: &str = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 \
    (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36";
pub const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
    (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const MAX_UA: usize = 400;
const MAX_CODE: usize = 100 * 1024;

/// Agente de usuario efectivo del servicio (`None` = el de WebView2).
pub fn user_agent(s: &Service) -> Option<&str> {
    match s.user_agent.as_deref() {
        None | Some("") => None,
        Some("mobile") => Some(MOBILE_UA),
        Some("chrome") => Some(CHROME_UA),
        Some(custom) => Some(custom),
    }
}

/// Proxy válido: `http://` o `socks5://`, con host y puerto, sin usuario ni ruta
/// (WebView2 no admite proxies con contraseña).
pub fn parse_proxy(input: &str) -> Result<Url, String> {
    let input = input.trim();
    let with_scheme = if input.contains("://") { input.to_string() } else { format!("http://{input}") };
    let url = Url::parse(&with_scheme).map_err(|_| "dirección de proxy no válida".to_string())?;
    if !matches!(url.scheme(), "http" | "socks5") {
        return Err("el proxy debe empezar por http:// o socks5://".into());
    }
    if url.host_str().is_none_or(str::is_empty) || url.port().is_none() {
        return Err("indica el servidor y el puerto del proxy, p. ej. socks5://127.0.0.1:1080".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("los proxies con usuario y contraseña no están soportados".into());
    }
    if !matches!(url.path(), "" | "/") || url.query().is_some() || url.fragment().is_some() {
        return Err("el proxy sólo lleva servidor y puerto".into());
    }
    Ok(url)
}

/// Valida y normaliza los campos avanzados de un cambio.
pub fn validate(p: &mut ServicePatch) -> AppResult<()> {
    let bad = |m: &str| AppError::InvalidInput(m.into());
    if let Some(ua) = p.user_agent.as_mut() {
        *ua = ua.trim().to_string();
        if ua.len() > MAX_UA || ua.chars().any(char::is_control) {
            return Err(bad("el agente de usuario no es válido"));
        }
    }
    if let Some(proxy) = p.proxy.as_mut() {
        if !proxy.trim().is_empty() {
            let url = parse_proxy(proxy).map_err(AppError::InvalidInput)?;
            *proxy = format!("{}://{}:{}", url.scheme(), url.host_str().unwrap_or_default(), url.port().unwrap_or_default());
        } else {
            proxy.clear();
        }
    }
    for code in [&p.custom_css, &p.custom_js].into_iter().flatten() {
        if code.len() > MAX_CODE {
            return Err(bad("el CSS o JavaScript propio no puede superar 100 KB"));
        }
    }
    Ok(())
}

/// Script que aplica el CSS y el JavaScript del usuario en la web del servicio.
///
/// El CSS va en una hoja "adoptada" (no le afecta la CSP de la web, que suele bloquear los
/// `<style>` insertados) y se vuelve a añadir si la web la quita. El JavaScript se ejecuta
/// cuando el documento está listo; sólo en la web del servicio, que no tiene acceso a OmniChat.
pub fn user_script(s: &Service) -> Option<String> {
    let css = s.custom_css.as_deref().filter(|c| !c.trim().is_empty());
    let js = s.custom_js.as_deref().filter(|c| !c.trim().is_empty());
    if css.is_none() && js.is_none() {
        return None;
    }
    let mut out = String::from("(function(){\n");
    if let Some(css) = css {
        let css = serde_json::to_string(css).unwrap_or_default();
        out.push_str(&format!(
            "try{{const sheet=new CSSStyleSheet();sheet.replaceSync({css});\
             const add=()=>{{if(!document.adoptedStyleSheets.includes(sheet))\
             document.adoptedStyleSheets=[...document.adoptedStyleSheets,sheet];}};\
             add();document.addEventListener('DOMContentLoaded',add);setInterval(add,2000);}}\
             catch(e){{console.error('OmniChat CSS',e);}}\n"
        ));
    }
    if let Some(js) = js {
        out.push_str(&format!(
            "const run=()=>{{try{{\n{js}\n}}catch(e){{console.error('OmniChat JS',e);}}}};\
             if(document.readyState==='loading')document.addEventListener('DOMContentLoaded',run,{{once:true}});else run();\n"
        ));
    }
    out.push_str("})();");
    Some(out)
}

/// Carpeta con las extensiones de Chrome descomprimidas (una subcarpeta por extensión).
pub fn extensions_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::NotFound(format!("carpeta de datos ({e})")))?
        .join("extensions");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Nombres de las extensiones instaladas (subcarpetas con `manifest.json`).
pub fn list_extensions(app: &AppHandle) -> AppResult<Vec<String>> {
    let dir = extensions_dir(app)?;
    let mut names: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|e| e.path().join("manifest.json").is_file())
        .map(|e| {
            let manifest = std::fs::read_to_string(e.path().join("manifest.json")).unwrap_or_default();
            serde_json::from_str::<serde_json::Value>(&manifest)
                .ok()
                .and_then(|m| m.get("name").and_then(|n| n.as_str()).map(str::to_string))
                .filter(|n| !n.starts_with("__MSG_"))
                .unwrap_or_else(|| e.file_name().to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxies() {
        assert_eq!(parse_proxy("127.0.0.1:8080").unwrap().as_str(), "http://127.0.0.1:8080/");
        assert!(parse_proxy("socks5://localhost:1080").is_ok());
        assert!(parse_proxy("ftp://x:21").is_err());
        assert!(parse_proxy("http://user:pw@x:80").is_err());
        assert!(parse_proxy("http://x").is_err());
        assert!(parse_proxy("http://x:80/path").is_err());
    }

    #[test]
    fn validate_normalizes() {
        let mut p = ServicePatch {
            proxy: Some(" socks5://10.0.0.2:1080 ".into()),
            user_agent: Some(" mobile ".into()),
            ..Default::default()
        };
        validate(&mut p).unwrap();
        assert_eq!(p.proxy.as_deref(), Some("socks5://10.0.0.2:1080"));
        assert_eq!(p.user_agent.as_deref(), Some("mobile"));
        let mut bad = ServicePatch { user_agent: Some("a\nb".into()), ..Default::default() };
        assert!(validate(&mut bad).is_err());
    }
}
