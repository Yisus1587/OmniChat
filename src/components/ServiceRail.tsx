import { Bell, BellOff, ListTodo, Lock, Moon, Plus, Settings } from "lucide-react";
import { useState } from "react";
import type { Service, Workspace } from "../lib/types";
import { ServiceIcon } from "./ServiceIcon";

interface Props {
  services: Service[];
  workspaces: Workspace[];
  activeWorkspace: string | null;
  active: string | null;
  unread: Record<string, number>;
  /** Notificaciones silenciadas ahora mismo (concentración u horario). */
  quiet: boolean;
  /** Servicios hibernados (cerrados para ahorrar memoria). */
  hibernated: Set<string>;
  /** Avisos nuevos desde que se abrió el centro de avisos. */
  unseen: number;
  hasPin: boolean;
  /** Tareas pendientes. */
  pendingTasks: number;
  onSelect: (id: string) => void;
  onSettings: (s: Service) => void;
  onAdd: () => void;
  onReorder: (ids: string[]) => void;
  onOpenWorkspaces: () => void;
  onOpenAppSettings: () => void;
  onOpenNotifications: () => void;
  onOpenTasks: () => void;
  onLock: () => void;
}

export function ServiceRail(p: Props) {
  const [dragging, setDragging] = useState<string | null>(null);
  const [over, setOver] = useState<string | null>(null);

  const drop = (targetId: string) => {
    if (!dragging || dragging === targetId) return;
    const ids = p.services.map((s) => s.id).filter((id) => id !== dragging);
    ids.splice(ids.indexOf(targetId), 0, dragging);
    p.onReorder(ids);
  };

  const ws = p.workspaces.find((w) => w.id === p.activeWorkspace);

  return (
    <nav aria-label="Servicios" className="on-dark flex w-[72px] shrink-0 flex-col items-center bg-rail py-3 text-rail-text">
      <button
        type="button"
        onClick={p.onOpenWorkspaces}
        title={ws ? `Espacio: ${ws.name}\nClic para cambiar` : "Todos los servicios\nClic para elegir un espacio"}
        aria-label={ws ? `Espacio de trabajo: ${ws.name}` : "Espacios de trabajo"}
        className="flex w-[60px] flex-col items-center gap-1 rounded-lg py-1.5 transition-colors hover:bg-rail-2"
      >
        <Logo />
        <span className="max-w-full truncate px-1 text-[10.5px] font-medium text-faint">{ws ? ws.name : "Todos"}</span>
      </button>

      <ul className="scroll-quiet mt-2 flex min-h-0 w-full flex-1 flex-col items-center gap-2 overflow-y-auto py-1">
        {p.services.map((s, i) => {
          const count = p.unread[s.id] ?? 0;
          const isActive = s.id === p.active;
          const asleep = p.hibernated.has(s.id);
          return (
            <li
              key={s.id}
              className={`relative transition-opacity ${dragging === s.id ? "opacity-40" : ""}`}
              draggable
              onDragStart={(e) => {
                setDragging(s.id);
                e.dataTransfer.effectAllowed = "move";
              }}
              onDragOver={(e) => {
                e.preventDefault();
                if (over !== s.id) setOver(s.id);
              }}
              onDragLeave={() => setOver((o) => (o === s.id ? null : o))}
              onDrop={(e) => {
                e.preventDefault();
                drop(s.id);
                setOver(null);
              }}
              onDragEnd={() => {
                setDragging(null);
                setOver(null);
              }}
            >
              {over === s.id && dragging && dragging !== s.id && (
                <span className="absolute -top-[5px] right-2 left-2 h-[2px] rounded bg-rail-text" />
              )}
              {isActive && <span className="absolute top-2 bottom-2 -left-[16px] w-[3px] rounded-r bg-rail-text" />}
              <button
                type="button"
                onClick={() => p.onSelect(s.id)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  p.onSettings(s);
                }}
                aria-current={isActive ? "page" : undefined}
                aria-label={asleep ? `${s.label}, hibernado` : count ? `${s.label}, ${count} sin leer` : s.label}
                title={`${s.label}${i < 9 ? `  (Ctrl+${i + 1})` : ""}${
                  asleep ? "\nHibernado: no recibe avisos hasta que lo abras" : ""
                }\nClic derecho: ajustes · Arrastra para ordenar`}
                className={`relative grid h-12 w-12 place-items-center rounded-xl transition-colors ${
                  isActive ? "bg-rail-3" : "hover:bg-rail-2"
                }`}
              >
                <span className={asleep ? "opacity-45 grayscale" : undefined}>
                  <ServiceIcon kind={s.kind} label={s.label} url={s.url} icon={s.icon} size={34} />
                </span>
                {asleep && (
                  <span className="absolute -bottom-0.5 -left-0.5 grid h-[18px] w-[18px] place-items-center rounded-full border-2 border-rail bg-rail-3 text-rail-text">
                    <Moon size={9} />
                  </span>
                )}
                {count > 0 && (
                  <span className="absolute -top-0.5 -right-0.5 grid h-[19px] min-w-[19px] place-items-center rounded-full border-2 border-rail bg-bad px-1 text-[10.5px] font-semibold text-white">
                    {count > 99 ? "99+" : count}
                  </span>
                )}
                {!s.notify && (
                  <span
                    className="absolute -right-0.5 -bottom-0.5 h-3 w-3 rounded-full border-2 border-rail bg-faint"
                    title="Notificaciones desactivadas"
                  />
                )}
              </button>
            </li>
          );
        })}
      </ul>

      <div className="mt-2 flex flex-col items-center gap-1.5">
        <button
          type="button"
          onClick={p.onAdd}
          title="Añadir servicio"
          aria-label="Añadir servicio"
          className="grid h-10 w-10 place-items-center rounded-xl border border-dashed border-rail-3 text-faint transition-colors hover:border-rail-text hover:text-rail-text"
        >
          <Plus size={18} />
        </button>
        <button
          type="button"
          onClick={p.onOpenNotifications}
          title={p.quiet ? "Avisos (notificaciones silenciadas)" : "Avisos"}
          aria-label={p.unseen ? `Avisos, ${p.unseen} nuevos` : "Avisos"}
          className="relative grid h-10 w-10 place-items-center rounded-xl text-faint transition-colors hover:bg-rail-2 hover:text-rail-text"
        >
          {p.quiet ? <BellOff size={18} className="text-warn" /> : <Bell size={18} />}
          {p.unseen > 0 && (
            <span className="absolute top-0.5 right-0.5 grid h-[17px] min-w-[17px] place-items-center rounded-full border-2 border-rail bg-bad px-1 text-[9.5px] font-semibold text-white">
              {p.unseen > 99 ? "99+" : p.unseen}
            </span>
          )}
        </button>
        <button
          type="button"
          onClick={p.onOpenTasks}
          title="Tareas"
          aria-label={p.pendingTasks ? `Tareas, ${p.pendingTasks} pendientes` : "Tareas"}
          className="relative grid h-10 w-10 place-items-center rounded-xl text-faint transition-colors hover:bg-rail-2 hover:text-rail-text"
        >
          <ListTodo size={18} />
          {p.pendingTasks > 0 && (
            <span className="absolute top-0.5 right-0.5 grid h-[17px] min-w-[17px] place-items-center rounded-full border-2 border-rail bg-rail-3 px-1 text-[9.5px] font-semibold text-rail-text">
              {p.pendingTasks > 99 ? "99+" : p.pendingTasks}
            </span>
          )}
        </button>
        {p.hasPin && (
          <button
            type="button"
            onClick={p.onLock}
            title="Bloquear OmniChat (Ctrl+L)"
            aria-label="Bloquear OmniChat"
            className="grid h-10 w-10 place-items-center rounded-xl text-faint transition-colors hover:bg-rail-2 hover:text-rail-text"
          >
            <Lock size={17} />
          </button>
        )}
        <button
          type="button"
          onClick={p.onOpenAppSettings}
          title="Ajustes"
          aria-label="Ajustes de OmniChat"
          className="grid h-10 w-10 place-items-center rounded-xl text-faint transition-colors hover:bg-rail-2 hover:text-rail-text"
        >
          <Settings size={18} />
        </button>
      </div>
    </nav>
  );
}

function Logo() {
  return (
    <svg viewBox="0 0 64 64" width="28" height="28" aria-hidden>
      <circle cx="32" cy="32" r="15" fill="none" stroke="var(--color-rail-text)" strokeWidth="4.5" />
      <circle cx="32" cy="17" r="3.6" fill="var(--color-s-telegram)" />
      <circle cx="47" cy="32" r="3.6" fill="var(--color-s-whatsapp)" />
      <circle cx="32" cy="47" r="3.6" fill="var(--color-s-outlook)" />
      <circle cx="17" cy="32" r="3.6" fill="var(--color-s-gmail)" />
    </svg>
  );
}
