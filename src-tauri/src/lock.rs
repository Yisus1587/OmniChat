//! Bloqueo con PIN.
//!
//! Mientras OmniChat está bloqueado, Rust oculta todos los servicios (la interfaz sólo
//! muestra la pantalla de bloqueo) y los avisos no enseñan remitente ni texto. Es una
//! protección frente a miradas o a alguien que use el equipo un momento, no un cifrado:
//! las sesiones de cada web siguen en el disco como en cualquier navegador.
//!
//! El PIN nunca se guarda: sólo una huella (SHA-256 iterado con sal aleatoria).

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager};

use crate::db;
use crate::error::{AppError, AppResult};
use crate::host::ServiceHost;
use crate::tray;

const PIN_KEY: &str = "pin";
const ROUNDS: u32 = 60_000;
const MAX_ATTEMPTS: u32 = 5;
const BLOCK_FOR: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PinRecord {
    salt: String,
    hash: String,
}

#[derive(Debug, Default)]
pub struct LockState {
    pub has_pin: bool,
    pub locked: bool,
    failed: u32,
    blocked_until: Option<Instant>,
    /// Desde cuándo OmniChat no tiene el foco (para el bloqueo por inactividad).
    pub unfocused_since: Option<Instant>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LockStatus {
    pub has_pin: bool,
    pub locked: bool,
    /// Milisegundos hasta poder reintentar tras varios PIN incorrectos.
    pub retry_in_ms: u64,
}

impl LockState {
    pub fn status(&self) -> LockStatus {
        let retry_in_ms = self
            .blocked_until
            .map(|t| t.saturating_duration_since(Instant::now()).as_millis() as u64)
            .unwrap_or(0);
        LockStatus { has_pin: self.has_pin, locked: self.locked, retry_in_ms }
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn digest(salt: &str, pin: &str) -> String {
    let mut h: [u8; 32] = Sha256::digest(format!("{salt}:{pin}").as_bytes()).into();
    for _ in 0..ROUNDS {
        let mut s = Sha256::new();
        s.update(h);
        s.update(salt.as_bytes());
        h = s.finalize().into();
    }
    to_hex(&h)
}

fn make_record(pin: &str) -> PinRecord {
    let salt = uuid::Uuid::new_v4().simple().to_string();
    PinRecord { hash: digest(&salt, pin), salt }
}

/// Comparación sin atajos (no revela cuántos caracteres coinciden).
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn valid_pin(pin: &str) -> bool {
    (4..=8).contains(&pin.len()) && pin.bytes().all(|b| b.is_ascii_digit())
}

async fn load_record(pool: &SqlitePool) -> Option<PinRecord> {
    db::get_setting::<Option<PinRecord>>(pool, PIN_KEY).await.ok().flatten()
}

/// Al arrancar: si hay PIN, OmniChat empieza bloqueado.
pub async fn init(app: &AppHandle) {
    let pool = app.state::<SqlitePool>();
    let has_pin = load_record(&pool).await.is_some();
    app.state::<ServiceHost>().with_lock(|l| {
        l.has_pin = has_pin;
        l.locked = has_pin;
    });
    tray::set_lock_available(app, has_pin);
}

fn changed(app: &AppHandle) -> LockStatus {
    let host = app.state::<ServiceHost>();
    let status = host.with_lock(|l| l.status());
    let _ = host.apply(app);
    tray::set_lock_available(app, status.has_pin);
    let _ = app.emit("lock:changed", &status);
    status
}

pub fn status(app: &AppHandle) -> LockStatus {
    app.state::<ServiceHost>().with_lock(|l| l.status())
}

/// Bloquea si hay PIN. Devuelve si quedó bloqueado.
pub fn lock(app: &AppHandle) -> bool {
    let did = app.state::<ServiceHost>().with_lock(|l| {
        if l.has_pin && !l.locked {
            l.locked = true;
            true
        } else {
            false
        }
    });
    if did {
        changed(app);
        // Los avisos ya mostrados seguirían legibles en el centro de notificaciones.
        crate::toast::clear_history(&app.state::<ServiceHost>().service_ids());
    }
    did
}

/// Comprueba el PIN contando intentos: tras 5 fallos hay que esperar 30 s.
async fn check(app: &AppHandle, pin: &str) -> AppResult<()> {
    let host = app.state::<ServiceHost>();
    let wait = host.with_lock(|l| l.status().retry_in_ms);
    if wait > 0 {
        return Err(AppError::InvalidInput(format!("demasiados intentos; espera {} s", wait.div_ceil(1000))));
    }
    let pool = app.state::<SqlitePool>();
    let Some(record) = load_record(&pool).await else {
        return Ok(());
    };
    if same(&digest(&record.salt, pin), &record.hash) {
        host.with_lock(|l| {
            l.failed = 0;
            l.blocked_until = None;
        });
        return Ok(());
    }
    let blocked = host.with_lock(|l| {
        l.failed += 1;
        if l.failed >= MAX_ATTEMPTS {
            l.failed = 0;
            l.blocked_until = Some(Instant::now() + BLOCK_FOR);
            true
        } else {
            false
        }
    });
    let _ = app.emit("lock:changed", status(app));
    Err(AppError::InvalidInput(if blocked {
        format!("PIN incorrecto. Espera {} s para volver a intentarlo", BLOCK_FOR.as_secs())
    } else {
        "PIN incorrecto".into()
    }))
}

pub async fn unlock(app: &AppHandle, pin: &str) -> AppResult<LockStatus> {
    check(app, pin).await?;
    app.state::<ServiceHost>().with_lock(|l| l.locked = false);
    Ok(changed(app))
}

/// Crear, cambiar (`new = Some`) o quitar (`new = None`) el PIN. Si ya hay uno, hace
/// falta el actual.
pub async fn set_pin(app: &AppHandle, current: Option<String>, new: Option<String>) -> AppResult<LockStatus> {
    let has_pin = status(app).has_pin;
    if has_pin {
        check(app, current.as_deref().unwrap_or_default()).await?;
    }
    let pool = app.state::<SqlitePool>();
    match new {
        Some(pin) => {
            if !valid_pin(&pin) {
                return Err(AppError::InvalidInput("el PIN debe tener entre 4 y 8 cifras".into()));
            }
            db::set_setting(&pool, PIN_KEY, &Some(make_record(&pin))).await?;
            app.state::<ServiceHost>().with_lock(|l| l.has_pin = true);
        }
        None => {
            db::set_setting(&pool, PIN_KEY, &None::<PinRecord>).await?;
            app.state::<ServiceHost>().with_lock(|l| {
                l.has_pin = false;
                l.locked = false;
            });
        }
    }
    Ok(changed(app))
}

/// La ventana se escondió en la bandeja.
pub fn on_hidden(app: &AppHandle) {
    if app.state::<ServiceHost>().settings().lock.on_hide {
        lock(app);
    }
}

pub fn on_focus(app: &AppHandle, focused: bool) {
    app.state::<ServiceHost>().with_lock(|l| {
        l.unfocused_since = if focused { None } else { Some(Instant::now()) };
    });
}

/// Tiempo sin teclado ni ratón en todo el equipo (`None` si no se puede saber).
#[cfg(windows)]
fn system_idle() -> Option<Duration> {
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    // SAFETY: `info` es válido durante la llamada y declara su tamaño.
    let now = unsafe {
        if !GetLastInputInfo(&mut info).as_bool() {
            return None;
        }
        GetTickCount()
    };
    // El contador de 32 bits da la vuelta cada 49 días; de ahí la resta modular.
    Some(Duration::from_millis(u64::from(now.wrapping_sub(info.dwTime))))
}

#[cfg(not(windows))]
fn system_idle() -> Option<Duration> {
    None
}

/// Tiempo sin usar OmniChat: desde que perdió el foco o, si lo conserva (el usuario se levantó
/// dejándolo en pantalla), desde la última pulsación en el equipo.
fn unused_for(unfocused: Option<Duration>, system: Option<Duration>) -> Duration {
    unfocused.or(system).unwrap_or_default()
}

/// Llamado periódicamente: bloquea tras N minutos sin usar OmniChat.
pub fn check_idle(app: &AppHandle) {
    let host = app.state::<ServiceHost>();
    let minutes = host.settings().lock.idle_minutes;
    if minutes == 0 {
        return;
    }
    let unfocused = host.with_lock(|l| l.unfocused_since.map(|t| t.elapsed()));
    if unused_for(unfocused, system_idle()) >= Duration::from_secs(u64::from(minutes) * 60) {
        lock(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_with_and_without_focus() {
        let (min, hour) = (Duration::from_secs(60), Duration::from_secs(3600));
        // En primer plano cuenta la inactividad del equipo.
        assert_eq!(unused_for(None, Some(hour)), hour);
        // En segundo plano, el tiempo sin foco aunque se esté usando otra app.
        assert_eq!(unused_for(Some(hour), Some(min)), hour);
        assert_eq!(unused_for(None, None), Duration::ZERO);
        #[cfg(windows)]
        assert!(system_idle().is_some());
    }

    #[test]
    fn pin_hash_and_validation() {
        let r = make_record("1234");
        assert_eq!(r.hash.len(), 64);
        assert!(same(&digest(&r.salt, "1234"), &r.hash));
        assert!(!same(&digest(&r.salt, "1235"), &r.hash));
        // Misma PIN, otra sal → otra huella.
        assert_ne!(make_record("1234").hash, r.hash);
        assert!(valid_pin("0000") && valid_pin("12345678"));
        assert!(!valid_pin("123") && !valid_pin("123456789") && !valid_pin("12a4"));
    }
}
