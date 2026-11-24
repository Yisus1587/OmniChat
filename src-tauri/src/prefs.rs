//! Guardar y aplicar los ajustes generales (tema, inicio con Windows, concentración…).

use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::host::{ServiceHost, MAIN_WINDOW};
use crate::settings::{self, Settings, Theme, KEY};
use crate::tray;

pub async fn load(pool: &SqlitePool) -> Settings {
    db::get_setting::<Settings>(pool, KEY).await.unwrap_or_default()
}

/// Aplica los efectos de los ajustes que no viven en la UI.
pub fn apply(app: &AppHandle, s: &Settings) {
    app.state::<ServiceHost>().set_settings(s.clone());

    if let Some(w) = app.get_window(MAIN_WINDOW) {
        let theme = match s.theme {
            Theme::System => None,
            Theme::Light => Some(tauri::Theme::Light),
            Theme::Dark => Some(tauri::Theme::Dark),
        };
        let _ = w.set_theme(theme);
    }

    let autolaunch = app.autolaunch();
    let enabled = autolaunch.is_enabled().unwrap_or(false);
    let result = match (s.autostart, enabled) {
        (true, false) => autolaunch.enable(),
        (false, true) => autolaunch.disable(),
        _ => Ok(()),
    };
    if let Err(e) = result {
        tracing::warn!(error = %e, "autostart toggle failed");
    }

    tray::refresh_focus(app, s);
    let _ = app.emit("settings:changed", s);
}

/// Valida, guarda y aplica.
pub async fn save(app: &AppHandle, s: Settings) -> AppResult<Settings> {
    s.validate().map_err(AppError::InvalidInput)?;
    let pool = app.state::<SqlitePool>();
    db::set_setting(&pool, KEY, &s).await?;
    apply(app, &s);
    Ok(s)
}

/// Modo concentración: `until` en epoch ms, o `None` para desactivarlo.
pub async fn set_focus(app: &AppHandle, until: Option<i64>) -> AppResult<Settings> {
    let mut s = app.state::<ServiceHost>().settings();
    s.focus_until = until.filter(|u| *u > settings::now_ms());
    save(app, s).await
}
