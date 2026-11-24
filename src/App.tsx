import { BellOff, ExternalLink, House, RotateCw, Settings2 } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { AddServiceDialog } from "./components/AddServiceDialog";
import { AppSettingsDialog } from "./components/AppSettingsDialog";
import { LockScreen } from "./components/LockScreen";
import { NotificationCenter } from "./components/NotificationCenter";
import { TasksDialog } from "./components/TasksDialog";
import { QuickSwitcher } from "./components/QuickSwitcher";
import { ServiceIcon } from "./components/ServiceIcon";
import { ServiceRail } from "./components/ServiceRail";
import { ServiceSettingsDialog } from "./components/ServiceSettingsDialog";
import { Welcome } from "./components/Welcome";
import { WorkspacesDialog } from "./components/WorkspacesDialog";
import { api, events, inTauri } from "./lib/api";
import { serviceColor } from "./lib/catalog";
import { servicePng } from "./lib/iconPng";
import {
  DEFAULT_SETTINGS,
  type Bounds,
  type HistoryEntry,
  type LockStatus,
  type Service,
  type Task,
  type Settings,
  type Workspace,
} from "./lib/types";

const ACTIVE_KEY = "omnichat.active";
const SEEN_KEY = "omnichat.notificationsSeen";
const HISTORY_MAX = 150;

/** Réplica en la UI de `Settings::is_quiet` (para el icono de campana tachada). */
function isQuiet(s: Settings, now: number): boolean {
  if (s.focusUntil && now < s.focusUntil) return true;
  const q = s.quietHours;
  if (!q.enabled) return false;
  const d = new Date(now);
  const mins = d.getHours() * 60 + d.getMinutes();
  const toMin = (t: string) => {
    const [h, m] = t.split(":").map(Number);
    return h * 60 + m;
  };
  const [a, b] = [toMin(q.start), toMin(q.end)];
  return a < b ? mins >= a && mins < b : a > b ? mins >= a || mins < b : false;
}

type Dialog = "add" | "workspaces" | "settings" | "switcher" | "notifications" | "tasks" | null;

export default function App() {
  const [services, setServices] = useState<Service[]>([]);
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [loaded, setLoaded] = useState(false);
  const [active, setActive] = useState<string | null>(() => localStorage.getItem(ACTIVE_KEY));
  const [unread, setUnread] = useState<Record<string, number>>({});
  const [dialog, setDialog] = useState<Dialog>(null);
  const [settingsFor, setSettingsFor] = useState<Service | null>(null);
  const [bounds, setBounds] = useState<Bounds | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const [hibernated, setHibernated] = useState<Set<string>>(() => new Set());
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [seenAt, setSeenAt] = useState(() => Number(localStorage.getItem(SEEN_KEY)) || 0);
  const [lock, setLock] = useState<LockStatus>({ hasPin: false, locked: false, retryInMs: 0 });
  const [tasks, setTasks] = useState<Task[]>([]);
  const stage = useRef<HTMLDivElement>(null);
  const sentIcons = useRef(new Map<string, string>());

  const overlay = dialog !== null || settingsFor !== null;
  const visible = useMemo(
    () => (settings.activeWorkspace ? services.filter((s) => s.workspaceId === settings.activeWorkspace) : services),
    [services, settings.activeWorkspace],
  );
  const current = visible.find((s) => s.id === active) ?? visible[0] ?? null;
  const quiet = isQuiet(settings, now);

  // ---- Carga inicial y eventos de Rust -------------------------------------
  const reloadWorkspaces = useCallback(() => api.listWorkspaces().then(setWorkspaces).catch(() => {}), []);
  const loadHistory = useCallback(() => api.listNotifications().then(setHistory).catch(() => {}), []);
  const loadTasks = useCallback(() => api.listTasks().then(setTasks).catch(() => {}), []);
  const reloadAll = useCallback(() => {
    api.listServices().then(setServices).catch(() => {});
    api.listWorkspaces().then(setWorkspaces).catch(() => {});
    api.getSettings().then(setSettings).catch(() => {});
  }, []);

  useEffect(() => {
    Promise.all([api.listServices(), api.getSettings(), api.listWorkspaces()])
      .then(([list, s, ws]) => {
        setServices(list);
        setSettings(s);
        setWorkspaces(ws);
      })
      .finally(() => setLoaded(true));
    api.unreadCounts().then(setUnread).catch(() => {});
    void loadTasks();
    api.listHibernated().then((ids) => setHibernated(new Set(ids))).catch(() => {});
    api
      .lockStatus()
      .then((l) => {
        setLock(l);
        if (!l.locked) void loadHistory();
      })
      .catch(() => {});
    const offs = [
      events.unread(({ id, count }) => setUnread((u) => ({ ...u, [id]: count }))),
      events.zoom(({ id, zoom }) => setServices((l) => l.map((s) => (s.id === id ? { ...s, zoom } : s)))),
      events.settings(setSettings),
      events.configChanged(reloadAll),
      events.serviceUpdated((s) => setServices((l) => l.map((x) => (x.id === s.id ? s : x)))),
      events.notification((e) => setHistory((h) => [e, ...h].slice(0, HISTORY_MAX))),
      events.hibernated(({ id, hibernated: on }) =>
        setHibernated((set) => {
          const next = new Set(set);
          if (on) next.add(id);
          else next.delete(id);
          return next;
        }),
      ),
      events.lock((l) => {
        setLock(l);
        if (l.locked) {
          setDialog(null);
          setSettingsFor(null);
        }
      }),
    ];
    const tick = setInterval(() => setNow(Date.now()), 30_000);
    return () => {
      offs.forEach((off) => off());
      clearInterval(tick);
    };
  }, []);

  useEffect(() => {
    if (current) localStorage.setItem(ACTIVE_KEY, current.id);
  }, [current]);

  // Tras desbloquear, el historial vuelve a estar disponible.
  useEffect(() => {
    if (!lock.locked) void loadHistory();
  }, [lock.locked, loadHistory]);

  // Logo de cada servicio para sus avisos de Windows (sólo si cambia el nombre o el tipo).
  useEffect(() => {
    if (!inTauri) return;
    for (const s of services) {
      const sig = `${s.kind}|${s.label}|${s.url ?? ""}|${s.icon?.length ?? 0}`;
      if (sentIcons.current.get(s.id) === sig) continue;
      sentIcons.current.set(s.id, sig);
      servicePng(s)
        .then((png) => api.setServiceIcon(s.id, png))
        .catch((e) => {
          sentIcons.current.delete(s.id);
          console.warn("icono de avisos", e);
        });
    }
  }, [services]);

  // ---- Tema ------------------------------------------------------------------
  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const dark = settings.theme === "dark" || (settings.theme === "system" && media.matches);
      document.documentElement.dataset.theme = dark ? "dark" : "light";
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [settings.theme]);

  // ---- Atajos de teclado (llegan de Rust) -----------------------------------
  const shortcutState = useRef({ visible, current, overlay, locked: lock.locked });
  shortcutState.current = { visible, current, overlay, locked: lock.locked };
  useEffect(
    () =>
      events.shortcut((a) => {
        const { visible: list, current: cur, locked } = shortcutState.current;
        if (locked) return;
        if (a.action === "switcher") return setDialog((d) => (d === "switcher" ? null : "switcher"));
        if (list.length === 0) return;
        const i = cur ? list.findIndex((s) => s.id === cur.id) : -1;
        if (a.action === "select" && list[a.index]) setActive(list[a.index].id);
        if (a.action === "next") setActive(list[(i + 1) % list.length].id);
        if (a.action === "previous") setActive(list[(i - 1 + list.length) % list.length].id);
      }),
    [],
  );

  // ---- Posición del webview nativo ------------------------------------------
  useLayoutEffect(() => {
    const el = stage.current;
    if (!el) return;
    const measure = () => {
      const r = el.getBoundingClientRect();
      setBounds({ x: r.left, y: r.top, width: r.width, height: r.height });
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    window.addEventListener("resize", measure);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, [current?.id, loaded, lock.locked]);

  useEffect(() => {
    if (!bounds || !loaded) return;
    api.syncLayout(bounds, current?.id ?? null, overlay).catch(console.error);
  }, [bounds, current?.id, overlay, loaded, services.length]);

  // ---- Acciones ---------------------------------------------------------------
  const saveSettings = useCallback(
    (patch: Partial<Settings>) => {
      const next = { ...settings, ...patch };
      setSettings(next);
      api.updateSettings(next).then(setSettings).catch(console.error);
    },
    [settings],
  );

  const added = (s: Service) => {
    setServices((list) => [...list, s]);
    // Si el servicio va a otro espacio, cambiamos a ese espacio para verlo.
    if (settings.activeWorkspace && s.workspaceId !== settings.activeWorkspace) {
      saveSettings({ activeWorkspace: s.workspaceId });
    }
    setActive(s.id);
    setDialog(null);
  };

  const reorder = (ids: string[]) => {
    // `ids` es el orden de los visibles; el resto mantiene su posición relativa al final.
    const rest = services.filter((s) => !ids.includes(s.id)).map((s) => s.id);
    const all = [...ids, ...rest];
    setServices(all.map((id) => services.find((s) => s.id === id)!).filter(Boolean));
    void api.reorderServices(all);
  };

  const move = (id: string, dir: -1 | 1) => {
    const ids = visible.map((s) => s.id);
    const i = ids.indexOf(id);
    const j = i + dir;
    if (i < 0 || j < 0 || j >= ids.length) return;
    [ids[i], ids[j]] = [ids[j], ids[i]];
    reorder(ids);
  };

  /** Abre un servicio de cualquier espacio (desde un aviso o el buscador). */
  const openService = useCallback(
    (s: Service) => {
      if (settings.activeWorkspace && s.workspaceId !== settings.activeWorkspace) {
        saveSettings({ activeWorkspace: s.workspaceId });
      }
      setActive(s.id);
      setDialog(null);
      setSettingsFor(null);
    },
    [settings.activeWorkspace, saveSettings],
  );
  const openServiceRef = useRef(openService);
  openServiceRef.current = openService;
  const servicesRef = useRef(services);
  servicesRef.current = services;
  useEffect(
    () =>
      events.notificationOpen((id) => {
        const s = servicesRef.current.find((x) => x.id === id);
        if (s) openServiceRef.current(s);
      }),
    [],
  );

  const unseen = history.filter((e) => e.at > seenAt && services.some((s) => s.id === e.serviceId)).length;
  const openNotifications = () => {
    const t = Date.now();
    setSeenAt(t);
    localStorage.setItem(SEEN_KEY, String(t));
    setDialog("notifications");
  };

  const settingsIndex = settingsFor ? visible.findIndex((s) => s.id === settingsFor.id) : -1;

  if (lock.locked) {
    return <LockScreen status={lock} onUnlocked={setLock} />;
  }

  return (
    <div className="flex h-full">
      <ServiceRail
        services={visible}
        workspaces={workspaces}
        activeWorkspace={settings.activeWorkspace}
        active={current?.id ?? null}
        unread={unread}
        quiet={quiet}
        hibernated={hibernated}
        unseen={unseen}
        hasPin={lock.hasPin}
        pendingTasks={tasks.filter((t) => !t.done).length}
        onSelect={setActive}
        onSettings={setSettingsFor}
        onAdd={() => setDialog("add")}
        onReorder={reorder}
        onOpenWorkspaces={() => setDialog("workspaces")}
        onOpenAppSettings={() => setDialog("settings")}
        onOpenNotifications={openNotifications}
        onOpenTasks={() => setDialog("tasks")}
        onLock={() => void api.lockNow().then(setLock)}
      />

      <main className="flex min-w-0 flex-1 flex-col bg-paper">
        {current ? (
          <>
            <header className="relative flex h-11 shrink-0 items-center gap-2.5 border-b border-line px-4">
              <span
                aria-hidden
                className="absolute right-0 bottom-[-1px] left-0 h-[2px]"
                style={{ background: serviceColor(current.kind, current.label, current.url) }}
              />
              <ServiceIcon kind={current.kind} label={current.label} url={current.url} icon={current.icon} size={22} />
              <h1 className="truncate text-[14px] font-semibold">{current.label}</h1>
              {(unread[current.id] ?? 0) > 0 && (
                <span className="text-[12.5px] text-muted">{unread[current.id]} sin leer</span>
              )}
              {quiet && (
                <button
                  type="button"
                  onClick={() => setDialog("settings")}
                  className="inline-flex items-center gap-1 rounded-md px-2 py-0.5 text-[12px] font-medium text-warn hover:bg-fog"
                  title="Notificaciones silenciadas"
                >
                  <BellOff size={13} /> Silenciado
                </button>
              )}
              <div className="ml-auto flex items-center gap-0.5">
                {Math.abs(current.zoom - 1) > 0.001 && (
                  <button
                    type="button"
                    title="Volver al tamaño normal (Ctrl+0)"
                    onClick={() =>
                      api
                        .updateService(current.id, { zoom: 1 })
                        .then((s) => setServices((l) => l.map((x) => (x.id === s.id ? s : x))))
                    }
                    className="mr-1 rounded-md px-2 py-1 text-[12px] tabular-nums text-muted hover:bg-fog hover:text-ink"
                  >
                    {Math.round(current.zoom * 100)} %
                  </button>
                )}
                <ToolbarButton label="Volver al inicio del servicio" onClick={() => api.goHomeService(current.id)}>
                  <House size={16} />
                </ToolbarButton>
                <ToolbarButton label="Recargar" onClick={() => api.reloadService(current.id)}>
                  <RotateCw size={16} />
                </ToolbarButton>
                <ToolbarButton label="Abrir en el navegador" onClick={() => api.openInBrowser(current.id)}>
                  <ExternalLink size={16} />
                </ToolbarButton>
                <ToolbarButton label="Ajustes del servicio" onClick={() => setSettingsFor(current)}>
                  <Settings2 size={16} />
                </ToolbarButton>
              </div>
            </header>

            {/* El webview nativo del servicio se superpone exactamente sobre este div. */}
            <div ref={stage} className="relative min-h-0 flex-1">
              {!overlay && (
                <div className="absolute inset-0 grid place-items-center">
                  <div className="flex flex-col items-center gap-3 text-muted">
                    <ServiceIcon kind={current.kind} label={current.label} url={current.url} icon={current.icon} size={44} />
                    <p className="text-[13px]">Cargando {current.label}…</p>
                  </div>
                </div>
              )}
            </div>
          </>
        ) : loaded && services.length > 0 ? (
          <div className="grid flex-1 place-items-center px-8 text-center">
            <div className="max-w-[360px]">
              <p className="text-[15px] font-medium">Este espacio no tiene servicios todavía.</p>
              <p className="mt-2 text-[13px] text-muted">
                Añade uno nuevo o asigna uno existente desde sus ajustes (clic derecho en su icono).
              </p>
              <div className="mt-5 flex justify-center gap-2">
                <button type="button" className="btn-primary" onClick={() => setDialog("add")}>
                  Añadir servicio
                </button>
                <button type="button" className="btn-ghost border border-line" onClick={() => saveSettings({ activeWorkspace: null })}>
                  Ver todos
                </button>
              </div>
            </div>
          </div>
        ) : (
          loaded && <Welcome onAdded={added} />
        )}
      </main>

      {dialog === "add" && (
        <AddServiceDialog
          existing={services}
          workspaces={workspaces}
          defaultWorkspace={settings.activeWorkspace}
          onClose={() => setDialog(null)}
          onAdded={added}
        />
      )}
      {dialog === "workspaces" && (
        <WorkspacesDialog
          workspaces={workspaces}
          services={services}
          unread={unread}
          active={settings.activeWorkspace}
          onSelect={(id) => {
            saveSettings({ activeWorkspace: id });
            setDialog(null);
          }}
          onChanged={() => {
            void reloadWorkspaces();
            api.listServices().then(setServices).catch(() => {});
          }}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog === "settings" && (
        <AppSettingsDialog
          settings={settings}
          lock={lock}
          services={services}
          hibernated={hibernated}
          onChanged={setSettings}
          onLockChanged={setLock}
          onImported={() => {
            void reloadWorkspaces();
            api.listServices().then(setServices).catch(() => {});
          }}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog === "tasks" && (
        <TasksDialog
          tasks={tasks}
          services={services}
          current={current}
          onChanged={() => void loadTasks()}
          onOpen={openService}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog === "notifications" && (
        <NotificationCenter
          entries={history}
          services={services}
          onOpen={openService}
          onClear={() => {
            setHistory([]);
            void api.clearNotifications();
          }}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog === "switcher" && (
        <QuickSwitcher
          services={services}
          workspaces={workspaces}
          unread={unread}
          onPick={openService}
          onClose={() => setDialog(null)}
        />
      )}
      {settingsFor && (
        <ServiceSettingsDialog
          key={settingsFor.id}
          service={settingsFor}
          workspaces={workspaces}
          canMoveUp={settingsIndex > 0}
          canMoveDown={settingsIndex >= 0 && settingsIndex < visible.length - 1}
          isActive={settingsFor.id === current?.id}
          hibernated={hibernated.has(settingsFor.id)}
          hibernationOn={settings.hibernation.enabled}
          onHibernate={() => {
            const id = settingsFor.id;
            return api.hibernateService(id).then(() => setHibernated((set) => new Set(set).add(id)));
          }}
          onMove={(dir) => move(settingsFor.id, dir)}
          onClose={() => setSettingsFor(null)}
          onChanged={(s) => setServices((list) => list.map((x) => (x.id === s.id ? s : x)))}
          onRemoved={(id) => {
            setServices((list) => list.filter((s) => s.id !== id));
            setUnread(({ [id]: _, ...u }) => u);
          }}
        />
      )}
    </div>
  );
}

function ToolbarButton({ label, onClick, children }: { label: string; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={label}
      aria-label={label}
      className="grid h-8 w-8 place-items-center rounded-md text-muted transition-colors hover:bg-fog hover:text-ink"
    >
      {children}
    </button>
  );
}
