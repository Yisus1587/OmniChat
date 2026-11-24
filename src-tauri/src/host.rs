//! Gestor de webviews de servicios.
//!
//! Cada servicio es un webview hijo de la ventana principal, colocado sobre el área de
//! contenido que reserva la UI (a la derecha de la barra lateral). Cada uno tiene su propio
//! directorio de datos → cookies y sesiones aisladas (permite varias cuentas del mismo servicio).
//!
//! Seguridad: las webs de terceros NO tienen acceso IPC (ninguna capability apunta a ellas).
//! Los enlaces que abren ventana nueva se mandan al navegador del sistema.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::webview::{NewWindowResponse, WebviewBuilder};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Url, WebviewUrl};
use tauri_plugin_opener::OpenerExt;

use crate::advanced;
use crate::catalog::{unread_from_title, ServiceKind};
use crate::db::Service;
use crate::error::{AppError, AppResult};
use crate::lock::LockState;
use crate::settings::{self, Settings};
use crate::toast::{self, Notice};
use crate::tray;

pub const MAIN_WINDOW: &str = "main";

/// Script que redirige las notificaciones de cada web hacia OmniChat (ver el propio fichero).
const NOTIFY_SHIM: &str = include_str!("notify_shim.js");
/// Marcador con el que el script codifica un aviso en `document.title`.
const NOTIFY_MARK: &str = "\u{2063}omnichat-notify:";
/// Un mismo aviso repetido dentro de este margen se descarta.
const DEDUPE_WINDOW: Duration = Duration::from_secs(6);

/// Límite de avisos por servicio: una web no puede inundar el escritorio ni OmniChat.
const RATE_WINDOW: Duration = Duration::from_secs(10);
/// Avisos de escritorio por servicio dentro de `RATE_WINDOW`.
const RATE_TOASTS: usize = 5;
/// Avisos registrados (centro de avisos) por servicio dentro de `RATE_WINDOW`.
const RATE_RECORDED: usize = 20;

/// Avisos que guarda el centro de notificaciones (sólo en memoria, durante la sesión).
const HISTORY_MAX: usize = 150;

pub const ZOOM_MIN: f64 = 0.5;
pub const ZOOM_MAX: f64 = 2.0;

/// Área (en píxeles lógicos / CSS) donde se pintan los servicios.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UnreadPayload<'a> {
    id: &'a str,
    count: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomPayload {
    pub id: String,
    pub zoom: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HibernatedPayload<'a> {
    id: &'a str,
    hibernated: bool,
}

/// Una entrada del centro de notificaciones.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: u64,
    pub service_id: String,
    pub title: String,
    pub body: String,
    /// epoch ms
    pub at: i64,
    /// No se mostró en el escritorio (concentración, horario o avisos desactivados).
    pub muted: bool,
}

/// Aviso enviado por `notify_shim.js`.
#[derive(Debug, Deserialize)]
struct WebNotification {
    #[serde(default)]
    t: String,
    #[serde(default)]
    b: String,
}

/// Lo que el host necesita saber de cada servicio para notificar.
#[derive(Debug, Clone)]
struct Meta {
    label: String,
    kind: ServiceKind,
    notify: bool,
    sound: bool,
    zoom: f64,
    hibernate: bool,
}

struct HostState {
    bounds: Option<Bounds>,
    active: Option<String>,
    /// Un diálogo de la UI está abierto: ocultamos los webviews para que no lo tapen.
    overlay: bool,
    unread: HashMap<String, u32>,
    meta: HashMap<String, Meta>,
    /// Servicios que ya han enviado avisos propios (con remitente y texto): para ellos no
    /// mostramos el aviso genérico "N sin leer".
    web_notifies: HashSet<String>,
    /// Últimos avisos mostrados, para no repetirlos.
    recent: Vec<(String, Instant)>,
    /// Momentos de los últimos avisos de cada servicio (límite de frecuencia).
    rate: HashMap<String, VecDeque<Instant>>,
    settings: Settings,
    history: VecDeque<HistoryEntry>,
    next_entry: u64,
    /// Servicios cerrados para ahorrar memoria; se vuelven a abrir al seleccionarlos.
    hibernated: HashSet<String>,
    /// Última vez que cada servicio estuvo en pantalla.
    last_used: HashMap<String, Instant>,
    started: Instant,
    lock: LockState,
}

impl HostState {
    fn new(settings: Settings) -> Self {
        Self {
            bounds: None,
            active: None,
            overlay: false,
            unread: HashMap::new(),
            meta: HashMap::new(),
            web_notifies: HashSet::new(),
            recent: Vec::new(),
            rate: HashMap::new(),
            settings,
            history: VecDeque::new(),
            next_entry: 1,
            hibernated: HashSet::new(),
            last_used: HashMap::new(),
            started: Instant::now(),
            lock: LockState::default(),
        }
    }
}

pub struct ServiceHost {
    data_root: PathBuf,
    state: Mutex<HostState>,
    /// Serializa la creación de webviews: dos `sync_layout` simultáneos no deben crear
    /// el mismo servicio dos veces.
    create_lock: Mutex<()>,
}

fn label_for(id: &str) -> String {
    format!("svc-{id}")
}

impl ServiceHost {
    pub fn new(data_root: PathBuf, settings: Settings) -> Self {
        Self {
            data_root,
            state: Mutex::new(HostState::new(settings)),
            create_lock: Mutex::default(),
        }
    }

    fn data_dir(&self, id: &str) -> PathBuf {
        self.data_root.join(id)
    }

    /// Crea el webview del servicio si aún no existe. Debe llamarse desde un comando
    /// `async` (en Windows `add_child` bloquea si se invoca desde el hilo principal).
    pub fn ensure(&self, app: &AppHandle, service: &Service) -> AppResult<()> {
        self.update_meta(app, service);
        let label = label_for(&service.id);
        let _guard = self.create_lock.lock().unwrap();
        if app.get_webview(&label).is_some() {
            return Ok(());
        }
        // Un servicio hibernado sólo se despierta cuando el usuario lo abre.
        let woke = {
            let mut st = self.state.lock().unwrap();
            if st.hibernated.contains(&service.id) {
                if st.active.as_deref() != Some(service.id.as_str()) {
                    return Ok(());
                }
                st.hibernated.remove(&service.id);
                true
            } else {
                false
            }
        };
        if woke {
            let _ = app.emit("service:hibernated", HibernatedPayload { id: &service.id, hibernated: false });
        }

        let window = app
            .get_window(MAIN_WINDOW)
            .ok_or_else(|| AppError::NotFound("ventana principal".into()))?;
        let url: Url = service
            .home_url()
            .and_then(|u| u.parse().ok())
            .ok_or_else(|| AppError::InvalidInput(format!("URL no válida para «{}»", service.label)))?;

        let id_title = service.id.clone();
        let app_title = app.clone();
        let app_popup = app.clone();

        let mut builder = WebviewBuilder::new(&label, WebviewUrl::External(url))
            .data_directory(self.data_dir(&service.id))
            .initialization_script(NOTIFY_SHIM)
            // Arrastrar archivos a WhatsApp, Gmail… lo gestiona la propia web (HTML5).
            .disable_drag_drop_handler()
            // Ctrl + / − / 0 los gestiona OmniChat para guardar el zoom por servicio.
            .zoom_hotkeys_enabled(false)
            .on_document_title_changed(move |_webview, title| {
                let host = app_title.state::<ServiceHost>();
                host.on_title(&app_title, &id_title, &title);
            })
            .on_new_window(move |url, _features| {
                // Enlaces externos → navegador del sistema. `about:blank` y `blob:` (vistas de
                // impresión o adjuntos) se abren dentro. Cualquier otro esquema (file:, ms-*,
                // protocolos de otras apps…) se bloquea: lo decide una web de terceros.
                match url.scheme() {
                    "http" | "https" | "mailto" | "tel" => {
                        let _ = app_popup.opener().open_url(url.as_str(), None::<&str>);
                        NewWindowResponse::Deny
                    }
                    "about" | "blob" => NewWindowResponse::Allow,
                    other => {
                        tracing::info!(scheme = other, "blocked new window");
                        NewWindowResponse::Deny
                    }
                }
            });

        // Ajustes avanzados (ver `advanced.rs`).
        if let Some(ua) = advanced::user_agent(service) {
            builder = builder.user_agent(ua);
        }
        if let Some(proxy) = service.proxy.as_deref().and_then(|p| advanced::parse_proxy(p).ok()) {
            builder = builder.proxy_url(proxy);
        }
        if service.extensions {
            builder = builder.browser_extensions_enabled(true).extensions_path(advanced::extensions_dir(app)?);
        }
        if let Some(script) = advanced::user_script(service) {
            builder = builder.initialization_script(script);
        }

        let (pos, size) = {
            let st = self.state.lock().unwrap();
            match st.bounds {
                Some(b) => (LogicalPosition::new(b.x, b.y), LogicalSize::new(b.width, b.height)),
                None => (LogicalPosition::new(0.0, 0.0), LogicalSize::new(1.0, 1.0)),
            }
        };
        let webview = window.add_child(builder, pos, size)?;
        webview.hide()?;
        if (service.zoom - 1.0).abs() > f64::EPSILON {
            webview.set_zoom(service.zoom)?;
        }
        tracing::info!(service = %service.id, kind = ?service.kind, "webview created");
        Ok(())
    }

    /// Vuelve a crear el webview para aplicar ajustes que sólo se leen al crearlo (agente,
    /// proxy, extensiones, CSS/JS). WebView2 tarda un momento en cerrar el anterior.
    pub async fn recreate(&self, app: &AppHandle, service: &Service) -> AppResult<()> {
        if self.state.lock().unwrap().hibernated.contains(&service.id) {
            self.update_meta(app, service);
            return Ok(()); // se aplicará al despertarlo
        }
        if let Some(w) = app.get_webview(&label_for(&service.id)) {
            w.close()?;
        }
        let mut last = Ok(());
        for _ in 0..8 {
            tokio::time::sleep(Duration::from_millis(600)).await;
            last = self.ensure(app, service);
            if last.is_ok() {
                break;
            }
        }
        last?;
        self.apply(app)
    }

    /// Recoloca y muestra/oculta todos los webviews según el estado actual.
    pub fn apply(&self, app: &AppHandle) -> AppResult<()> {
        let (bounds, active, overlay) = {
            let st = self.state.lock().unwrap();
            // Bloqueado: ningún servicio a la vista.
            (st.bounds, st.active.clone(), st.overlay || st.lock.locked)
        };
        for (label, webview) in app.webviews() {
            let Some(id) = label.strip_prefix("svc-") else { continue };
            let visible = !overlay && active.as_deref() == Some(id) && bounds.is_some();
            if visible {
                let b = bounds.expect("checked");
                webview.set_position(LogicalPosition::new(b.x, b.y))?;
                webview.set_size(LogicalSize::new(b.width, b.height))?;
                webview.show()?;
                webview.set_focus()?;
            } else {
                webview.hide()?;
            }
        }
        Ok(())
    }

    pub fn set_bounds(&self, bounds: Bounds) {
        self.state.lock().unwrap().bounds = Some(bounds);
    }

    pub fn set_active(&self, id: Option<String>) {
        let mut st = self.state.lock().unwrap();
        if st.active != id {
            let now = Instant::now();
            for s in [st.active.clone(), id.clone()].into_iter().flatten() {
                st.last_used.insert(s, now);
            }
            st.active = id;
        }
    }

    pub fn set_overlay(&self, overlay: bool) {
        self.state.lock().unwrap().overlay = overlay;
    }

    pub fn settings(&self) -> Settings {
        self.state.lock().unwrap().settings.clone()
    }

    pub fn set_settings(&self, settings: Settings) {
        self.state.lock().unwrap().settings = settings;
    }

    pub fn service_ids(&self) -> Vec<String> {
        self.state.lock().unwrap().meta.keys().cloned().collect()
    }

    pub fn with_lock<R>(&self, f: impl FnOnce(&mut LockState) -> R) -> R {
        f(&mut self.state.lock().unwrap().lock)
    }

    pub fn update_meta(&self, app: &AppHandle, s: &Service) {
        let meta = Meta {
            label: s.label.clone(),
            kind: s.kind,
            notify: s.notify,
            sound: s.sound,
            zoom: s.zoom,
            hibernate: s.hibernate,
        };
        let old = self.state.lock().unwrap().meta.insert(s.id.clone(), meta);
        // Nombre con el que Windows muestra sus avisos («WhatsApp · OmniChat»).
        if old.is_none_or(|m| m.label != s.label) {
            toast::register_service(app, &s.id, &s.label);
        }
    }

    // ---- Centro de notificaciones ---------------------------------------------

    pub fn history(&self) -> Vec<HistoryEntry> {
        self.state.lock().unwrap().history.iter().cloned().collect()
    }

    pub fn clear_history(&self) {
        self.state.lock().unwrap().history.clear();
    }

    // ---- Hibernación ------------------------------------------------------------

    pub fn hibernated(&self) -> Vec<String> {
        self.state.lock().unwrap().hibernated.iter().cloned().collect()
    }

    /// Cierra el webview del servicio para liberar memoria (su sesión se conserva).
    pub fn hibernate(&self, app: &AppHandle, id: &str) -> AppResult<()> {
        let _guard = self.create_lock.lock().unwrap();
        {
            let mut st = self.state.lock().unwrap();
            if st.active.as_deref() == Some(id) {
                return Err(AppError::InvalidInput("abre otro servicio antes de hibernar este".into()));
            }
            if !st.hibernated.insert(id.to_string()) {
                return Ok(());
            }
            st.unread.remove(id);
            st.web_notifies.remove(id);
        }
        if let Some(w) = app.get_webview(&label_for(id)) {
            w.close()?;
        }
        tracing::info!(service = %id, "service hibernated");
        let _ = app.emit("service:unread", UnreadPayload { id, count: 0 });
        let _ = app.emit("service:hibernated", HibernatedPayload { id, hibernated: true });
        self.refresh_totals(app);
        Ok(())
    }

    /// Hiberna los servicios que llevan un rato sin abrirse (si está activado).
    pub fn hibernate_idle(&self, app: &AppHandle) {
        let candidates: Vec<String> = {
            let st = self.state.lock().unwrap();
            if !st.settings.hibernation.enabled {
                return;
            }
            let limit = Duration::from_secs(u64::from(st.settings.hibernation.minutes) * 60);
            st.meta
                .iter()
                .filter(|(id, m)| {
                    m.hibernate
                        && st.active.as_deref() != Some(id.as_str())
                        && !st.hibernated.contains(*id)
                        && st.last_used.get(*id).copied().unwrap_or(st.started).elapsed() >= limit
                })
                .map(|(id, _)| id.clone())
                .collect()
        };
        for id in candidates {
            if let Err(e) = self.hibernate(app, &id) {
                tracing::warn!(service = %id, error = %e, "could not hibernate");
            }
        }
    }

    pub fn unread_snapshot(&self) -> HashMap<String, u32> {
        self.state.lock().unwrap().unread.clone()
    }

    pub fn reload(&self, app: &AppHandle, id: &str) -> AppResult<()> {
        if let Some(w) = app.get_webview(&label_for(id)) {
            w.reload()?;
        }
        Ok(())
    }

    /// Vuelve a la página inicial del servicio (útil si una web se queda en otra URL).
    pub fn go_home(&self, app: &AppHandle, service: &Service) -> AppResult<()> {
        if let (Some(w), Some(url)) = (app.get_webview(&label_for(&service.id)), service.home_url()) {
            w.navigate(url.parse().map_err(|_| AppError::InvalidInput("URL no válida".into()))?)?;
        }
        Ok(())
    }

    /// Abre en el navegador del sistema la página que se está viendo.
    pub fn open_in_browser(&self, app: &AppHandle, id: &str) -> AppResult<()> {
        let w = app
            .get_webview(&label_for(id))
            .ok_or_else(|| AppError::NotFound(format!("servicio {id}")))?;
        let url = w.url()?;
        // La web decide su URL: nunca pasar al sistema esquemas como file: o ms-*.
        if !matches!(url.scheme(), "http" | "https") {
            return Err(AppError::InvalidInput("esta página no se puede abrir en el navegador".into()));
        }
        app.opener()
            .open_url(url.as_str(), None::<&str>)
            .map_err(|e| AppError::InvalidInput(e.to_string()))
    }

    /// Aplica un zoom (acotado) a un servicio. Devuelve el valor final.
    pub fn set_zoom(&self, app: &AppHandle, id: &str, zoom: f64) -> AppResult<f64> {
        let zoom = (zoom.clamp(ZOOM_MIN, ZOOM_MAX) * 100.0).round() / 100.0;
        if let Some(w) = app.get_webview(&label_for(id)) {
            w.set_zoom(zoom)?;
        }
        if let Some(m) = self.state.lock().unwrap().meta.get_mut(id) {
            m.zoom = zoom;
        }
        let _ = app.emit("service:zoom", ZoomPayload { id: id.to_string(), zoom });
        Ok(zoom)
    }

    /// Zoom del servicio activo: `delta` relativo, o `None` para volver al 100 %.
    pub fn zoom_active(&self, app: &AppHandle, delta: Option<f64>) -> Option<(String, f64)> {
        let (id, current) = {
            let st = self.state.lock().unwrap();
            let id = st.active.clone()?;
            let current = st.meta.get(&id).map(|m| m.zoom).unwrap_or(1.0);
            (id, current)
        };
        let target = delta.map(|d| current + d).unwrap_or(1.0);
        self.set_zoom(app, &id, target).ok().map(|z| (id, z))
    }

    /// Cierra el webview y, si `wipe`, borra su sesión (cookies, caché…) del disco.
    pub fn remove(&self, app: &AppHandle, id: &str, wipe: bool) -> AppResult<()> {
        if let Some(w) = app.get_webview(&label_for(id)) {
            w.close()?;
        }
        {
            let mut st = self.state.lock().unwrap();
            st.unread.remove(id);
            st.meta.remove(id);
            st.web_notifies.remove(id);
            st.rate.remove(id);
            st.hibernated.remove(id);
            st.last_used.remove(id);
            st.history.retain(|e| e.service_id != id);
            if st.active.as_deref() == Some(id) {
                st.active = None;
            }
        }
        if wipe {
            toast::forget_service(app, id);
        }
        self.refresh_totals(app);
        if wipe {
            // WebView2 tarda un momento en soltar los ficheros tras cerrarse.
            let dir = self.data_dir(id);
            tauri::async_runtime::spawn(async move {
                for _ in 0..10 {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    if !dir.exists() || std::fs::remove_dir_all(&dir).is_ok() {
                        return;
                    }
                }
                tracing::warn!(dir = %dir.display(), "could not delete service data");
            });
        }
        Ok(())
    }

    fn on_title(&self, app: &AppHandle, id: &str, title: &str) {
        if let Some(json) = title.strip_prefix(NOTIFY_MARK) {
            if let Ok(n) = serde_json::from_str::<WebNotification>(json) {
                self.on_web_notification(app, id, n);
            }
            return; // título temporal: no es el contador
        }

        let count = unread_from_title(title);
        let (previous, meta, is_active, has_web_notifies) = {
            let mut st = self.state.lock().unwrap();
            let previous = st.unread.insert(id.to_string(), count).unwrap_or(0);
            (previous, st.meta.get(id).cloned(), st.active.as_deref() == Some(id), st.web_notifies.contains(id))
        };
        if previous == count {
            return;
        }
        let _ = app.emit("service:unread", UnreadPayload { id, count });
        self.refresh_totals(app);

        // Respaldo para webs que no envían avisos propios: "N sin leer".
        let Some(meta) = meta else { return };
        if !has_web_notifies && count > previous && !self.user_is_looking(app, is_active) {
            let title = if count == 1 {
                "1 mensaje sin leer".to_string()
            } else {
                format!("{count} mensajes sin leer")
            };
            self.notify(app, id, &meta, title, String::new());
        }
    }

    fn on_web_notification(&self, app: &AppHandle, id: &str, n: WebNotification) {
        let (meta, is_active) = {
            let mut st = self.state.lock().unwrap();
            st.web_notifies.insert(id.to_string());
            (st.meta.get(id).cloned(), st.active.as_deref() == Some(id))
        };
        let Some(meta) = meta else { return };
        if self.user_is_looking(app, is_active) {
            return;
        }
        // La web puede saltarse el script y escribir el título directamente: acotar aquí.
        let sender: String = n.t.trim().chars().take(200).collect();
        let body: String = n.b.trim().chars().take(500).collect();
        let title = if sender.is_empty() { meta.label.clone() } else { sender };
        self.notify(app, id, &meta, title, body);
    }

    fn user_is_looking(&self, app: &AppHandle, is_active: bool) -> bool {
        is_active
            && !self.state.lock().unwrap().lock.locked
            && app
                .get_window(MAIN_WINDOW)
                .map(|w| w.is_focused().unwrap_or(false) && w.is_visible().unwrap_or(false))
                .unwrap_or(false)
    }

    /// Guarda el aviso en el centro de notificaciones y, si procede, lo muestra en el
    /// escritorio: sin duplicados, respetando concentración, horario, privacidad y bloqueo.
    fn notify(&self, app: &AppHandle, id: &str, meta: &Meta, title: String, body: String) {
        let (entry, show, hide_content) = {
            let mut st = self.state.lock().unwrap();
            let key = format!("{id}\u{0}{title}\u{0}{body}");
            let now = Instant::now();
            st.recent.retain(|(_, at)| now.duration_since(*at) < DEDUPE_WINDOW);
            if st.recent.iter().any(|(k, _)| *k == key) {
                return;
            }
            st.recent.push((key, now));

            let times = st.rate.entry(id.to_string()).or_default();
            while times.front().is_some_and(|t| now.duration_since(*t) >= RATE_WINDOW) {
                times.pop_front();
            }
            if times.len() >= RATE_RECORDED {
                return;
            }
            times.push_back(now);
            let flooding = times.len() > RATE_TOASTS;

            let quiet = st.settings.is_quiet(settings::now_ms());
            let entry = HistoryEntry {
                id: st.next_entry,
                service_id: id.to_string(),
                title: title.clone(),
                body: body.clone(),
                at: settings::now_ms(),
                muted: quiet || !meta.notify,
            };
            st.next_entry += 1;
            st.history.push_front(entry.clone());
            st.history.truncate(HISTORY_MAX);
            (entry, meta.notify && !quiet && !flooding, st.settings.hide_content || st.lock.locked)
        };
        let _ = app.emit("notification:new", &entry);
        if !show {
            return;
        }
        let (title, body) = if hide_content { ("Nuevo mensaje".to_string(), String::new()) } else { (title, body) };
        toast::show(
            app,
            Notice {
                service_id: Some(id.to_string()),
                title,
                body,
                // En Windows, sin sonido explícito el aviso es silencioso.
                sound: meta.sound.then(|| meta.kind.sound()),
            },
        );
    }

    /// Total de no leídos en el título de la ventana y en el icono de la bandeja.
    pub fn refresh_totals(&self, app: &AppHandle) {
        let total: u32 = self.state.lock().unwrap().unread.values().sum();
        if let Some(w) = app.get_window(MAIN_WINDOW) {
            let title = if total > 0 { format!("({total}) OmniChat") } else { "OmniChat".into() };
            let _ = w.set_title(&title);
        }
        tray::set_unread(app, total);
    }
}
