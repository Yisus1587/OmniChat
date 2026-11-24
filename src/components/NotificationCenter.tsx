import { BellOff } from "lucide-react";
import type { HistoryEntry, Service } from "../lib/types";
import { Modal } from "./Modal";
import { ServiceIcon } from "./ServiceIcon";

const clock = new Intl.DateTimeFormat("es", { hour: "2-digit", minute: "2-digit" });
const day = new Intl.DateTimeFormat("es", { day: "numeric", month: "short" });

function when(at: number, now: number): string {
  const mins = Math.floor((now - at) / 60_000);
  if (mins < 1) return "Ahora";
  if (mins < 60) return `Hace ${mins} min`;
  const d = new Date(at);
  return new Date(now).toDateString() === d.toDateString() ? clock.format(d) : `${day.format(d)}, ${clock.format(d)}`;
}

/** Avisos que han llegado mientras OmniChat estaba abierto (incluidos los silenciados). */
export function NotificationCenter({
  entries,
  services,
  onOpen,
  onClear,
  onClose,
}: {
  entries: HistoryEntry[];
  services: Service[];
  onOpen: (s: Service) => void;
  onClear: () => void;
  onClose: () => void;
}) {
  const now = Date.now();
  const byId = new Map(services.map((s) => [s.id, s]));
  const list = entries.filter((e) => byId.has(e.serviceId));

  return (
    <Modal title="Avisos" onClose={onClose} width={460}>
      {list.length === 0 ? (
        <div className="px-4 py-10 text-center">
          <p className="text-[14px] font-medium">No hay avisos.</p>
          <p className="mx-auto mt-1.5 max-w-[300px] text-[12.5px] text-muted">
            Aquí se guardan los mensajes que lleguen mientras OmniChat está abierto, también los que llegan con las
            notificaciones silenciadas.
          </p>
        </div>
      ) : (
        <>
          <ul className="-mx-2 flex flex-col">
            {list.map((e) => {
              const s = byId.get(e.serviceId)!;
              return (
                <li key={e.id}>
                  <button
                    type="button"
                    onClick={() => onOpen(s)}
                    className="flex w-full items-start gap-3 rounded-lg px-2 py-2.5 text-left transition-colors hover:bg-fog"
                    title={`Abrir ${s.label}`}
                  >
                    <ServiceIcon kind={s.kind} label={s.label} url={s.url} icon={s.icon} size={30} />
                    <span className="min-w-0 flex-1">
                      <span className="flex items-baseline gap-2">
                        <span className="truncate text-[13.5px] font-semibold">{e.title}</span>
                        <span className="ml-auto shrink-0 text-[11.5px] text-faint tabular-nums">{when(e.at, now)}</span>
                      </span>
                      {e.body && <span className="line-clamp-2 text-[12.5px] text-ink-3">{e.body}</span>}
                      <span className="mt-0.5 flex items-center gap-1.5 text-[11.5px] text-muted">
                        {s.label}
                        {e.muted && (
                          <span className="inline-flex items-center gap-1 text-warn" title="No se mostró en el escritorio">
                            <BellOff size={11} /> silenciado
                          </span>
                        )}
                      </span>
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
          <div className="mt-3 flex items-center justify-between border-t border-line pt-3">
            <p className="text-[12px] text-faint">Se borran al cerrar OmniChat.</p>
            <button type="button" className="btn-ghost px-2.5 py-1.5 text-[12.5px]" onClick={onClear}>
              Borrar todo
            </button>
          </div>
        </>
      )}
    </Modal>
  );
}
