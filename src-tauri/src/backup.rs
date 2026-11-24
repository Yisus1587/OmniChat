//! Exportar e importar la configuración (servicios, espacios y ajustes) como JSON.
//!
//! No incluye las sesiones de cada web (cookies) ni el PIN: al importar en otro equipo
//! hay que iniciar sesión otra vez en cada servicio.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::catalog::{normalize_custom_url, ServiceKind};
use crate::db::{self, ServicePatch};
use crate::error::{AppError, AppResult};
use crate::host::{ServiceHost, ZOOM_MAX, ZOOM_MIN};
use crate::prefs;
use crate::settings::{self, Settings};

const FORMAT: &str = "omnichat";
const VERSION: u32 = 1;
const MAX_SERVICES: usize = 200;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceEntry {
    id: String,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceEntry {
    kind: ServiceKind,
    label: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    workspace_id: Option<String>,
    #[serde(default = "yes")]
    notify: bool,
    #[serde(default = "yes")]
    sound: bool,
    #[serde(default = "one")]
    zoom: f64,
    #[serde(default = "yes")]
    hibernate: bool,
    #[serde(default)]
    user_agent: Option<String>,
}

fn yes() -> bool {
    true
}

fn one() -> f64 {
    1.0
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigFile {
    format: String,
    version: u32,
    #[serde(default)]
    exported_at: i64,
    #[serde(default)]
    workspaces: Vec<WorkspaceEntry>,
    #[serde(default)]
    services: Vec<ServiceEntry>,
    #[serde(default)]
    settings: Option<Settings>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub services: usize,
    pub workspaces: usize,
    /// Servicios que ya existían (mismo tipo, nombre y dirección) y no se duplicaron.
    pub skipped: usize,
}

fn export_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let p = app.path();
    p.download_dir()
        .or_else(|_| p.document_dir())
        .or_else(|_| p.home_dir())
        .map_err(|e| AppError::NotFound(format!("carpeta de descargas ({e})")))
}

/// Guarda la configuración en Descargas, la señala en el Explorador y devuelve la ruta.
pub async fn export(app: &AppHandle) -> AppResult<String> {
    let pool = app.state::<SqlitePool>();
    let mut settings = app.state::<ServiceHost>().settings();
    // Estado del momento, no preferencias.
    settings.focus_until = None;
    settings.active_workspace = None;

    let file = ConfigFile {
        format: FORMAT.into(),
        version: VERSION,
        exported_at: settings::now_ms(),
        workspaces: db::list_workspaces(&pool)
            .await?
            .into_iter()
            .map(|w| WorkspaceEntry { id: w.id, name: w.name })
            .collect(),
        services: db::list(&pool)
            .await?
            .into_iter()
            .map(|s| ServiceEntry {
                kind: s.kind,
                label: s.label,
                url: s.url,
                workspace_id: s.workspace_id,
                notify: s.notify,
                sound: s.sound,
                zoom: s.zoom,
                hibernate: s.hibernate,
                user_agent: s.user_agent,
            })
            .collect(),
        settings: Some(settings),
    };
    let json = serde_json::to_string_pretty(&file).map_err(|e| AppError::InvalidInput(e.to_string()))?;

    let date = chrono::Local::now().format("%Y-%m-%d");
    let dir = export_dir(app)?;
    let mut path = dir.join(format!("omnichat-configuracion-{date}.json"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("omnichat-configuracion-{date} ({n}).json"));
        n += 1;
    }
    std::fs::write(&path, json)?;
    let _ = app.opener().reveal_item_in_dir(&path);
    Ok(path.display().to_string())
}

/// Añade lo que contiene el fichero: crea los espacios y servicios que falten (sin
/// duplicar los que ya hay) y aplica sus preferencias.
pub async fn import(app: &AppHandle, json: &str) -> AppResult<ImportSummary> {
    let bad = |why: &str| AppError::InvalidInput(format!("el fichero no es una configuración de OmniChat válida ({why})"));
    let file: ConfigFile = serde_json::from_str(json).map_err(|e| bad(&e.to_string()))?;
    if file.format != FORMAT {
        return Err(bad("formato desconocido"));
    }
    if file.version > VERSION {
        return Err(AppError::InvalidInput(
            "el fichero es de una versión más nueva de OmniChat; actualiza la app primero".into(),
        ));
    }
    if file.services.len() > MAX_SERVICES {
        return Err(bad("demasiados servicios"));
    }

    let pool = app.state::<SqlitePool>();
    let mut summary = ImportSummary { services: 0, workspaces: 0, skipped: 0 };

    // Espacios: se reutiliza el que ya tenga el mismo nombre.
    let mut existing_ws: HashMap<String, String> = db::list_workspaces(&pool)
        .await?
        .into_iter()
        .map(|w| (w.name.to_lowercase(), w.id))
        .collect();
    let mut ws_map: HashMap<String, String> = HashMap::new();
    for w in &file.workspaces {
        let name = w.name.trim();
        if name.is_empty() || name.chars().count() > 30 {
            continue;
        }
        let id = match existing_ws.get(&name.to_lowercase()) {
            Some(id) => id.clone(),
            None => {
                let created = db::create_workspace(&pool, name).await?;
                existing_ws.insert(name.to_lowercase(), created.id.clone());
                summary.workspaces += 1;
                created.id
            }
        };
        ws_map.insert(w.id.clone(), id);
    }

    let mut current = db::list(&pool).await?;
    for s in &file.services {
        let label: String = s.label.trim().chars().take(40).collect();
        let label = if label.is_empty() { s.kind.name().to_string() } else { label };
        let url = match s.kind {
            ServiceKind::Custom => match s.url.as_deref().and_then(normalize_custom_url) {
                Some(u) => Some(u),
                None => {
                    summary.skipped += 1;
                    continue;
                }
            },
            _ => None,
        };
        if current.iter().any(|c| c.kind == s.kind && c.label == label && c.url == url) {
            summary.skipped += 1;
            continue;
        }
        let ws = s.workspace_id.as_ref().and_then(|w| ws_map.get(w)).map(String::as_str);
        let created = db::create(&pool, s.kind, &label, url.as_deref(), ws).await?;
        let mut patch = ServicePatch {
            user_agent: s.user_agent.clone(),
            notify: Some(s.notify),
            sound: Some(s.sound),
            zoom: Some(s.zoom.clamp(ZOOM_MIN, ZOOM_MAX)),
            hibernate: Some(s.hibernate),
            ..Default::default()
        };
        if crate::advanced::validate(&mut patch).is_err() {
            patch.user_agent = None;
        }
        let updated = db::update(&pool, &created.id, &patch).await?;
        crate::siteicon::refresh(app, &updated);
        current.push(updated);
        summary.services += 1;
    }

    if let Some(imported) = file.settings {
        let now = app.state::<ServiceHost>().settings();
        let merged = Settings {
            focus_until: now.focus_until,
            active_workspace: now.active_workspace,
            sync: now.sync,
            ..imported
        };
        if merged.validate().is_ok() {
            prefs::save(app, merged).await?;
        }
    }
    Ok(summary)
}
