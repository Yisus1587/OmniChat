//! Icono en la bandeja del sistema (junto al reloj).
//!
//! * Clic izquierdo: muestra/oculta OmniChat.
//! * Menú: Abrir · Concentración (1 hora / hasta mañana / desactivar) · Salir.
//! * Punto rojo en el icono cuando hay mensajes sin leer; el total va en el tooltip.
//! * Cerrar la ventana la esconde aquí en lugar de salir (los servicios siguen recibiendo).

use std::sync::{Mutex, OnceLock};

use chrono::{Local, TimeZone};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::host::MAIN_WINDOW;
use crate::settings::{self, Settings};
use crate::{lock, prefs, window_state};

const TRAY_ID: &str = "omnichat";

struct Icons {
    normal: Image<'static>,
    unread: Image<'static>,
}

static ICONS: OnceLock<Icons> = OnceLock::new();

/// Elementos del menú que cambian de texto/estado.
struct FocusItems {
    status: MenuItem<Wry>,
    off: MenuItem<Wry>,
}

static FOCUS_ITEMS: OnceLock<Mutex<FocusItems>> = OnceLock::new();
/// "Bloquear ahora": sólo activo si hay un PIN.
static LOCK_ITEM: OnceLock<Mutex<MenuItem<Wry>>> = OnceLock::new();

/// Usa el icono de la app que Tauri ya incrusta decodificado (RGBA) al compilar.
fn init_icons(app: &AppHandle) {
    ICONS.get_or_init(|| {
        let base = app.default_window_icon().expect("app icon configured in tauri.conf.json");
        let (w, h) = (base.width(), base.height());
        let normal = Image::new_owned(base.rgba().to_vec(), w, h);
        let unread = Image::new_owned(with_dot(base.rgba(), w, h), w, h);
        Icons { normal, unread }
    });
}

/// Dibuja un punto rojo con borde oscuro en la esquina inferior derecha.
fn with_dot(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut out = rgba.to_vec();
    let r = w.min(h) as f32 * 0.25;
    let (cx, cy) = (w as f32 - r - 0.5, h as f32 - r - 0.5);
    for y in 0..h {
        for x in 0..w {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            let i = ((y * w + x) * 4) as usize;
            let px = if d <= r - 1.5 {
                [0xE0, 0x3B, 0x3B, 0xFF]
            } else if d <= r {
                [0x17, 0x20, 0x2C, 0xFF]
            } else {
                continue;
            };
            out[i..i + 4].copy_from_slice(&px);
        }
    }
    out
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_window(MAIN_WINDOW) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn toggle_main(app: &AppHandle) {
    if let Some(w) = app.get_window(MAIN_WINDOW) {
        let visible = w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false);
        if visible && w.is_focused().unwrap_or(false) {
            window_state::save(app, &w);
            let _ = w.hide();
            lock::on_hidden(app);
        } else {
            show_main(app);
        }
    }
}

fn spawn_focus(app: &AppHandle, until: Option<i64>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = prefs::set_focus(&app, until).await {
            tracing::warn!(error = %e, "could not change focus mode");
        }
    });
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    init_icons(app);

    let open = MenuItem::with_id(app, "open", "Abrir OmniChat", true, None::<&str>)?;
    let status = MenuItem::with_id(app, "focus_status", "Notificaciones activas", false, None::<&str>)?;
    let hour = MenuItem::with_id(app, "focus_1h", "Silenciar 1 hora", true, None::<&str>)?;
    let tomorrow = MenuItem::with_id(app, "focus_tomorrow", "Silenciar hasta mañana (8:00)", true, None::<&str>)?;
    let off = MenuItem::with_id(app, "focus_off", "Reactivar notificaciones", false, None::<&str>)?;
    let focus = Submenu::with_items(
        app,
        "Concentración",
        true,
        &[&status, &PredefinedMenuItem::separator(app)?, &hour, &tomorrow, &off],
    )?;
    let lock_now = MenuItem::with_id(app, "lock", "Bloquear ahora", false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Salir de OmniChat", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &focus, &lock_now, &PredefinedMenuItem::separator(app)?, &quit])?;

    let _ = FOCUS_ITEMS.set(Mutex::new(FocusItems { status, off }));
    let _ = LOCK_ITEM.set(Mutex::new(lock_now));

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(ICONS.get().expect("icons").normal.clone())
        .tooltip("OmniChat")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "focus_1h" => spawn_focus(app, Some(settings::now_ms() + 3_600_000)),
            "focus_tomorrow" => spawn_focus(app, Some(settings::tomorrow_morning_ms())),
            "focus_off" => spawn_focus(app, None),
            "lock" => {
                lock::lock(app);
            }
            "quit" => {
                if let Some(w) = app.get_window(MAIN_WINDOW) {
                    if w.is_visible().unwrap_or(false) {
                        window_state::save(app, &w);
                    }
                }
                app.exit(0)
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                toggle_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Actualiza el texto del submenú "Concentración".
pub fn refresh_focus(_app: &AppHandle, s: &Settings) {
    let Some(items) = FOCUS_ITEMS.get() else { return };
    let items = items.lock().unwrap();
    let active_until = s.focus_until.filter(|u| *u > settings::now_ms());
    let text = match active_until {
        Some(until) => {
            let hhmm = Local
                .timestamp_millis_opt(until)
                .single()
                .map(|t| t.format("%H:%M").to_string())
                .unwrap_or_default();
            format!("Silenciado hasta las {hhmm}")
        }
        None if s.quiet_hours.enabled => {
            format!("Horario silencioso: {}–{}", s.quiet_hours.start, s.quiet_hours.end)
        }
        None => "Notificaciones activas".to_string(),
    };
    let _ = items.status.set_text(text);
    let _ = items.off.set_enabled(active_until.is_some());
}

pub fn set_lock_available(_app: &AppHandle, available: bool) {
    if let Some(item) = LOCK_ITEM.get() {
        let _ = item.lock().unwrap().set_enabled(available);
    }
}

pub fn set_unread(app: &AppHandle, total: u32) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let Some(icons) = ICONS.get() else { return };
    let icon = if total > 0 { icons.unread.clone() } else { icons.normal.clone() };
    let _ = tray.set_icon(Some(icon));
    let tooltip = match total {
        0 => "OmniChat".to_string(),
        1 => "OmniChat: 1 mensaje sin leer".to_string(),
        n => format!("OmniChat: {n} mensajes sin leer"),
    };
    let _ = tray.set_tooltip(Some(tooltip));
}
