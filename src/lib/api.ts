import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppError,
  Bounds,
  ExtensionsInfo,
  HistoryEntry,
  Task,
  ImportSummary,
  LockStatus,
  Service,
  ServiceKind,
  ServicePatch,
  Settings,
  ShortcutAction,
  Workspace,
} from "./types";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export function toAppError(e: unknown): AppError {
  if (e && typeof e === "object" && "message" in e) {
    const o = e as Partial<AppError>;
    return { kind: o.kind ?? "unknown", message: String(o.message) };
  }
  return { kind: "unknown", message: String(e) };
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri && import.meta.env.DEV) {
    const { mockInvoke } = await import("./mock");
    try {
      return mockInvoke(cmd, args) as T;
    } catch (e) {
      throw toAppError(e);
    }
  }
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toAppError(e);
  }
}

export const api = {
  listServices: () => call<Service[]>("list_services"),
  syncLayout: (bounds: Bounds, active: string | null, overlay: boolean) =>
    call<void>("sync_layout", { bounds, active, overlay }),
  addService: (kind: ServiceKind, opts: { label?: string; url?: string; workspaceId?: string | null } = {}) =>
    call<Service>("add_service", { kind, label: opts.label, url: opts.url, workspaceId: opts.workspaceId ?? null }),
  updateService: (id: string, patch: ServicePatch) => call<Service>("update_service", { id, patch }),
  reorderServices: (ids: string[]) => call<void>("reorder_services", { ids }),
  removeService: (id: string) => call<void>("remove_service", { id }),
  reloadService: (id: string) => call<void>("reload_service", { id }),
  goHomeService: (id: string) => call<void>("go_home_service", { id }),
  openInBrowser: (id: string) => call<void>("open_in_browser", { id }),
  unreadCounts: () => call<Record<string, number>>("unread_counts"),

  listWorkspaces: () => call<Workspace[]>("list_workspaces"),
  createWorkspace: (name: string) => call<Workspace>("create_workspace", { name }),
  renameWorkspace: (id: string, name: string) => call<void>("rename_workspace", { id, name }),
  deleteWorkspace: (id: string) => call<void>("delete_workspace", { id }),

  getSettings: () => call<Settings>("get_settings"),
  updateSettings: (settings: Settings) => call<Settings>("update_settings", { settings }),
  setFocus: (opts: { minutes?: number; untilTomorrow?: boolean }) =>
    call<Settings>("set_focus", { minutes: opts.minutes ?? null, untilTomorrow: opts.untilTomorrow ?? false }),

  setServiceIcon: (id: string, png: number[]) => call<void>("set_service_icon", { id, png }),
  listNotifications: () => call<HistoryEntry[]>("list_notifications"),
  clearNotifications: () => call<void>("clear_notifications"),

  hibernateService: (id: string) => call<void>("hibernate_service", { id }),
  listHibernated: () => call<string[]>("list_hibernated"),

  lockStatus: () => call<LockStatus>("lock_status"),
  lockNow: () => call<LockStatus>("lock_now"),
  unlock: (pin: string) => call<LockStatus>("unlock", { pin }),
  /** `next = null` quita el PIN. */
  setPin: (current: string | null, next: string | null) => call<LockStatus>("set_pin", { current, next }),

  refreshServiceIcon: (id: string) => call<void>("refresh_service_icon", { id }),

  serviceMemory: () => call<Record<string, number>>("service_memory"),
  extensionsInfo: () => call<ExtensionsInfo>("extensions_info"),
  openExtensionsFolder: () => call<void>("open_extensions_folder"),
  reloadExtensions: () => call<void>("reload_extensions"),

  syncSuggestions: () => call<{ name: string; path: string }[]>("sync_suggestions"),
  setSyncFolder: (folder: string | null) => call<Settings>("set_sync_folder", { folder }),
  syncNow: () => call<void>("sync_now"),

  listTasks: () => call<Task[]>("list_tasks"),
  addTask: (text: string, serviceId: string | null) => call<Task>("add_task", { text, serviceId }),
  setTaskDone: (id: string, done: boolean) => call<void>("set_task_done", { id, done }),
  deleteTask: (id: string) => call<void>("delete_task", { id }),
  clearDoneTasks: () => call<void>("clear_done_tasks"),

  exportConfig: () => call<string>("export_config"),
  importConfig: (json: string) => call<ImportSummary>("import_config", { json }),
};

/** Suscripción a un evento de Rust; devuelve la función para cancelarla. */
function subscribe<T>(name: string, handler: (payload: T) => void): () => void {
  if (!inTauri) return () => {};
  let unlisten: UnlistenFn | undefined;
  let disposed = false;
  listen<T>(name, (e) => handler(e.payload)).then((fn) => {
    if (disposed) fn();
    else unlisten = fn;
  });
  return () => {
    disposed = true;
    unlisten?.();
  };
}

export const events = {
  /** No leídos de un servicio (leído del título de su web). */
  unread: (h: (p: { id: string; count: number }) => void) => subscribe("service:unread", h),
  /** Zoom cambiado con Ctrl + / − / 0. */
  zoom: (h: (p: { id: string; zoom: number }) => void) => subscribe("service:zoom", h),
  /** Ajustes cambiados (p. ej. concentración desde la bandeja). */
  settings: (h: (s: Settings) => void) => subscribe("settings:changed", h),
  /** Atajos de teclado globales. */
  shortcut: (h: (a: ShortcutAction) => void) => subscribe("shortcut", h),
  /** Nuevo aviso en el centro de notificaciones. */
  notification: (h: (e: HistoryEntry) => void) => subscribe("notification:new", h),
  /** Clic en un aviso de Windows: abrir ese servicio. */
  notificationOpen: (h: (serviceId: string) => void) => subscribe("notification:open", h),
  /** Un servicio hibernó o despertó. */
  hibernated: (h: (p: { id: string; hibernated: boolean }) => void) => subscribe("service:hibernated", h),
  /** Servicio actualizado desde Rust (p. ej. ya tiene el icono de su web). */
  serviceUpdated: (h: (s: Service) => void) => subscribe("service:updated", h),
  /** Otro equipo cambió la configuración (sincronización): recargar todo. */
  configChanged: (h: () => void) => subscribe("config:changed", h),
  /** La sincronización falló (carpeta no disponible…). */
  syncError: (h: (message: string) => void) => subscribe("sync:error", h),
  /** Bloqueo con PIN. */
  lock: (h: (s: LockStatus) => void) => subscribe("lock:changed", h),
};
