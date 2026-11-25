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
/// `(99+) Messenger`, `Inbox (2,345)`… Tomamos el primer `(N)` o `(N+)` que parezca un contador.
///
/// Al principio del título siempre lo es. Más adelante puede ser parte del texto (el asunto de
/// un correo, el nombre de un documento): se descartan los años y, en las webs personalizadas,
/// también las cifras de cuatro o más dígitos sin separador de miles.
pub fn unread_from_title(kind: ServiceKind, title: &str) -> u32 {
    let mut leading = true;
    for (i, c) in title.char_indices() {
        if c == '(' {
            if let Some((count, grouped)) = title[i + 1..].split_once(')').and_then(|(n, _)| parse_count(n)) {
                let is_year = (1900..=2099).contains(&count);
                let too_big = kind == ServiceKind::Custom && count >= 1000;
                if leading || grouped || !(is_year || too_big) {
                    return count;
                }
            }
        }
        // Viñetas, emojis o espacios delante del contador no cuentan como texto.
        leading &= !c.is_alphanumeric() && c != '(';
    }
    0
}

/// `12`, `99+`, `2,345` o `2.345`. El segundo valor dice si lleva separador de miles.
fn parse_count(text: &str) -> Option<(u32, bool)> {
    let text = text.strip_suffix('+').unwrap_or(text);
    let groups: Vec<&str> = text.split([',', '.', '\u{a0}', '\u{202f}']).collect();
    if groups.iter().any(|g| g.is_empty() || !g.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    let grouped = groups.len() > 1;
    if grouped && (groups[0].len() > 3 || groups[1..].iter().any(|g| g.len() != 3)) {
        return None;
    }
    groups.concat().parse().ok().map(|n| (n, grouped))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_unread_counts() {
        use ServiceKind::*;
        assert_eq!(unread_from_title(Whatsapp, "(3) WhatsApp"), 3);
        assert_eq!(unread_from_title(Gmail, "Recibidos (12) - ana@gmail.com - Gmail"), 12);
        assert_eq!(unread_from_title(Messenger, "(99+) Messenger"), 99);
        assert_eq!(unread_from_title(Whatsapp, "WhatsApp"), 0);
        assert_eq!(unread_from_title(Outlook, "Correo (sin leer) - Outlook"), 0);
        assert_eq!(unread_from_title(Custom, "Ñandú (5)"), 5);
        assert_eq!(unread_from_title(Custom, "• (4) Discord"), 4);
        assert_eq!(unread_from_title(Gmail, "Recibidos (2.345) - ana@gmail.com - Gmail"), 2345);
        assert_eq!(unread_from_title(Gmail, "Inbox (12,345) - ana@gmail.com - Gmail"), 12345);
        assert_eq!(unread_from_title(Gmail, "Inbox (4567) - ana@gmail.com - Gmail"), 4567);
        assert_eq!(unread_from_title(Custom, "(1200) Foro"), 1200);
    }

    #[test]
    fn ignores_numbers_that_are_not_counts() {
        use ServiceKind::*;
        assert_eq!(unread_from_title(Custom, "Informe (2024)"), 0);
        assert_eq!(unread_from_title(Gmail, "Factura (2024) - ana@gmail.com - Gmail"), 0);
        assert_eq!(unread_from_title(Custom, "Pedido (123456) - Tienda"), 0);
        assert_eq!(unread_from_title(Custom, "Informe (2024) (3)"), 3);
        assert_eq!(unread_from_title(Custom, "Versión (1.2)"), 0);
        assert_eq!(unread_from_title(Custom, "(99999999999)"), 0);
        for t in ["(", "()", "(+)", "(,)", "((((", "(1.234", "texto ("] {
            assert_eq!(unread_from_title(Custom, t), 0, "{t}");
        }
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
