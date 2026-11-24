import { siteFor } from "./sites";
import type { ServiceKind } from "./types";

export interface CatalogEntry {
  kind: ServiceKind;
  name: string;
  /** Para qué cuentas sirve, en palabras del usuario. */
  detail: string;
  /** Variable CSS definida en index.css. */
  color: string;
}

export const CATALOG: CatalogEntry[] = [
  { kind: "whatsapp", name: "WhatsApp", detail: "Vincula escaneando un código QR con tu móvil", color: "var(--color-s-whatsapp)" },
  { kind: "telegram", name: "Telegram", detail: "Inicia sesión con tu número o un código QR", color: "var(--color-s-telegram)" },
  { kind: "messenger", name: "Messenger", detail: "Con tu cuenta de Facebook", color: "var(--color-s-messenger)" },
  { kind: "instagram", name: "Instagram", detail: "Mensajes directos de tu cuenta", color: "var(--color-s-instagram)" },
  { kind: "gmail", name: "Gmail", detail: "Cuentas de Google y Google Workspace", color: "var(--color-s-gmail)" },
  { kind: "outlook", name: "Outlook", detail: "Outlook.com, Hotmail y Live", color: "var(--color-s-outlook)" },
  { kind: "office365", name: "Microsoft 365", detail: "Correo de trabajo o centro educativo", color: "var(--color-s-office365)" },
  { kind: "custom", name: "Otra web", detail: "Discord, Slack, LinkedIn… cualquier dirección", color: "var(--color-s-custom)" },
];

export const byKind = Object.fromEntries(CATALOG.map((c) => [c.kind, c])) as Record<ServiceKind, CatalogEntry>;

/** Color estable a partir de un texto (para los servicios personalizados). */
export function colorFor(text: string): string {
  let h = 0;
  for (const ch of text) h = (h * 31 + ch.charCodeAt(0)) >>> 0;
  return `hsl(${h % 360} 55% 45%)`;
}

/** Color de la línea del servicio: el de su marca, o uno derivado del nombre si es personalizado. */
export function serviceColor(kind: ServiceKind, label: string, url?: string | null): string {
  return kind === "custom" ? (siteFor(url)?.color ?? colorFor(label)) : byKind[kind].color;
}
