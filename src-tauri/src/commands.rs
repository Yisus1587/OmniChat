//! Comandos IPC del launcher. Cada uno debe figurar en `build.rs` (AppManifest) y en
//! `capabilities/default.json`; si no, Tauri lo bloquea.
//!
//! Los que crean webviews son `async`: en Windows `add_child` no puede ejecutarse en el
//! hilo principal (ver tauri `Window::add_child`).

use std::collections::HashMap;

use sqlx::SqlitePool;
use tauri::{AppHandle, State};

use crate::backup::{self, ImportSummary};
use crate::db::Task;
use crate::catalog::{normalize_custom_url, ServiceKind};
use crate::db::{self, Service, ServicePatch, Workspace};
use crate::error::{AppError, AppResult};
use crate::host::{Bounds, HistoryEntry, ServiceHost};
use crate::lock::{self, LockStatus};
use crate::settings::{self, Settings};
use crate::{advanced, memory, prefs, siteicon, sync, toast};
use tauri_plugin_opener::OpenerExt;

fn clean_label(label: Option<String>) -> AppResult<Option<String>> {
    let label = label.map(|l| l.trim().to_string()).filter(|l| !l.is_empty());
    if label.as_ref().is_some_and(|l| l.chars().count() > 40) {
        return Err(AppError::InvalidInput("el nombre es demasiado largo (máx. 40)".into()));
    }
    Ok(label)
}

// ---------------------------------------------------------------------------
// Servicios
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_services(pool: State<'_, SqlitePool>) -> AppResult<Vec<Service>> {
    db::list(&pool).await
}

/// La UI informa del área de contenido (y lo repite al redimensionar). La primera vez
/// también crea los webviews de todos los servicios para que empiecen a cargar y a
/// recibir mensajes en segundo plano.
#[tauri::command]
pub async fn sync_layout(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    host: State<'_, ServiceHost>,
    bounds: Bounds,
    active: Option<String>,
    overlay: bool,
) -> AppResult<()> {
    host.set_bounds(bounds);
    host.set_active(active);
    host.set_overlay(overlay);
    for service in db::list(&pool).await? {
        if let Err(e) = host.ensure(&app, &service) {
            tracing::warn!(service = %service.id, error = %e, "could not create webview");
        }
    }
    host.apply(&app)
}

#[tauri::command]
pub async fn add_service(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    host: State<'_, ServiceHost>,
    kind: ServiceKind,
    label: Option<String>,
    url: Option<String>,
    workspace_id: Option<String>,
) -> AppResult<Service> {
    let url = match kind {
        ServiceKind::Custom => Some(
            url.as_deref()
                .and_then(normalize_custom_url)
                .ok_or_else(|| AppError::InvalidInput("escribe una dirección web válida (https://…)".into()))?,
        ),
        _ => None,
    };
    let label = clean_label(label)?.unwrap_or_else(|| match &url {
        Some(u) => tauri::Url::parse(u)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string()))
            .unwrap_or_else(|| kind.name().to_string()),
        None => kind.name().to_string(),
    });
    let workspace_id = workspace_id.filter(|w| !w.is_empty());
    let service = db::create(&pool, kind, &label, url.as_deref(), workspace_id.as_deref()).await?;
    host.ensure(&app, &service)?;
    siteicon::refresh(&app, &service);
    Ok(service)
}

#[tauri::command]
pub async fn update_service(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    host: State<'_, ServiceHost>,
    id: String,
    mut patch: ServicePatch,
) -> AppResult<Service> {
    patch.label = clean_label(patch.label.take())?;
    advanced::validate(&mut patch)?;
    let before = db::get(&pool, &id).await?;
    if let Some(u) = patch.url.take() {
        if before.kind != ServiceKind::Custom {
            return Err(AppError::InvalidInput("sólo los servicios personalizados tienen URL".into()));
        }
        patch.url = Some(
            normalize_custom_url(&u)
                .ok_or_else(|| AppError::InvalidInput("escribe una dirección web válida (https://…)".into()))?,
        );
    }
    let zoom = patch.zoom.take();
    let mut service = db::update(&pool, &id, &patch).await?;
    if let Some(z) = zoom {
        let z = host.set_zoom(&app, &id, z)?;
        service = db::update(&pool, &id, &ServicePatch { zoom: Some(z), ..Default::default() }).await?;
    }
    host.update_meta(&app, &service);
    if service.url != before.url {
        // Otra web, otro icono.
        service = db::set_icon(&pool, &id, None).await?;
        siteicon::refresh(&app, &service);
    }
    // Agente, proxy, extensiones y CSS/JS sólo se aplican al crear el webview.
    let rebuild = service.user_agent != before.user_agent
        || service.proxy != before.proxy
        || service.extensions != before.extensions
        || service.custom_css != before.custom_css
        || service.custom_js != before.custom_js;
    if rebuild {
        host.recreate(&app, &service).await?;
    } else if service.url != before.url {
        host.go_home(&app, &service)?;
    }
    Ok(service)
}

#[tauri::command]
pub async fn reorder_services(pool: State<'_, SqlitePool>, ids: Vec<String>) -> AppResult<()> {
    db::reorder(&pool, &ids).await
}

/// Quita el servicio y borra su sesión de este equipo.
#[tauri::command]
pub async fn remove_service(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    host: State<'_, ServiceHost>,
    id: String,
) -> AppResult<()> {
    host.remove(&app, &id, true)?;
    db::delete(&pool, &id).await?;
    sync::record_removed(&app, &id).await;
    Ok(())
}

#[tauri::command]
pub async fn reload_service(app: AppHandle, host: State<'_, ServiceHost>, id: String) -> AppResult<()> {
    host.reload(&app, &id)
}

#[tauri::command]
pub async fn go_home_service(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    host: State<'_, ServiceHost>,
    id: String,
) -> AppResult<()> {
    let service = db::get(&pool, &id).await?;
    host.go_home(&app, &service)
}

#[tauri::command]
pub async fn open_in_browser(app: AppHandle, host: State<'_, ServiceHost>, id: String) -> AppResult<()> {
    host.open_in_browser(&app, &id)
}

#[tauri::command]
pub async fn unread_counts(host: State<'_, ServiceHost>) -> AppResult<HashMap<String, u32>> {
    Ok(host.unread_snapshot())
}

// ---------------------------------------------------------------------------
// Espacios de trabajo
// ---------------------------------------------------------------------------

fn clean_workspace_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 30 {
        return Err(AppError::InvalidInput("el nombre del espacio debe tener entre 1 y 30 caracteres".into()));
    }
    Ok(name.to_string())
}

#[tauri::command]
pub async fn list_workspaces(pool: State<'_, SqlitePool>) -> AppResult<Vec<Workspace>> {
    db::list_workspaces(&pool).await
}

#[tauri::command]
pub async fn create_workspace(pool: State<'_, SqlitePool>, name: String) -> AppResult<Workspace> {
    db::create_workspace(&pool, &clean_workspace_name(&name)?).await
}

#[tauri::command]
pub async fn rename_workspace(pool: State<'_, SqlitePool>, id: String, name: String) -> AppResult<()> {
    db::rename_workspace(&pool, &id, &clean_workspace_name(&name)?).await
}

#[tauri::command]
pub async fn delete_workspace(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    host: State<'_, ServiceHost>,
    id: String,
) -> AppResult<()> {
    db::delete_workspace(&pool, &id).await?;
    sync::record_removed(&app, &id).await;
    let mut s = host.settings();
    if s.active_workspace.as_deref() == Some(id.as_str()) {
        s.active_workspace = None;
        prefs::save(&app, s).await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Ajustes
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_settings(host: State<'_, ServiceHost>) -> AppResult<Settings> {
    Ok(host.settings())
}

#[tauri::command]
pub async fn update_settings(app: AppHandle, settings: Settings) -> AppResult<Settings> {
    prefs::save(&app, settings).await
}

/// Modo concentración: `minutes` desde ahora, `untilTomorrow` (8:00) o ninguno = desactivar.
#[tauri::command]
pub async fn set_focus(app: AppHandle, minutes: Option<u32>, until_tomorrow: Option<bool>) -> AppResult<Settings> {
    let until = if until_tomorrow.unwrap_or(false) {
        Some(settings::tomorrow_morning_ms())
    } else {
        minutes.map(|m| settings::now_ms() + i64::from(m) * 60_000)
    };
    prefs::set_focus(&app, until).await
}

// ---------------------------------------------------------------------------
// Avisos
// ---------------------------------------------------------------------------

/// La interfaz dibuja el logo del servicio (PNG) para los avisos de Windows.
#[tauri::command]
pub async fn set_service_icon(app: AppHandle, pool: State<'_, SqlitePool>, id: String, png: Vec<u8>) -> AppResult<()> {
    let service = db::get(&pool, &id).await?;
    toast::set_service_icon(&app, &id, &service.label, &png)?;
    Ok(())
}

#[tauri::command]
pub async fn list_notifications(app: AppHandle, host: State<'_, ServiceHost>) -> AppResult<Vec<HistoryEntry>> {
    if lock::status(&app).locked {
        return Err(AppError::InvalidInput("OmniChat está bloqueado".into()));
    }
    Ok(host.history())
}

#[tauri::command]
pub async fn clear_notifications(host: State<'_, ServiceHost>) -> AppResult<()> {
    host.clear_history();
    Ok(())
}

// ---------------------------------------------------------------------------
// Hibernación
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn hibernate_service(app: AppHandle, host: State<'_, ServiceHost>, id: String) -> AppResult<()> {
    host.hibernate(&app, &id)
}

#[tauri::command]
pub async fn list_hibernated(host: State<'_, ServiceHost>) -> AppResult<Vec<String>> {
    Ok(host.hibernated())
}

// ---------------------------------------------------------------------------
// Bloqueo
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn lock_status(app: AppHandle) -> AppResult<LockStatus> {
    Ok(lock::status(&app))
}

#[tauri::command]
pub async fn lock_now(app: AppHandle) -> AppResult<LockStatus> {
    lock::lock(&app);
    Ok(lock::status(&app))
}

#[tauri::command]
pub async fn unlock(app: AppHandle, pin: String) -> AppResult<LockStatus> {
    lock::unlock(&app, &pin).await
}

/// Crear/cambiar (`next`) o quitar (`next = null`) el PIN; `current` si ya hay uno.
#[tauri::command]
pub async fn set_pin(app: AppHandle, current: Option<String>, next: Option<String>) -> AppResult<LockStatus> {
    lock::set_pin(&app, current, next).await
}

// ---------------------------------------------------------------------------
// Copia de seguridad
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn export_config(app: AppHandle) -> AppResult<String> {
    backup::export(&app).await
}

#[tauri::command]
pub async fn import_config(app: AppHandle, json: String) -> AppResult<ImportSummary> {
    if json.len() > 2 * 1024 * 1024 {
        return Err(AppError::InvalidInput("el fichero es demasiado grande".into()));
    }
    backup::import(&app, &json).await
}

/// Vuelve a buscar el icono de una web personalizada.
#[tauri::command]
pub async fn refresh_service_icon(app: AppHandle, pool: State<'_, SqlitePool>, id: String) -> AppResult<()> {
    siteicon::refresh(&app, &db::get(&pool, &id).await?);
    Ok(())
}

// ---------------------------------------------------------------------------
// Memoria y extensiones
// ---------------------------------------------------------------------------

/// Memoria (bytes) de cada servicio abierto.
#[tauri::command]
pub async fn service_memory(pool: State<'_, SqlitePool>) -> AppResult<HashMap<String, u64>> {
    let ids: Vec<String> = db::list(&pool).await?.into_iter().map(|s| s.id).collect();
    tauri::async_runtime::spawn_blocking(move || memory::by_service(&ids))
        .await
        .map_err(|e| AppError::InvalidInput(e.to_string()))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionsInfo {
    folder: String,
    names: Vec<String>,
}

#[tauri::command]
pub async fn extensions_info(app: AppHandle) -> AppResult<ExtensionsInfo> {
    Ok(ExtensionsInfo {
        folder: advanced::extensions_dir(&app)?.display().to_string(),
        names: advanced::list_extensions(&app)?,
    })
}

#[tauri::command]
pub async fn open_extensions_folder(app: AppHandle) -> AppResult<()> {
    let dir = advanced::extensions_dir(&app)?;
    app.opener().open_path(dir.display().to_string(), None::<&str>).map_err(|e| AppError::InvalidInput(e.to_string()))
}

/// Vuelve a crear los servicios que usan extensiones (tras añadir o quitar alguna).
#[tauri::command]
pub async fn reload_extensions(app: AppHandle, pool: State<'_, SqlitePool>, host: State<'_, ServiceHost>) -> AppResult<()> {
    for s in db::list(&pool).await?.into_iter().filter(|s| s.extensions) {
        host.recreate(&app, &s).await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Sincronización
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
pub struct SyncSuggestion {
    name: String,
    path: String,
}

#[tauri::command]
pub async fn sync_suggestions() -> AppResult<Vec<SyncSuggestion>> {
    Ok(sync::suggestions().into_iter().map(|(name, path)| SyncSuggestion { name, path }).collect())
}

/// Activa (`folder`) o desactiva (`null`) la sincronización y sincroniza en el acto.
#[tauri::command]
pub async fn set_sync_folder(app: AppHandle, host: State<'_, ServiceHost>, folder: Option<String>) -> AppResult<Settings> {
    let mut s = host.settings();
    s.sync.folder = match folder.filter(|f| !f.trim().is_empty()) {
        Some(f) => Some(sync::check_folder(&f)?),
        None => None,
    };
    let saved = prefs::save(&app, s).await?;
    sync::tick(&app).await;
    Ok(saved)
}

#[tauri::command]
pub async fn sync_now(app: AppHandle) -> AppResult<()> {
    sync::tick(&app).await;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tareas
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_tasks(pool: State<'_, SqlitePool>) -> AppResult<Vec<Task>> {
    db::list_tasks(&pool).await
}

#[tauri::command]
pub async fn add_task(pool: State<'_, SqlitePool>, text: String, service_id: Option<String>) -> AppResult<Task> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 300 {
        return Err(AppError::InvalidInput("la tarea debe tener entre 1 y 300 caracteres".into()));
    }
    let service_id = match service_id.filter(|s| !s.is_empty()) {
        Some(id) => Some(db::get(&pool, &id).await?.id),
        None => None,
    };
    db::add_task(&pool, text, service_id.as_deref()).await
}

#[tauri::command]
pub async fn set_task_done(pool: State<'_, SqlitePool>, id: String, done: bool) -> AppResult<()> {
    db::set_task_done(&pool, &id, done).await
}

#[tauri::command]
pub async fn delete_task(pool: State<'_, SqlitePool>, id: String) -> AppResult<()> {
    db::delete_task(&pool, &id).await
}

#[tauri::command]
pub async fn clear_done_tasks(pool: State<'_, SqlitePool>) -> AppResult<()> {
    db::clear_done_tasks(&pool).await
}
