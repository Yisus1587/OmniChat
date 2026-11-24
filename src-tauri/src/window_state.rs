//! Recuerda tamaño, posición y maximizado de la ventana principal entre sesiones
//! (`<app_data>/window.json`).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Window};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Saved {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    maximized: bool,
}

fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("window.json"))
}

/// Aplica el estado guardado, si lo hay y cae dentro de algún monitor actual.
pub fn restore(app: &AppHandle, window: &Window) {
    let Some(p) = path(app) else { return };
    let Ok(text) = std::fs::read_to_string(p) else { return };
    let Ok(s) = serde_json::from_str::<Saved>(&text) else { return };
    if s.width < 400 || s.height < 300 {
        return;
    }
    let on_screen = window
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .any(|m| {
            let (mp, ms) = (m.position(), m.size());
            s.x + 50 >= mp.x
                && s.y >= mp.y - 10
                && s.x + 50 < mp.x + ms.width as i32
                && s.y + 50 < mp.y + ms.height as i32
        });
    let _ = window.set_size(PhysicalSize::new(s.width, s.height));
    if on_screen {
        let _ = window.set_position(PhysicalPosition::new(s.x, s.y));
    }
    if s.maximized {
        let _ = window.maximize();
    }
}

/// Guarda el estado actual (se llama al cerrar/esconder la ventana).
pub fn save(app: &AppHandle, window: &Window) {
    let maximized = window.is_maximized().unwrap_or(false);
    if window.is_minimized().unwrap_or(false) {
        return; // posición de "minimizada" no sirve
    }
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.inner_size()) else { return };
    let saved = Saved { x: pos.x, y: pos.y, width: size.width, height: size.height, maximized };
    if let (Some(p), Ok(json)) = (path(app), serde_json::to_string(&saved)) {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, json);
    }
}
