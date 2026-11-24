//! Ajustes generales de la app (se guardan como JSON en la tabla `settings`).

use chrono::{Local, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};

pub const KEY: &str = "app";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// Horario fijo de "No molestar" (p. ej. 22:00 → 08:00, cruzando la medianoche).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct QuietHours {
    pub enabled: bool,
    /// "HH:MM"
    pub start: String,
    /// "HH:MM"
    pub end: String,
}

impl Default for QuietHours {
    fn default() -> Self {
        Self { enabled: false, start: "22:00".into(), end: "08:00".into() }
    }
}

/// Cerrar los servicios que no se usan para liberar memoria.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Hibernation {
    pub enabled: bool,
    /// Minutos sin abrir un servicio antes de hibernarlo.
    pub minutes: u32,
}

impl Default for Hibernation {
    fn default() -> Self {
        Self { enabled: false, minutes: 30 }
    }
}

/// Cuándo se bloquea OmniChat (sólo si hay un PIN configurado).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LockPrefs {
    /// Al cerrar la ventana (se queda junto al reloj).
    pub on_hide: bool,
    /// Tras estos minutos sin usar OmniChat (0 = nunca).
    pub idle_minutes: u32,
}

impl Default for LockPrefs {
    fn default() -> Self {
        Self { on_hide: true, idle_minutes: 0 }
    }
}

/// Sincronización mediante una carpeta que ya se sincroniza (OneDrive, Dropbox…).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SyncPrefs {
    /// `None` = desactivada.
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: Theme,
    /// Notificaciones sin remitente ni texto ("Nuevo mensaje en WhatsApp").
    pub hide_content: bool,
    pub quiet_hours: QuietHours,
    /// Modo concentración hasta esta hora (epoch ms). `None` = desactivado.
    pub focus_until: Option<i64>,
    /// Espacio de trabajo abierto (`None` = todos los servicios).
    pub active_workspace: Option<String>,
    /// Arrancar con Windows (minimizado en la bandeja).
    pub autostart: bool,
    pub hibernation: Hibernation,
    pub lock: LockPrefs,
    pub sync: SyncPrefs,
}

fn parse_hm(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s.trim(), "%H:%M").ok()
}

/// ¿`now` cae dentro del horario? Admite tramos que cruzan la medianoche.
pub fn in_quiet_hours(q: &QuietHours, now: NaiveTime) -> bool {
    if !q.enabled {
        return false;
    }
    let (Some(start), Some(end)) = (parse_hm(&q.start), parse_hm(&q.end)) else { return false };
    let now = NaiveTime::from_hms_opt(now.hour(), now.minute(), 0).unwrap_or(now);
    match start.cmp(&end) {
        std::cmp::Ordering::Less => now >= start && now < end,
        std::cmp::Ordering::Greater => now >= start || now < end,
        std::cmp::Ordering::Equal => false,
    }
}

impl Settings {
    /// ¿Hay que silenciar las notificaciones ahora mismo?
    pub fn is_quiet(&self, now_ms: i64) -> bool {
        self.focus_until.is_some_and(|until| now_ms < until)
            || in_quiet_hours(&self.quiet_hours, Local::now().time())
    }

    pub fn validate(&self) -> Result<(), String> {
        if parse_hm(&self.quiet_hours.start).is_none() || parse_hm(&self.quiet_hours.end).is_none() {
            return Err("el horario debe tener el formato HH:MM".into());
        }
        if !(5..=1440).contains(&self.hibernation.minutes) {
            return Err("la hibernación debe ser de entre 5 minutos y 24 horas".into());
        }
        if self.lock.idle_minutes > 1440 {
            return Err("el bloqueo por inactividad no puede superar 24 horas".into());
        }
        Ok(())
    }
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// "Hasta mañana": las 08:00 del día siguiente (hora local), en epoch ms.
pub fn tomorrow_morning_ms() -> i64 {
    let tomorrow = Local::now().date_naive().succ_opt().unwrap_or_else(|| Local::now().date_naive());
    tomorrow
        .and_hms_opt(8, 0, 0)
        .and_then(|dt| dt.and_local_timezone(Local).earliest())
        .map(|dt| dt.timestamp_millis())
        .unwrap_or_else(|| now_ms() + 12 * 3_600_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn quiet_hours_across_midnight() {
        let q = QuietHours { enabled: true, start: "22:00".into(), end: "08:00".into() };
        assert!(in_quiet_hours(&q, t(23, 30)));
        assert!(in_quiet_hours(&q, t(2, 0)));
        assert!(in_quiet_hours(&q, t(22, 0)));
        assert!(!in_quiet_hours(&q, t(8, 0)));
        assert!(!in_quiet_hours(&q, t(15, 0)));
    }

    #[test]
    fn quiet_hours_same_day_and_disabled() {
        let q = QuietHours { enabled: true, start: "13:00".into(), end: "15:30".into() };
        assert!(in_quiet_hours(&q, t(14, 0)));
        assert!(!in_quiet_hours(&q, t(15, 30)));
        assert!(!in_quiet_hours(&QuietHours { enabled: false, ..q.clone() }, t(14, 0)));
        assert!(!in_quiet_hours(&QuietHours { start: "25:00".into(), ..q }, t(14, 0)));
    }

    #[test]
    fn focus_until_and_defaults() {
        let s = Settings { focus_until: Some(2_000), ..Default::default() };
        assert!(s.is_quiet(1_000));
        assert!(!Settings::default().is_quiet(1_000));
        let parsed: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert_eq!(parsed.theme, Theme::Dark);
        assert_eq!(parsed.quiet_hours.start, "22:00");
        assert_eq!(parsed.hibernation.minutes, 30);
        assert!(parsed.lock.on_hide);
        assert!(parsed.validate().is_ok());
        let bad = Settings { hibernation: Hibernation { enabled: true, minutes: 1 }, ..Default::default() };
        assert!(bad.validate().is_err());
        assert!(tomorrow_morning_ms() > now_ms());
    }
}
