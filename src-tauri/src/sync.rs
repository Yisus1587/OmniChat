//! Sincronización entre equipos sin servidor propio: OmniChat escribe su configuración en
//! un fichero dentro de una carpeta que ya se sincroniza (OneDrive, Dropbox, Google Drive…)
//! y lee los cambios que dejan los demás equipos.
//!
//! * Se sincronizan servicios, espacios y preferencias; no las sesiones, el PIN, el proxy,
//!   el CSS/JS propio ni las extensiones (cada equipo los suyos).
//! * Cada servicio tiene un id de sincronización. Si un equipo ya tenía el mismo servicio
//!   (mismo tipo, nombre y dirección) se enlaza con él en vez de duplicarlo.
//! * Los borrados se propagan con una lista de ids borrados.
//! * Si dos equipos cambian lo mismo a la vez, gana el último fichero leído.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager};

use crate::catalog::{normalize_custom_url, ServiceKind};
use crate::db::{self, ServicePatch};
use crate::error::{AppError, AppResult};
use crate::host::{ServiceHost, ZOOM_MAX, ZOOM_MIN};
use crate::settings::{self, Settings};
use crate::{advanced, prefs};

const FILE: &str = "omnichat-sync.json";
const FORMAT: &str = "omnichat-sync";
const VERSION: u32 = 1;
const META_KEY: &str = "sync_state";
const MAX_FILE: u64 = 4 * 1024 * 1024;
const MAX_REMOVED: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SyncWorkspace {
    id: String,
    name: String,
    position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SyncService {
    id: String,
    kind: ServiceKind,
    label: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    workspace_id: Option<String>,
    #[serde(default)]
    position: i64,
    notify: bool,
    sound: bool,
    zoom: f64,
    hibernate: bool,
    #[serde(default)]
    user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncFile {
    format: String,
    version: u32,
    device: String,
    updated_at: i64,
    workspaces: Vec<SyncWorkspace>,
    services: Vec<SyncService>,
    #[serde(default)]
    removed: Vec<String>,
    settings: Settings,
}

/// Estado local de la sincronización (en la tabla `settings`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Meta {
    device: String,
    /// id local → id de sincronización (cuando difieren).
    aliases: HashMap<String, String>,
    /// ids de sincronización borrados.
    removed: Vec<String>,
    /// Huella del último contenido escrito o leído.
    last_hash: Option<String>,
}

impl Meta {
    fn sync_id(&self, local: &str) -> String {
        self.aliases.get(local).cloned().unwrap_or_else(|| local.to_string())
    }

    fn local_id(&self, sync: &str) -> String {
        self.aliases.iter().find(|(_, s)| *s == sync).map(|(l, _)| l.clone()).unwrap_or_else(|| sync.to_string())
    }
}

/// Fecha del fichero leído por última vez (para no releerlo si no cambia).
static LAST_MTIME: Mutex<Option<SystemTime>> = Mutex::new(None);
/// Evita dos sincronizaciones a la vez.
static RUNNING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Los ids vienen de un fichero externo y se usan como nombre de carpeta: sólo 32 hex.
fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Preferencias que se comparten (las propias de cada equipo se conservan).
fn shared_settings(mut s: Settings, local: &Settings) -> Settings {
    s.sync = local.sync.clone();
    s.focus_until = local.focus_until;
    s.active_workspace = local.active_workspace.clone();
    s.autostart = local.autostart;
    s
}

fn hash(file: &SyncFile) -> String {
    let mut f = file.clone();
    f.device.clear();
    f.updated_at = 0;
    f.services.sort_by(|a, b| a.id.cmp(&b.id));
    f.workspaces.sort_by(|a, b| a.id.cmp(&b.id));
    f.removed.sort();
    f.settings = shared_settings(f.settings, &Settings::default());
    let json = serde_json::to_vec(&f).unwrap_or_default();
    Sha256::digest(&json).iter().map(|b| format!("{b:02x}")).collect()
}

async fn load_meta(pool: &SqlitePool) -> Meta {
    let mut m: Meta = db::get_setting(pool, META_KEY).await.unwrap_or_default();
    if m.device.is_empty() {
        m.device = uuid::Uuid::new_v4().simple().to_string();
    }
    m
}

async fn save_meta(pool: &SqlitePool, m: &Meta) -> AppResult<()> {
    db::set_setting(pool, META_KEY, m).await
}

fn folder(app: &AppHandle) -> Option<PathBuf> {
    app.state::<ServiceHost>().settings().sync.folder.map(PathBuf::from).filter(|p| p.is_dir())
}

/// Comprueba la carpeta elegida y la crea si hace falta (su carpeta padre debe existir).
pub fn check_folder(path: &str) -> AppResult<String> {
    let p = Path::new(path.trim());
    if !p.is_absolute() {
        return Err(AppError::InvalidInput("indica una ruta completa, p. ej. C:\\Users\\tú\\OneDrive\\OmniChat".into()));
    }
    if !p.exists() {
        if !p.parent().is_some_and(Path::is_dir) {
            return Err(AppError::InvalidInput("esa carpeta no existe".into()));
        }
        std::fs::create_dir(p)?;
    }
    if !p.is_dir() {
        return Err(AppError::InvalidInput("la ruta no es una carpeta".into()));
    }
    Ok(p.display().to_string())
}

/// Carpetas sincronizadas habituales en este equipo, con la subcarpeta OmniChat.
pub fn suggestions() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    let candidates = [
        ("OneDrive", std::env::var("OneDrive").ok()),
        ("OneDrive", std::env::var("OneDriveConsumer").ok()),
        ("OneDrive (trabajo)", std::env::var("OneDriveCommercial").ok()),
        ("Dropbox", Some(format!("{home}\\Dropbox"))),
        ("Google Drive", Some("G:\\Mi unidad".to_string())),
        ("Google Drive", Some("G:\\My Drive".to_string())),
        ("iCloud Drive", Some(format!("{home}\\iCloudDrive"))),
    ];
    for (name, path) in candidates {
        let Some(path) = path.filter(|p| !p.is_empty() && Path::new(p).is_dir()) else { continue };
        if seen.insert(path.to_lowercase()) {
            out.push((name.to_string(), format!("{path}\\OmniChat")));
        }
    }
    out
}

async fn snapshot(app: &AppHandle, meta: &Meta) -> AppResult<SyncFile> {
    let pool = app.state::<SqlitePool>();
    Ok(SyncFile {
        format: FORMAT.into(),
        version: VERSION,
        device: meta.device.clone(),
        updated_at: settings::now_ms(),
        workspaces: db::list_workspaces(&pool)
            .await?
            .into_iter()
            .map(|w| SyncWorkspace { id: meta.sync_id(&w.id), name: w.name, position: w.position })
            .collect(),
        services: db::list(&pool)
            .await?
            .into_iter()
            .map(|s| SyncService {
                id: meta.sync_id(&s.id),
                kind: s.kind,
                label: s.label,
                url: s.url,
                workspace_id: s.workspace_id.map(|w| meta.sync_id(&w)),
                position: s.position,
                notify: s.notify,
                sound: s.sound,
                zoom: s.zoom,
                hibernate: s.hibernate,
                user_agent: s.user_agent,
            })
            .collect(),
        removed: meta.removed.clone(),
        settings: app.state::<ServiceHost>().settings(),
    })
}

/// Aplica el fichero de otro equipo a la base de datos local.
async fn apply_remote(app: &AppHandle, file: &SyncFile, meta: &mut Meta) -> AppResult<()> {
    let pool = app.state::<SqlitePool>();
    let host = app.state::<ServiceHost>();

    // Borrados.
    let mut removed: Vec<String> = meta.removed.clone();
    for id in &file.removed {
        if valid_id(id) && !removed.contains(id) {
            removed.push(id.clone());
        }
    }
    let gone: HashSet<String> = removed.iter().cloned().collect();
    for s in db::list(&pool).await? {
        if gone.contains(&meta.sync_id(&s.id)) {
            host.remove(app, &s.id, true)?;
            db::delete(&pool, &s.id).await?;
            meta.aliases.remove(&s.id);
        }
    }
    for w in db::list_workspaces(&pool).await? {
        if gone.contains(&meta.sync_id(&w.id)) {
            db::delete_workspace(&pool, &w.id).await?;
            meta.aliases.remove(&w.id);
        }
    }
    let excess = removed.len().saturating_sub(MAX_REMOVED);
    removed.drain(..excess);
    meta.removed = removed;

    // Espacios: por id, o por nombre si es la primera vez que se ven.
    let remote_ws: HashSet<&str> = file.workspaces.iter().map(|w| w.id.as_str()).collect();
    let mut local_ws = db::list_workspaces(&pool).await?;
    for w in &file.workspaces {
        let name = w.name.trim();
        if !valid_id(&w.id) || gone.contains(&w.id) || name.is_empty() || name.chars().count() > 30 {
            continue;
        }
        let local = meta.local_id(&w.id);
        if let Some(l) = local_ws.iter().find(|l| l.id == local) {
            if l.name != name {
                db::rename_workspace(&pool, &l.id, name).await?;
            }
        } else if let Some(l) = local_ws.iter().find(|l| {
            l.name.eq_ignore_ascii_case(name) && !remote_ws.contains(meta.sync_id(&l.id).as_str())
        }) {
            meta.aliases.insert(l.id.clone(), w.id.clone());
        } else {
            db::create_workspace_with_id(&pool, &w.id, name).await?;
            local_ws = db::list_workspaces(&pool).await?;
        }
    }

    // Servicios: igual (por id, o mismo tipo + nombre + dirección).
    let remote_ids: HashSet<&str> = file.services.iter().map(|s| s.id.as_str()).collect();
    for f in &file.services {
        if !valid_id(&f.id) || gone.contains(&f.id) {
            continue;
        }
        let label: String = f.label.trim().chars().take(40).collect();
        let label = if label.is_empty() { f.kind.name().to_string() } else { label };
        let url = match f.kind {
            ServiceKind::Custom => match f.url.as_deref().and_then(normalize_custom_url) {
                Some(u) => Some(u),
                None => continue,
            },
            _ => None,
        };
        let local_list = db::list(&pool).await?;
        let local = meta.local_id(&f.id);
        let existing = match local_list.iter().find(|s| s.id == local) {
            Some(s) => Some(s.clone()),
            None => {
                let twin = local_list.iter().find(|s| {
                    s.kind == f.kind && s.label == label && s.url == url && !remote_ids.contains(meta.sync_id(&s.id).as_str())
                });
                if let Some(t) = twin {
                    meta.aliases.insert(t.id.clone(), f.id.clone());
                }
                twin.cloned()
            }
        };
        let ws = f
            .workspace_id
            .as_deref()
            .map(|w| meta.local_id(w))
            .filter(|w| local_ws.iter().any(|l| &l.id == w));
        let mut patch = ServicePatch {
            label: Some(label.clone()),
            notify: Some(f.notify),
            sound: Some(f.sound),
            zoom: Some(f.zoom.clamp(ZOOM_MIN, ZOOM_MAX)),
            hibernate: Some(f.hibernate),
            workspace_id: Some(ws.clone().unwrap_or_default()),
            user_agent: Some(f.user_agent.clone().unwrap_or_default()),
            url: url.clone(),
            ..Default::default()
        };
        if advanced::validate(&mut patch).is_err() {
            patch.user_agent = None;
        }
        let before = match existing {
            Some(s) => s,
            None => db::create_with_id(&pool, &f.id, f.kind, &label, url.as_deref(), ws.as_deref()).await?,
        };
        let after = db::update(&pool, &before.id, &patch).await?;
        db::set_position(&pool, &after.id, f.position).await?;
        if after.url != before.url || after.user_agent != before.user_agent {
            // Los datos ya están guardados: un webview que no arranca no debe parar la
            // sincronización (se vuelve a crear al recolocar los servicios).
            if let Err(e) = host.recreate(app, &after).await {
                tracing::warn!(service = %after.id, error = %e, "sync: could not recreate webview");
            }
        } else {
            host.update_meta(app, &after);
        }
    }

    // Preferencias compartidas.
    let current = host.settings();
    let merged = shared_settings(file.settings.clone(), &current);
    if merged != current && merged.validate().is_ok() {
        prefs::save(app, merged).await?;
    }
    Ok(())
}

/// Lee los cambios de otros equipos y escribe los propios. Se llama cada 30 s.
pub async fn tick(app: &AppHandle) {
    let Ok(_guard) = RUNNING.try_lock() else { return };
    let Some(dir) = folder(app) else { return };
    if let Err(e) = run(app, &dir).await {
        tracing::warn!(error = %e, "sync failed");
        let _ = app.emit("sync:error", e.to_string());
    }
}

/// Fecha del fichero, o `None` si no existe.
fn modified(path: &Path) -> AppResult<Option<SystemTime>> {
    match std::fs::metadata(path) {
        Ok(md) => Ok(Some(md.modified()?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Lee el fichero compartido (`None` si no existe). Si está a medio descargar, dañado o es de
/// una versión más nueva devuelve un error, para no sobrescribir lo que no se entiende.
fn read_file(path: &Path) -> AppResult<Option<SyncFile>> {
    #[derive(Deserialize)]
    struct Header {
        format: String,
        version: u32,
    }
    let bad = |m: &str| AppError::InvalidInput(m.into());

    match std::fs::metadata(path) {
        Ok(md) if md.len() > MAX_FILE => return Err(bad("el fichero compartido es demasiado grande")),
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let bytes = std::fs::read(path)?;
    let unreadable = || bad("el fichero compartido está incompleto o dañado");
    let header: Header = serde_json::from_slice(&bytes).map_err(|_| unreadable())?;
    if header.format != FORMAT {
        return Err(unreadable());
    }
    if header.version > VERSION {
        return Err(bad("el fichero compartido es de una versión más nueva de OmniChat; actualiza la app"));
    }
    serde_json::from_slice(&bytes).map(Some).map_err(|_| unreadable())
}

async fn run(app: &AppHandle, dir: &Path) -> AppResult<()> {
    let pool = app.state::<SqlitePool>();
    let mut meta = load_meta(&pool).await;
    let path = dir.join(FILE);

    // 1. Leer. Si algo falla se sale sin escribir y se reintenta en la próxima pasada: el
    // fichero sólo se da por leído (`LAST_MTIME`) cuando sus cambios están aplicados.
    let seen = modified(&path)?;
    let changed = *LAST_MTIME.lock().unwrap() != seen;
    if changed {
        if let Some(file) = read_file(&path)? {
            let h = hash(&file);
            if meta.last_hash.as_deref() != Some(h.as_str()) && file.device != meta.device {
                let applied = apply_remote(app, &file, &mut meta).await;
                if applied.is_ok() {
                    meta.last_hash = Some(h);
                }
                // Aunque falle a mitad, se conservan los enlaces y borrados ya aplicados.
                save_meta(&pool, &meta).await?;
                let _ = app.emit("config:changed", ());
                applied?;
                tracing::info!("sync: applied changes from another device");
            }
        }
        *LAST_MTIME.lock().unwrap() = seen;
    }

    // 2. Escribir si lo local cambió.
    let snap = snapshot(app, &meta).await?;
    let h = hash(&snap);
    if meta.last_hash.as_deref() != Some(h.as_str()) {
        let json = serde_json::to_vec_pretty(&snap).map_err(|e| AppError::InvalidInput(e.to_string()))?;
        let tmp = dir.join(format!("{FILE}.{}.tmp", &meta.device[..8]));
        std::fs::write(&tmp, json)?;
        if modified(&path)? != seen {
            // Otro equipo acaba de escribir: primero hay que leer sus cambios.
            let _ = std::fs::remove_file(&tmp);
            return Ok(());
        }
        std::fs::rename(&tmp, &path)?;
        *LAST_MTIME.lock().unwrap() = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        meta.last_hash = Some(h);
        save_meta(&pool, &meta).await?;
    }
    Ok(())
}

/// Al borrar un servicio o espacio, se anota para borrarlo también en los demás equipos.
pub async fn record_removed(app: &AppHandle, local_id: &str) {
    if app.state::<ServiceHost>().settings().sync.folder.is_none() {
        return;
    }
    // Una sincronización en curso guardaría su copia del estado y perdería este borrado.
    let _guard = RUNNING.lock().await;
    let pool = app.state::<SqlitePool>();
    let mut meta = load_meta(&pool).await;
    let id = meta.sync_id(local_id);
    meta.aliases.remove(local_id);
    if !meta.removed.contains(&id) {
        meta.removed.push(id);
    }
    let _ = save_meta(&pool, &meta).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_hash() {
        assert!(valid_id("0123456789abcdef0123456789abcdef"));
        assert!(!valid_id("../../../../Windows/System32/xx"));
        assert!(!valid_id("0123456789ABCDEF0123456789ABCDEF"));

        let a = SyncFile {
            format: FORMAT.into(),
            version: 1,
            device: "a".into(),
            updated_at: 1,
            workspaces: vec![],
            services: vec![],
            removed: vec!["b".into(), "a".into()],
            settings: Settings::default(),
        };
        let mut b = a.clone();
        b.device = "b".into();
        b.updated_at = 99;
        b.removed = vec!["a".into(), "b".into()];
        b.settings.autostart = true; // propio de cada equipo: no cuenta
        assert_eq!(hash(&a), hash(&b));
        b.settings.hide_content = true;
        assert_ne!(hash(&a), hash(&b));
    }

    #[test]
    fn reads_only_what_it_understands() {
        let dir = std::env::temp_dir().join(format!("omnichat-test-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join(FILE);
        assert!(modified(&path).unwrap().is_none());
        assert!(read_file(&path).unwrap().is_none());

        let file = SyncFile {
            format: FORMAT.into(),
            version: VERSION,
            device: "a".into(),
            updated_at: 1,
            workspaces: vec![],
            services: vec![],
            removed: vec![],
            settings: Settings::default(),
        };
        let good = serde_json::to_vec(&file).unwrap();
        std::fs::write(&path, &good).unwrap();
        assert!(modified(&path).unwrap().is_some());
        assert_eq!(hash(&read_file(&path).unwrap().unwrap()), hash(&file));

        // A medio descargar, vacío, ajeno o de una versión futura (con otra estructura).
        let cases: [&[u8]; 4] = [
            &good[..good.len() / 2],
            b"",
            br#"{"format":"otra-cosa","version":1}"#,
            br#"{"format":"omnichat-sync","version":2,"nuevo":true}"#,
        ];
        for bytes in cases {
            std::fs::write(&path, bytes).unwrap();
            assert!(read_file(&path).is_err());
        }
        assert!(read_file(&path).unwrap_err().to_string().contains("más nueva"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
