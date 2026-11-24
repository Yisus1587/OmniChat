//! Atajos de teclado. El foco del teclado suele estar dentro de la web del servicio
//! (WhatsApp, Gmail…), así que la UI no ve las teclas: se registran como atajos globales
//! SÓLO mientras la ventana de OmniChat está activa, y se liberan al perder el foco.
//!
//! * Ctrl+1 … Ctrl+9 → ir al servicio N de la barra.
//! * Ctrl+Tab / Ctrl+Shift+Tab → servicio siguiente / anterior.
//! * Ctrl+K → buscador rápido.
//! * Ctrl + / Ctrl − / Ctrl 0 → zoom del servicio (se guarda).
//! * Ctrl+L → bloquear (si hay PIN).

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::db::{self, ServicePatch};
use crate::host::ServiceHost;
use crate::lock;

const DIGITS: [Code; 9] = [
    Code::Digit1,
    Code::Digit2,
    Code::Digit3,
    Code::Digit4,
    Code::Digit5,
    Code::Digit6,
    Code::Digit7,
    Code::Digit8,
    Code::Digit9,
];

const ZOOM_STEP: f64 = 0.1;

#[derive(Clone, Serialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum ShortcutAction {
    Select { index: usize },
    Next,
    Previous,
    Switcher,
}

fn all() -> Vec<Shortcut> {
    let ctrl = Some(Modifiers::CONTROL);
    let ctrl_shift = Some(Modifiers::CONTROL | Modifiers::SHIFT);
    let mut v: Vec<Shortcut> = DIGITS.iter().map(|c| Shortcut::new(ctrl, *c)).collect();
    v.extend([
        Shortcut::new(ctrl, Code::Tab),
        Shortcut::new(ctrl_shift, Code::Tab),
        Shortcut::new(ctrl, Code::KeyK),
        Shortcut::new(ctrl, Code::Equal),
        Shortcut::new(ctrl, Code::NumpadAdd),
        Shortcut::new(ctrl, Code::Minus),
        Shortcut::new(ctrl, Code::NumpadSubtract),
        Shortcut::new(ctrl, Code::Digit0),
        Shortcut::new(ctrl, Code::Numpad0),
        Shortcut::new(ctrl, Code::KeyL),
    ]);
    v
}

pub fn plugin() -> TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            handle(app, shortcut);
        })
        .build()
}

fn handle(app: &AppHandle, s: &Shortcut) {
    let shift = s.mods.contains(Modifiers::SHIFT);
    let emit = |a: ShortcutAction| {
        let _ = app.emit("shortcut", a);
    };
    if let Some(i) = DIGITS.iter().position(|c| *c == s.key) {
        return emit(ShortcutAction::Select { index: i });
    }
    match s.key {
        Code::Tab if shift => emit(ShortcutAction::Previous),
        Code::Tab => emit(ShortcutAction::Next),
        Code::KeyK => emit(ShortcutAction::Switcher),
        Code::Equal | Code::NumpadAdd => zoom(app, Some(ZOOM_STEP)),
        Code::Minus | Code::NumpadSubtract => zoom(app, Some(-ZOOM_STEP)),
        Code::Digit0 | Code::Numpad0 => zoom(app, None),
        Code::KeyL => {
            lock::lock(app);
        }
        _ => {}
    }
}

fn zoom(app: &AppHandle, delta: Option<f64>) {
    let Some((id, value)) = app.state::<ServiceHost>().zoom_active(app, delta) else { return };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let pool = app.state::<SqlitePool>();
        let patch = ServicePatch { zoom: Some(value), ..Default::default() };
        if let Err(e) = db::update(&pool, &id, &patch).await {
            tracing::warn!(error = %e, "could not persist zoom");
        }
    });
}

/// Activa o libera los atajos según el foco de la ventana.
pub fn set_active(app: &AppHandle, active: bool) {
    let gs = app.global_shortcut();
    if active {
        for s in all() {
            if !gs.is_registered(s) {
                // Si otra app ya usa la combinación, simplemente no la tendremos.
                if let Err(e) = gs.register(s) {
                    tracing::debug!(error = %e, "shortcut unavailable");
                }
            }
        }
    } else if let Err(e) = gs.unregister_all() {
        tracing::debug!(error = %e, "could not release shortcuts");
    }
}
