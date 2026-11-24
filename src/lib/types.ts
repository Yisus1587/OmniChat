// Espejo de los tipos de Rust (src-tauri/src/catalog.rs, db.rs y settings.rs).

export type ServiceKind =
  | "whatsapp"
  | "telegram"
  | "messenger"
  | "instagram"
  | "gmail"
  | "outlook"
  | "office365"
  | "custom";

export interface Service {
  id: string;
  kind: ServiceKind;
  label: string;
  /** Sólo en servicios personalizados. */
  url: string | null;
  workspaceId: string | null;
  position: number;
  notify: boolean;
  sound: boolean;
  zoom: number;
  /** Puede hibernar si no se usa durante un rato. */
  hibernate: boolean;
  /** Icono descargado de la web (data URL), sólo en servicios personalizados. */
  icon: string | null;
  /** null = el de WebView2; "mobile", "chrome" o un agente personalizado. */
  userAgent: string | null;
  proxy: string | null;
  customCss: string | null;
  customJs: string | null;
  /** Carga las extensiones de Chrome de la carpeta de extensiones. */
  extensions: boolean;
}

export interface ServicePatch {
  label?: string;
  notify?: boolean;
  sound?: boolean;
  /** "" = quitar del espacio. */
  workspaceId?: string;
  url?: string;
  zoom?: number;
  hibernate?: boolean;
  /** "" = volver al valor por defecto. */
  userAgent?: string;
  proxy?: string;
  customCss?: string;
  customJs?: string;
  extensions?: boolean;
}

export interface Workspace {
  id: string;
  name: string;
  position: number;
}

export type Theme = "system" | "light" | "dark";

export interface Settings {
  theme: Theme;
  hideContent: boolean;
  quietHours: { enabled: boolean; start: string; end: string };
  focusUntil: number | null;
  activeWorkspace: string | null;
  autostart: boolean;
  hibernation: { enabled: boolean; minutes: number };
  /** Cuándo se bloquea OmniChat (si hay PIN). `idleMinutes` 0 = nunca. */
  lock: { onHide: boolean; idleMinutes: number };
  /** Carpeta sincronizada (OneDrive, Dropbox…); null = desactivada. */
  sync: { folder: string | null };
}

export const DEFAULT_SETTINGS: Settings = {
  theme: "system",
  hideContent: false,
  quietHours: { enabled: false, start: "22:00", end: "08:00" },
  focusUntil: null,
  activeWorkspace: null,
  autostart: false,
  hibernation: { enabled: false, minutes: 30 },
  lock: { onHide: true, idleMinutes: 0 },
  sync: { folder: null },
};

export interface Task {
  id: string;
  text: string;
  done: boolean;
  serviceId: string | null;
  createdAt: number;
}

export interface ExtensionsInfo {
  folder: string;
  names: string[];
}

/** Una entrada del centro de notificaciones. */
export interface HistoryEntry {
  id: number;
  serviceId: string;
  title: string;
  body: string;
  at: number;
  /** No se mostró en el escritorio (silenciado o avisos desactivados). */
  muted: boolean;
}

export interface LockStatus {
  hasPin: boolean;
  locked: boolean;
  /** ms hasta poder reintentar tras varios PIN incorrectos. */
  retryInMs: number;
}

export interface ImportSummary {
  services: number;
  workspaces: number;
  skipped: number;
}

export type ShortcutAction =
  | { action: "select"; index: number }
  | { action: "next" }
  | { action: "previous" }
  | { action: "switcher" };

export interface Bounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface AppError {
  kind: string;
  message: string;
}
