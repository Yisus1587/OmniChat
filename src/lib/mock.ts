// Datos de demostración para previsualizar la UI en un navegador (`npm run dev`) sin Tauri.
import { DEFAULT_SETTINGS, type Service, type ServiceKind, type Settings, type Workspace } from "./types";

const svc = (s: Partial<Service> & Pick<Service, "id" | "kind" | "label">): Service => ({
  url: null,
  workspaceId: null,
  position: 0,
  notify: true,
  sound: true,
  zoom: 1,
  hibernate: true,
  icon: null,
  userAgent: null,
  proxy: null,
  customCss: null,
  customJs: null,
  extensions: false,
  ...s,
});

let workspaces: Workspace[] = [
  { id: "w1", name: "Personal", position: 0 },
  { id: "w2", name: "Trabajo", position: 1 },
];

let services: Service[] = [
  svc({ id: "s1", kind: "whatsapp", label: "WhatsApp", workspaceId: "w1" }),
  svc({ id: "s2", kind: "telegram", label: "Telegram", workspaceId: "w1" }),
  svc({ id: "s3", kind: "gmail", label: "Gmail personal", workspaceId: "w1" }),
  svc({ id: "s4", kind: "office365", label: "Correo del trabajo", workspaceId: "w2", notify: false }),
  svc({ id: "s5", kind: "custom", label: "Discord", url: "https://discord.com/app", workspaceId: "w2" }),
  svc({ id: "s6", kind: "custom", label: "Intranet", url: "https://intranet.ejemplo.es", workspaceId: "w2" }),
];

let settings: Settings = { ...DEFAULT_SETTINGS };
let lock = { hasPin: false, locked: false, retryInMs: 0 };
let tasks = [
  { id: "t1", text: "Contestar a Lucía lo de la cena", done: false, serviceId: "s1", createdAt: Date.now() - 3_600_000 },
  { id: "t2", text: "Revisar el extracto del banco", done: false, serviceId: "s3", createdAt: Date.now() - 7_200_000 },
  { id: "t3", text: "Llamar al fontanero", done: true, serviceId: null, createdAt: Date.now() - 86_400_000 },
];
let pin: string | null = null;
let history = [
  { id: 3, serviceId: "s1", title: "Lucía Romero", body: "¿Te viene bien a las 7 en la terraza? Llevo las entradas.", at: Date.now() - 4 * 60_000, muted: false },
  { id: 2, serviceId: "s3", title: "Banco Norte", body: "Tu extracto de octubre ya está disponible", at: Date.now() - 52 * 60_000, muted: true },
  { id: 1, serviceId: "s2", title: "Grupo Excursión", body: "Marcos: he subido las fotos al álbum compartido", at: Date.now() - 3 * 3_600_000, muted: false },
];

export function mockInvoke(cmd: string, args: Record<string, unknown> = {}): unknown {
  switch (cmd) {
    case "list_services":
      return services;
    case "unread_counts":
      return { s1: 4, s3: 12, s5: 2 };
    case "add_service": {
      const s = svc({
        id: `s${Date.now()}`,
        kind: args.kind as ServiceKind,
        label: (args.label as string) || String(args.kind),
        url: (args.url as string) ?? null,
        workspaceId: (args.workspaceId as string) ?? null,
        position: services.length,
      });
      services = [...services, s];
      return s;
    }
    case "update_service": {
      const p = args.patch as Partial<Service> & { workspaceId?: string };
      services = services.map((s) =>
        s.id === args.id
          ? { ...s, ...p, workspaceId: p.workspaceId === undefined ? s.workspaceId : p.workspaceId || null }
          : s,
      );
      return services.find((s) => s.id === args.id);
    }
    case "remove_service":
      services = services.filter((s) => s.id !== args.id);
      return null;
    case "list_workspaces":
      return workspaces;
    case "create_workspace": {
      const w = { id: `w${Date.now()}`, name: String(args.name), position: workspaces.length };
      workspaces = [...workspaces, w];
      return w;
    }
    case "rename_workspace":
      workspaces = workspaces.map((w) => (w.id === args.id ? { ...w, name: String(args.name) } : w));
      return null;
    case "delete_workspace":
      workspaces = workspaces.filter((w) => w.id !== args.id);
      services = services.map((s) => (s.workspaceId === args.id ? { ...s, workspaceId: null } : s));
      return null;
    case "get_settings":
      return settings;
    case "update_settings":
      settings = args.settings as Settings;
      return settings;
    case "set_focus":
      settings = {
        ...settings,
        focusUntil: args.untilTomorrow
          ? Date.now() + 10 * 3_600_000
          : args.minutes
            ? Date.now() + Number(args.minutes) * 60_000
            : null,
      };
      return settings;
    case "service_memory":
      return { s1: 412e6, s2: 238e6, s3: 305e6, s5: 520e6, s6: 96e6 };
    case "extensions_info":
      return { folder: "C:\\Users\\demo\\AppData\\Roaming\\com.omnichat.desktop\\extensions", names: ["uBlock Origin Lite"] };
    case "sync_suggestions":
      return [{ name: "OneDrive", path: "C:\\Users\\demo\\OneDrive\\OmniChat" }];
    case "set_sync_folder":
      settings = { ...settings, sync: { folder: (args.folder as string | null) ?? null } };
      return settings;
    case "list_tasks":
      return [...tasks].sort((a, b) => Number(a.done) - Number(b.done) || b.createdAt - a.createdAt);
    case "add_task": {
      const t = { id: `t${Date.now()}`, text: String(args.text), done: false, serviceId: (args.serviceId as string) || null, createdAt: Date.now() };
      tasks = [t, ...tasks];
      return t;
    }
    case "set_task_done":
      tasks = tasks.map((t) => (t.id === args.id ? { ...t, done: Boolean(args.done) } : t));
      return null;
    case "delete_task":
      tasks = tasks.filter((t) => t.id !== args.id);
      return null;
    case "clear_done_tasks":
      tasks = tasks.filter((t) => !t.done);
      return null;
    case "list_notifications":
      return history;
    case "clear_notifications":
      history = [];
      return null;
    case "list_hibernated":
      return ["s4"];
    case "lock_status":
      return lock;
    case "lock_now":
      lock = { ...lock, locked: lock.hasPin };
      return lock;
    case "unlock":
      if (args.pin !== pin) throw { kind: "invalid_input", message: "PIN incorrecto" };
      lock = { ...lock, locked: false };
      return lock;
    case "set_pin":
      if (lock.hasPin && args.current !== pin) throw { kind: "invalid_input", message: "PIN incorrecto" };
      pin = (args.next as string | null) ?? null;
      lock = { hasPin: pin !== null, locked: false, retryInMs: 0 };
      return lock;
    case "export_config":
      return "C:\\Users\\demo\\Downloads\\omnichat-configuracion-2026-11-23.json";
    case "import_config":
      return { services: 2, workspaces: 1, skipped: 3 };
    default:
      return null;
  }
}
