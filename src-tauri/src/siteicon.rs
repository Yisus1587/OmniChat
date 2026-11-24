//! Icono de las webs personalizadas (Discord, Slack o cualquier otra).
//!
//! Se descarga la página de inicio, se buscan sus iconos (`apple-touch-icon`, `<link
//! rel="icon">` y el manifiesto web) y se guarda el más adecuado como data URL en la base
//! de datos. Las webs populares ya tienen su logo oficial incluido en la interfaz; esto
//! cubre todas las demás.

use std::time::Duration;

use base64::Engine;
use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, Url};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager};

use crate::catalog::ServiceKind;
use crate::db::{self, Service};

const MAX_PAGE: usize = 1024 * 1024;
const MAX_ICON: usize = 256 * 1024;
const MAX_MANIFEST: usize = 128 * 1024;
/// Tamaño ideal: el icono se ve a 34 px en la barra (68 px en pantallas 2x).
const IDEAL: u32 = 96;
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
    (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0";

#[derive(Debug, Clone, PartialEq)]
struct Candidate {
    url: Url,
    /// Lado en píxeles (0 = desconocido).
    size: u32,
}

impl Candidate {
    /// Mayor es mejor: el más pequeño que llegue al tamaño ideal; si ninguno llega, el mayor.
    fn score(&self) -> u32 {
        if self.size >= IDEAL {
            100_000 - self.size.min(99_999)
        } else {
            self.size
        }
    }
}

fn client() -> reqwest::Result<Client> {
    Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
}

/// Lee el cuerpo hasta `max` bytes (si es mayor, se descarta).
async fn read_limited(mut resp: reqwest::Response, max: usize) -> Option<Vec<u8>> {
    if resp.content_length().is_some_and(|n| n as usize > max) {
        return None;
    }
    let mut buf = Vec::new();
    while let Some(chunk) = resp.chunk().await.ok()? {
        buf.extend_from_slice(&chunk);
        if buf.len() > max {
            return None;
        }
    }
    Some(buf)
}

/// Valor de un atributo en una etiqueta HTML (`<link rel="icon" href="…">`).
fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find(name) {
        let start = from + i;
        from = start + name.len();
        // Debe ser un atributo completo: precedido de espacio y seguido de `=`.
        if !lower[..start].ends_with(|c: char| c.is_ascii_whitespace()) {
            continue;
        }
        let rest = lower[from..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else { continue };
        let offset = tag.len() - rest.len();
        let raw = tag[offset..].trim_start();
        let value = match raw.chars().next()? {
            q @ ('"' | '\'') => raw[1..].split(q).next()?,
            _ => raw.split(|c: char| c.is_ascii_whitespace() || c == '>').next()?,
        };
        return Some(value.replace("&amp;", "&").trim().to_string());
    }
    None
}

fn parse_size(sizes: &str) -> u32 {
    if sizes.eq_ignore_ascii_case("any") {
        return 512;
    }
    sizes
        .split_whitespace()
        .filter_map(|s| s.to_ascii_lowercase().split('x').next()?.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
}

/// Iconos declarados en el HTML y dirección del manifiesto web.
fn parse_html(html: &str, base: &Url) -> (Vec<Candidate>, Option<Url>) {
    let mut icons = Vec::new();
    let mut manifest = None;
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find("<link") {
        let start = from + i;
        let end = lower[start..].find('>').map(|e| start + e).unwrap_or(lower.len());
        let tag = &html[start..end];
        from = end;
        let (Some(rel), Some(href)) = (attr(tag, "rel"), attr(tag, "href")) else { continue };
        let Ok(url) = base.join(&href) else { continue };
        let rel = rel.to_ascii_lowercase();
        let rels: Vec<&str> = rel.split_whitespace().collect();
        if rels.contains(&"manifest") {
            manifest = Some(url);
        } else if rels.iter().any(|r| r.starts_with("apple-touch-icon")) {
            let size = attr(tag, "sizes").map(|s| parse_size(&s)).filter(|s| *s > 0).unwrap_or(180);
            icons.push(Candidate { url, size });
        } else if rels.contains(&"icon") {
            let svg = attr(tag, "type").is_some_and(|t| t.contains("svg")) || href.to_ascii_lowercase().ends_with(".svg");
            let size = attr(tag, "sizes").map(|s| parse_size(&s)).unwrap_or(0);
            let size = if size == 0 && svg { 512 } else if size == 0 { 32 } else { size };
            icons.push(Candidate { url, size });
        }
    }
    (icons, manifest)
}

#[derive(serde::Deserialize)]
struct Manifest {
    #[serde(default)]
    icons: Vec<ManifestIcon>,
}

#[derive(serde::Deserialize)]
struct ManifestIcon {
    src: String,
    #[serde(default)]
    sizes: String,
    #[serde(default)]
    purpose: String,
}

fn parse_manifest(json: &[u8], base: &Url) -> Vec<Candidate> {
    let Ok(m) = serde_json::from_slice::<Manifest>(json) else { return Vec::new() };
    m.icons
        .into_iter()
        // Los monocromos son siluetas para la barra de tareas: no sirven aquí.
        .filter(|i| !i.purpose.contains("monochrome"))
        .filter_map(|i| Some(Candidate { url: base.join(&i.src).ok()?, size: parse_size(&i.sizes) }))
        .collect()
}

/// Tipo real de la imagen según sus primeros bytes.
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    let head = &bytes[..bytes.len().min(512)];
    if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if head.starts_with(b"GIF8") {
        Some("image/gif")
    } else if head.len() > 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        Some("image/webp")
    } else if head.starts_with(&[0, 0, 1, 0]) {
        Some("image/x-icon")
    } else if String::from_utf8_lossy(head).to_ascii_lowercase().contains("<svg") {
        Some("image/svg+xml")
    } else {
        None
    }
}

async fn download_icon(client: &Client, c: &Candidate) -> Option<String> {
    if !matches!(c.url.scheme(), "http" | "https") {
        return None;
    }
    let resp = client.get(c.url.clone()).send().await.ok()?.error_for_status().ok()?;
    let bytes = read_limited(resp, MAX_ICON).await?;
    let mime = sniff(&bytes)?;
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes)))
}

/// Busca y descarga el icono de una web. `None` si no se encuentra ninguno usable.
pub async fn fetch(page: &str) -> Option<String> {
    let client = client().ok()?;
    let mut candidates = Vec::new();
    let mut base: Url = page.parse().ok()?;

    if let Ok(resp) = client.get(base.clone()).send().await {
        base = resp.url().clone();
        let is_html = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|t| t.contains("html"));
        if is_html {
            if let Some(body) = read_limited(resp, MAX_PAGE).await {
                let (icons, manifest) = parse_html(&String::from_utf8_lossy(&body), &base);
                candidates.extend(icons);
                if let Some(m) = manifest {
                    if let Ok(r) = client.get(m.clone()).send().await {
                        if let Some(json) = read_limited(r, MAX_MANIFEST).await {
                            candidates.extend(parse_manifest(&json, &m));
                        }
                    }
                }
            }
        }
    }
    // Ubicaciones estándar por si la página no declara nada.
    for (path, size) in [("/apple-touch-icon.png", 180), ("/favicon.ico", 16)] {
        if let Ok(url) = base.join(path) {
            candidates.push(Candidate { url, size });
        }
    }

    candidates.sort_by_key(|c| std::cmp::Reverse(c.score()));
    candidates.dedup_by(|a, b| a.url == b.url);
    for c in candidates.iter().take(6) {
        if let Some(icon) = download_icon(&client, c).await {
            return Some(icon);
        }
    }
    None
}

/// Descarga (en segundo plano) el icono de un servicio personalizado, lo guarda y avisa a la UI.
pub fn refresh(app: &AppHandle, service: &Service) {
    if service.kind != ServiceKind::Custom {
        return;
    }
    let Some(url) = service.url.clone() else { return };
    let (app, id) = (app.clone(), service.id.clone());
    tauri::async_runtime::spawn(async move {
        let Some(icon) = fetch(&url).await else {
            tracing::info!(service = %id, "no icon found");
            return;
        };
        let pool = app.state::<SqlitePool>();
        match db::set_icon(&pool, &id, Some(&icon)).await {
            Ok(updated) => {
                let _ = app.emit("service:updated", &updated);
            }
            Err(e) => tracing::warn!(error = %e, "could not save icon"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_icons_in_html() {
        let base: Url = "https://example.com/app/".parse().unwrap();
        let html = r#"<head>
            <LINK REL="shortcut icon" href="/favicon.ico">
            <link rel=icon type="image/png" sizes="192x192" href="icons/192.png">
            <link rel="icon" sizes="32x32 16x16" href='https://cdn.example.com/32.png?a=1&amp;b=2'>
            <link rel="apple-touch-icon" href="/apple.png">
            <link rel="manifest" href="/site.webmanifest">
            <link rel="stylesheet" href="/x.css">
            <link rel="mask-icon" href="/mask.svg">
        </head>"#;
        let (icons, manifest) = parse_html(html, &base);
        let got: Vec<(String, u32)> = icons.iter().map(|c| (c.url.to_string(), c.size)).collect();
        assert_eq!(
            got,
            vec![
                ("https://example.com/favicon.ico".to_string(), 32),
                ("https://example.com/app/icons/192.png".to_string(), 192),
                ("https://cdn.example.com/32.png?a=1&b=2".to_string(), 32),
                ("https://example.com/apple.png".to_string(), 180),
            ]
        );
        assert_eq!(manifest.unwrap().as_str(), "https://example.com/site.webmanifest");

        // El preferido es el más pequeño que llegue a 96 px.
        let best = icons.iter().max_by_key(|c| c.score()).unwrap();
        assert_eq!(best.size, 180);
    }

    #[test]
    fn manifest_icons_and_sniffing() {
        let base: Url = "https://example.com/m.json".parse().unwrap();
        let json = br#"{"icons":[{"src":"/a.png","sizes":"512x512"},{"src":"/m.png","sizes":"96x96","purpose":"monochrome"}]}"#;
        let icons = parse_manifest(json, &base);
        assert_eq!(icons.len(), 1);
        assert_eq!(icons[0].size, 512);
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\nrest"), Some("image/png"));
        assert_eq!(sniff(b"<?xml version='1.0'?><svg></svg>"), Some("image/svg+xml"));
        assert_eq!(sniff(b"<html>not found</html>"), None);
    }
}
