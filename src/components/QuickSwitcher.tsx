import { Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { byKind } from "../lib/catalog";
import type { Service, Workspace } from "../lib/types";
import { Modal } from "./Modal";
import { ServiceIcon } from "./ServiceIcon";

function norm(s: string) {
  return s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
}

/** Ctrl+K: escribe para filtrar y Enter para saltar a un servicio (de cualquier espacio). */
export function QuickSwitcher({
  services,
  workspaces,
  unread,
  onPick,
  onClose,
}: {
  services: Service[];
  workspaces: Workspace[];
  unread: Record<string, number>;
  onPick: (s: Service) => void;
  onClose: () => void;
}) {
  const [q, setQ] = useState("");
  const [index, setIndex] = useState(0);
  const listRef = useRef<HTMLUListElement>(null);
  const wsName = (id: string | null) => workspaces.find((w) => w.id === id)?.name;

  const results = useMemo(() => {
    const needle = norm(q.trim());
    const list = needle
      ? services.filter((s) =>
          norm(`${s.label} ${s.kind === "custom" ? s.url ?? "" : byKind[s.kind].name} ${wsName(s.workspaceId) ?? ""}`).includes(
            needle,
          ),
        )
      : services;
    // Primero los que tienen mensajes sin leer.
    return [...list].sort((a, b) => (unread[b.id] ?? 0) - (unread[a.id] ?? 0));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [q, services, unread]);

  useEffect(() => setIndex(0), [q]);
  useEffect(() => {
    listRef.current?.children[index]?.scrollIntoView({ block: "nearest" });
  }, [index]);

  return (
    <Modal title="Ir a un servicio" onClose={onClose} width={460}>
      <label className="relative block">
        <Search size={15} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-faint" />
        <input
          autoFocus
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown") {
              e.preventDefault();
              setIndex((i) => Math.min(i + 1, results.length - 1));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setIndex((i) => Math.max(i - 1, 0));
            } else if (e.key === "Enter" && results[index]) {
              e.preventDefault();
              onPick(results[index]);
            }
          }}
          placeholder="Escribe un nombre: whatsapp, trabajo, gmail…"
          aria-label="Buscar servicio"
          className="field pl-9"
        />
      </label>
      <ul ref={listRef} className="scroll-quiet mt-3 max-h-[320px] overflow-y-auto" role="listbox">
        {results.length === 0 && <li className="px-3 py-6 text-center text-[13px] text-muted">Ningún servicio coincide.</li>}
        {results.map((s, i) => (
          <li key={s.id} role="option" aria-selected={i === index}>
            <button
              type="button"
              onMouseEnter={() => setIndex(i)}
              onClick={() => onPick(s)}
              className={`flex w-full items-center gap-3 rounded-md px-3 py-2 text-left ${i === index ? "bg-fog-2" : ""}`}
            >
              <ServiceIcon kind={s.kind} label={s.label} url={s.url} icon={s.icon} size={28} />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[13.5px] font-medium">{s.label}</span>
                {wsName(s.workspaceId) && <span className="block text-[12px] text-muted">{wsName(s.workspaceId)}</span>}
              </span>
              {(unread[s.id] ?? 0) > 0 && (
                <span className="rounded-full bg-bad px-1.5 text-[11px] font-semibold text-white">{unread[s.id]}</span>
              )}
            </button>
          </li>
        ))}
      </ul>
    </Modal>
  );
}
