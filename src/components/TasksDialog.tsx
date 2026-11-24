import { Trash2 } from "lucide-react";
import { useState, type FormEvent } from "react";
import { api } from "../lib/api";
import type { AppError, Service, Task } from "../lib/types";
import { Modal } from "./Modal";
import { ServiceIcon } from "./ServiceIcon";

/** Lista de tareas sencilla; cada tarea puede ir ligada a un servicio para abrirlo con un clic. */
export function TasksDialog({
  tasks,
  services,
  current,
  onChanged,
  onOpen,
  onClose,
}: {
  tasks: Task[];
  services: Service[];
  /** Servicio abierto: se propone como servicio de las tareas nuevas. */
  current: Service | null;
  onChanged: () => void;
  onOpen: (s: Service) => void;
  onClose: () => void;
}) {
  const [text, setText] = useState("");
  const [serviceId, setServiceId] = useState(current?.id ?? "");
  const [error, setError] = useState<string | null>(null);
  const byId = new Map(services.map((s) => [s.id, s]));
  const pending = tasks.filter((t) => !t.done);
  const done = tasks.filter((t) => t.done);

  const run = async (fn: () => Promise<unknown>) => {
    setError(null);
    try {
      await fn();
      onChanged();
    } catch (e) {
      setError((e as AppError).message);
    }
  };

  const add = (e: FormEvent) => {
    e.preventDefault();
    if (!text.trim()) return;
    void run(async () => {
      await api.addTask(text, serviceId || null);
      setText("");
    });
  };

  const row = (t: Task) => {
    const s = t.serviceId ? byId.get(t.serviceId) : undefined;
    return (
      <li key={t.id} className="group flex items-center gap-2.5 rounded-md px-2 py-1.5 hover:bg-fog">
        <input
          type="checkbox"
          checked={t.done}
          onChange={(e) => void run(() => api.setTaskDone(t.id, e.target.checked))}
          aria-label={t.done ? `Marcar «${t.text}» como pendiente` : `Marcar «${t.text}» como hecha`}
          className="h-4 w-4 shrink-0 accent-[var(--color-ink)]"
        />
        <span className={`min-w-0 flex-1 text-[13px] break-words ${t.done ? "text-faint line-through" : ""}`}>{t.text}</span>
        {s && (
          <button
            type="button"
            onClick={() => onOpen(s)}
            title={`Abrir ${s.label}`}
            className="flex shrink-0 items-center gap-1.5 rounded-md px-1.5 py-0.5 text-[12px] text-muted hover:bg-fog-2 hover:text-ink"
          >
            <ServiceIcon kind={s.kind} label={s.label} url={s.url} icon={s.icon} size={18} />
            <span className="max-w-[90px] truncate">{s.label}</span>
          </button>
        )}
        <button
          type="button"
          onClick={() => void run(() => api.deleteTask(t.id))}
          aria-label={`Borrar «${t.text}»`}
          title="Borrar"
          className="grid h-7 w-7 shrink-0 place-items-center rounded-md text-faint opacity-0 group-hover:opacity-100 hover:text-bad focus-visible:opacity-100"
        >
          <Trash2 size={14} />
        </button>
      </li>
    );
  };

  return (
    <Modal title="Tareas" onClose={onClose} width={480}>
      <form onSubmit={add} className="flex flex-col gap-2">
        <input
          autoFocus
          value={text}
          onChange={(e) => setText(e.target.value)}
          maxLength={300}
          placeholder="Algo que no quieras olvidar, p. ej. contestar a Lucía"
          className="field"
        />
        <div className="flex gap-2">
          <select value={serviceId} onChange={(e) => setServiceId(e.target.value)} className="field" aria-label="Servicio relacionado">
            <option value="">Sin servicio</option>
            {services.map((s) => (
              <option key={s.id} value={s.id}>
                {s.label}
              </option>
            ))}
          </select>
          <button type="submit" disabled={!text.trim()} className="btn-primary shrink-0">
            Añadir
          </button>
        </div>
      </form>
      {error && (
        <p role="alert" className="mt-2 text-[12.5px] text-bad">
          {error}
        </p>
      )}

      {tasks.length === 0 ? (
        <p className="py-8 text-center text-[13px] text-muted">No tienes tareas. Se guardan sólo en este equipo.</p>
      ) : (
        <>
          <ul className="-mx-2 mt-4 flex flex-col">{pending.map(row)}</ul>
          {pending.length === 0 && <p className="mt-4 text-[13px] text-muted">Todo hecho.</p>}
          {done.length > 0 && (
            <>
              <div className="mt-4 flex items-center justify-between border-t border-line pt-3">
                <span className="text-[12.5px] font-medium text-ink-3">Hechas ({done.length})</span>
                <button type="button" className="text-[12.5px] text-muted hover:text-ink" onClick={() => void run(() => api.clearDoneTasks())}>
                  Borrar las hechas
                </button>
              </div>
              <ul className="-mx-2 mt-1 flex flex-col">{done.map(row)}</ul>
            </>
          )}
        </>
      )}
    </Modal>
  );
}
