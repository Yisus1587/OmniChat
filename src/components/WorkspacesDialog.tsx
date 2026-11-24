import { Check, Pencil, Trash2 } from "lucide-react";
import { useState, type FormEvent } from "react";
import { api } from "../lib/api";
import type { AppError, Service, Workspace } from "../lib/types";
import { Modal } from "./Modal";

/** Elegir, crear, renombrar y borrar espacios de trabajo. */
export function WorkspacesDialog({
  workspaces,
  services,
  unread,
  active,
  onSelect,
  onChanged,
  onClose,
}: {
  workspaces: Workspace[];
  services: Service[];
  unread: Record<string, number>;
  active: string | null;
  onSelect: (id: string | null) => void;
  onChanged: () => void;
  onClose: () => void;
}) {
  const [name, setName] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [editName, setEditName] = useState("");
  const [error, setError] = useState<string | null>(null);

  const stats = (id: string | null) => {
    const list = id ? services.filter((s) => s.workspaceId === id) : services;
    return { count: list.length, unread: list.reduce((n, s) => n + (unread[s.id] ?? 0), 0) };
  };

  const run = async (fn: () => Promise<unknown>) => {
    setError(null);
    try {
      await fn();
      onChanged();
    } catch (e) {
      setError((e as AppError).message);
    }
  };

  const create = (e: FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    void run(async () => {
      const w = await api.createWorkspace(name);
      setName("");
      onSelect(w.id);
    });
  };

  const Row = ({ id, label }: { id: string | null; label: string }) => {
    const s = stats(id);
    const selected = active === id;
    return (
      <button
        type="button"
        onClick={() => onSelect(id)}
        className={`flex min-w-0 flex-1 items-center gap-3 rounded-md px-3 py-2 text-left transition-colors ${
          selected ? "bg-fog-2" : "hover:bg-fog"
        }`}
      >
        <span className="grid h-4 w-4 place-items-center">{selected && <Check size={15} />}</span>
        <span className="min-w-0 flex-1">
          <span className="block truncate text-[13.5px] font-medium">{label}</span>
          <span className="block text-[12px] text-muted">
            {s.count === 1 ? "1 servicio" : `${s.count} servicios`}
            {s.unread > 0 && `, ${s.unread} sin leer`}
          </span>
        </span>
      </button>
    );
  };

  return (
    <Modal title="Espacios de trabajo" onClose={onClose} width={420}>
      <p className="mb-3 text-[12.5px] text-muted">
        Agrupa tus servicios (p. ej. Personal y Trabajo) y muestra sólo los de un espacio. Los demás siguen recibiendo
        mensajes. Asigna cada servicio a un espacio desde sus ajustes.
      </p>

      <ul className="flex flex-col gap-0.5">
        <li className="flex">
          <Row id={null} label="Todos los servicios" />
        </li>
        {workspaces.map((w) => (
          <li key={w.id} className="flex items-center gap-1">
            {editing === w.id ? (
              <form
                className="flex flex-1 gap-2 px-1 py-1"
                onSubmit={(e) => {
                  e.preventDefault();
                  void run(async () => {
                    await api.renameWorkspace(w.id, editName);
                    setEditing(null);
                  });
                }}
              >
                <input value={editName} onChange={(e) => setEditName(e.target.value)} maxLength={30} autoFocus className="field" />
                <button type="submit" className="btn-primary">
                  Guardar
                </button>
              </form>
            ) : (
              <>
                <Row id={w.id} label={w.name} />
                <button
                  type="button"
                  title="Renombrar"
                  aria-label={`Renombrar ${w.name}`}
                  className="grid h-8 w-8 place-items-center rounded-md text-muted hover:bg-fog hover:text-ink"
                  onClick={() => {
                    setEditing(w.id);
                    setEditName(w.name);
                  }}
                >
                  <Pencil size={14} />
                </button>
                <button
                  type="button"
                  title="Borrar espacio (los servicios no se borran)"
                  aria-label={`Borrar ${w.name}`}
                  className="grid h-8 w-8 place-items-center rounded-md text-muted hover:bg-fog hover:text-bad"
                  onClick={() =>
                    void run(async () => {
                      await api.deleteWorkspace(w.id);
                      if (active === w.id) onSelect(null);
                    })
                  }
                >
                  <Trash2 size={14} />
                </button>
              </>
            )}
          </li>
        ))}
      </ul>

      <form onSubmit={create} className="mt-4 flex gap-2 border-t border-line pt-4">
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Nuevo espacio, p. ej. Trabajo"
          maxLength={30}
          className="field"
        />
        <button type="submit" disabled={!name.trim()} className="btn-primary shrink-0">
          Crear
        </button>
      </form>

      {error && (
        <p role="alert" className="mt-3 text-[12.5px] text-bad">
          {error}
        </p>
      )}
    </Modal>
  );
}
