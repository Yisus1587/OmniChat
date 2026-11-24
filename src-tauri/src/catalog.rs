//! Catálogo de servicios soportados: la web oficial de cada uno.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum ServiceKind {
    Whatsapp,
    Telegram,
    Messenger,
    Instagram,
    Gmail,
    Outlook,
    Office365,
    /// Cualquier web que el usuario quiera (Discord, Slack, LinkedIn…).
    Custom,
}

impl ServiceKind {
    pub fn name(self) -> &'static str {
        match self {
            ServiceKind::Whatsapp => "WhatsApp",
            ServiceKind::Telegram => "Telegram",
            ServiceKind::Messenger => "Messenger",
            ServiceKind::Instagram => "Instagram",
            ServiceKind::Gmail => "Gmail",
            ServiceKind::Outlook => "Outlook",
            ServiceKind::Office365 => "Microsoft 365",
            ServiceKind::Custom => "Web",
        }
    }

    /// Página que se carga en el webview del servicio (`None` para `Custom`: la pone el usuario).
    pub fn url(self) -> Option<&'static str> {
        Some(match self {
            ServiceKind::Whatsapp => "https://web.whatsapp.com/",
            ServiceKind::Telegram => "https://web.telegram.org/a/",
            ServiceKind::Messenger => "https://www.facebook.com/messages/",
            ServiceKind::Instagram => "https://www.instagram.com/direct/inbox/",
            ServiceKind::Gmail => "https://mail.google.com/mail/u/0/",
            // Cuentas personales (outlook.com, hotmail, live).
            ServiceKind::Outlook => "https://outlook.live.com/mail/",
            // Cuentas de trabajo o escuela.
            ServiceKind::Office365 => "https://outlook.office.com/mail/",
            ServiceKind::Custom => return None,
        })
    }

    /// Sonido de Windows para sus notificaciones: de correo o de mensajería.
    pub fn sound(self) -> &'static str {
        match self {
            ServiceKind::Gmail | ServiceKind::Outlook | ServiceKind::Office365 => "Mail",
            _ => "IM",
        }
    }
}

/// Valida y normaliza la URL de un servicio personalizado. Sólo http(s).
pub fn normalize_custom_url(input: &str) -> Option<String> {
    let trimmed = input.trim();
    let with_scheme = if trimmed.contains("://") { trimmed.to_string() } else { format!("https://{trimmed}") };
    let url = tauri::Url::parse(&with_scheme).ok()?;
    let host_ok = url.host_str().is_some_and(|h| h.contains('.') || h == "localhost");
    (matches!(url.scheme(), "http" | "https") && host_ok).then(|| url.to_string())
}

/// Extrae el número de no leídos del título de la página.
///
/// Las webs lo publican así: `(3) WhatsApp`, `Recibidos (12) - ana@gmail.com - Gmail`,
/// `(99+) Messenger`… Tomamos el primer `(N)` o `(N+)`.
pub fn unread_from_title(title: &str) -> u32 {
    let bytes = title.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'(' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > start {
                let mut end = j;
                if end < bytes.len() && bytes[end] == b'+' {
                    end += 1;
                }
                if end < bytes.len() && bytes[end] == b')' {
                    return title[start..j].parse().unwrap_or(0);
                }
            }
        }
        i += 1;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_unread_counts() {
        assert_eq!(unread_from_title("(3) WhatsApp"), 3);
        assert_eq!(unread_from_title("Recibidos (12) - ana@gmail.com - Gmail"), 12);
        assert_eq!(unread_from_title("(99+) Messenger"), 99);
        assert_eq!(unread_from_title("WhatsApp"), 0);
        assert_eq!(unread_from_title("Correo (sin leer) - Outlook"), 0);
        assert_eq!(unread_from_title("Ñandú (5)"), 5);
    }

    #[test]
    fn normalizes_custom_urls() {
        assert_eq!(normalize_custom_url("discord.com/app").as_deref(), Some("https://discord.com/app"));
        assert_eq!(normalize_custom_url(" https://app.slack.com ").as_deref(), Some("https://app.slack.com/"));
        assert_eq!(normalize_custom_url("javascript:alert(1)"), None);
        assert_eq!(normalize_custom_url("file:///C:/x"), None);
        assert_eq!(normalize_custom_url("hola"), None);
    }
}
