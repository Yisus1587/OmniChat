//! Avisos de escritorio con identidad propia.
//!
//! En Windows cada servicio se registra como un remitente de avisos distinto (un
//! AppUserModelID propio en `HKCU\Software\Classes\AppUserModelId`, con su nombre y su
//! logo). Así el aviso aparece como «WhatsApp · OmniChat» con el logo de WhatsApp, el
//! centro de notificaciones de Windows los agrupa por servicio y en Configuración →
//! Notificaciones se puede ajustar cada uno por separado. Sin esto, Windows atribuye los
//! avisos de una app sin instalar a «Windows PowerShell».
//!
//! Al hacer clic en un aviso se abre OmniChat directamente en ese servicio.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter, Manager};

use crate::tray;

/// Debe coincidir con `identifier` de tauri.conf.json (el instalador usa el mismo).
pub const APP_AUMID: &str = "com.omnichat.desktop";

/// Icono de OmniChat para sus propios avisos.
const APP_ICON: &[u8] = include_bytes!("../icons/128x128.png");

pub fn service_aumid(id: &str) -> String {
    format!("{APP_AUMID}.svc.{id}")
}

fn icons_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("icons"))
}

pub fn service_icon_path(app: &AppHandle, id: &str) -> Option<PathBuf> {
    icons_dir(app).map(|d| d.join(format!("{id}.png")))
}

fn app_icon_path(app: &AppHandle) -> Option<PathBuf> {
    icons_dir(app).map(|d| d.join("omnichat.png"))
}

/// Escribe el fichero sólo si cambia (Windows guarda en caché los iconos).
fn write_if_changed(path: &Path, bytes: &[u8]) -> std::io::Result<bool> {
    if std::fs::read(path).is_ok_and(|old| old == bytes) {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, bytes)?;
    Ok(true)
}

/// Registra OmniChat como remitente de sus propios avisos (al arrancar).
pub fn register_app(app: &AppHandle) {
    let Some(icon) = app_icon_path(app) else { return };
    if let Err(e) = write_if_changed(&icon, APP_ICON) {
        tracing::warn!(error = %e, "could not write app icon");
    }
    platform::register(APP_AUMID, "OmniChat", Some(&icon));
}

/// Registra (o actualiza) un servicio como remitente: «WhatsApp · OmniChat».
pub fn register_service(app: &AppHandle, id: &str, label: &str) {
    let icon = service_icon_path(app, id).filter(|p| p.exists());
    platform::register(&service_aumid(id), &format!("{label} · OmniChat"), icon.as_deref());
}

/// Guarda el logo del servicio (PNG generado por la interfaz) y lo registra.
pub fn set_service_icon(app: &AppHandle, id: &str, label: &str, png: &[u8]) -> std::io::Result<()> {
    const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
    if !png.starts_with(PNG_MAGIC) || png.len() > 512 * 1024 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "icono no válido"));
    }
    if let Some(path) = service_icon_path(app, id) {
        write_if_changed(&path, png)?;
    }
    register_service(app, id, label);
    Ok(())
}

/// Al quitar un servicio: fuera su registro y su icono.
pub fn forget_service(app: &AppHandle, id: &str) {
    platform::unregister(&service_aumid(id));
    if let Some(path) = service_icon_path(app, id) {
        let _ = std::fs::remove_file(path);
    }
}

/// Quita del centro de notificaciones de Windows los avisos ya mostrados de estos servicios
/// (al bloquear con PIN: sus vistas previas no deben seguir a la vista).
pub fn clear_history(service_ids: &[String]) {
    for id in service_ids {
        platform::clear(&service_aumid(id));
    }
}

pub struct Notice {
    /// `None` = aviso de la propia OmniChat.
    pub service_id: Option<String>,
    pub title: String,
    pub body: String,
    /// Sonido del sistema ("IM", "Mail"…); `None` = silencioso.
    pub sound: Option<&'static str>,
}

/// Clic en un aviso: muestra OmniChat y, si es de un servicio, lo abre.
fn activate(app: &AppHandle, service_id: Option<&str>) {
    tray::show_main(app);
    if let Some(id) = service_id {
        let _ = app.emit("notification:open", id);
    }
}

pub fn show(app: &AppHandle, notice: Notice) {
    let app = app.clone();
    // Windows tarda unos milisegundos en entregar el aviso: fuera del hilo de la UI.
    std::thread::spawn(move || {
        if let Err(e) = platform::show(&app, &notice) {
            tracing::warn!(error = %e, "notification failed");
        }
    });
}

#[cfg(windows)]
mod platform {
    use std::path::Path;
    use std::str::FromStr;

    use tauri::AppHandle;
    use tauri_winrt_notification::{IconCrop, Sound, Toast};
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    use super::{activate, app_icon_path, service_aumid, service_icon_path, Notice, APP_AUMID};

    const AUMID_KEY: &str = r"Software\Classes\AppUserModelId";

    pub fn register(aumid: &str, name: &str, icon: Option<&Path>) {
        let result = (|| -> std::io::Result<()> {
            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            let (key, _) = hkcu.create_subkey(format!(r"{AUMID_KEY}\{aumid}"))?;
            key.set_value("DisplayName", &name.to_string())?;
            if let Some(icon) = icon {
                key.set_value("IconUri", &icon.display().to_string())?;
            }
            Ok(())
        })();
        if let Err(e) = result {
            tracing::warn!(error = %e, aumid, "could not register notification sender");
        }
    }

    pub fn unregister(aumid: &str) {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let _ = hkcu.delete_subkey_all(format!(r"{AUMID_KEY}\{aumid}"));
    }

    pub fn clear(aumid: &str) {
        use windows::core::HSTRING;
        use windows::UI::Notifications::ToastNotificationManager;
        let result = ToastNotificationManager::History().and_then(|h| h.ClearWithId(&HSTRING::from(aumid)));
        if let Err(e) = result {
            tracing::debug!(error = %e, aumid, "could not clear notification history");
        }
    }

    pub fn show(app: &AppHandle, n: &Notice) -> Result<(), String> {
        let (aumid, icon) = match &n.service_id {
            Some(id) => (service_aumid(id), service_icon_path(app, id)),
            None => (APP_AUMID.to_string(), app_icon_path(app)),
        };
        let mut toast = Toast::new(&aumid)
            .title(&n.title)
            .sound(n.sound.and_then(|s| Sound::from_str(s).ok()));
        if !n.body.is_empty() {
            toast = toast.text1(&n.body);
        }
        if let Some(icon) = icon.filter(|p| p.exists()) {
            toast = toast.icon(&icon, IconCrop::Square, "");
        }
        let app = app.clone();
        let id = n.service_id.clone();
        toast
            .on_activated(move |_| {
                activate(&app, id.as_deref());
                Ok(())
            })
            .show()
            .map_err(|e| e.to_string())
    }
}

#[cfg(not(windows))]
mod platform {
    use std::path::Path;

    use tauri::AppHandle;
    use tauri_plugin_notification::NotificationExt;

    use super::Notice;

    pub fn register(_aumid: &str, _name: &str, _icon: Option<&Path>) {}

    pub fn unregister(_aumid: &str) {}

    pub fn clear(_aumid: &str) {}

    pub fn show(app: &AppHandle, n: &Notice) -> Result<(), String> {
        let mut b = app.notification().builder().title(&n.title).body(&n.body);
        if let Some(s) = n.sound {
            b = b.sound(s);
        }
        b.show().map_err(|e| e.to_string())
    }
}
