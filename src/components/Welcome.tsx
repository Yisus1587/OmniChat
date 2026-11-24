import { useState } from "react";
import { api } from "../lib/api";
import { CATALOG } from "../lib/catalog";
import type { AppError, Service, ServiceKind } from "../lib/types";
import { ServiceIcon } from "./ServiceIcon";

/** Primera pantalla: elegir el primer servicio con un clic. */
export function Welcome({ onAdded }: { onAdded: (s: Service) => void }) {
  const [busy, setBusy] = useState<ServiceKind | null>(null);
  const [error, setError] = useState<string | null>(null);

  const add = async (kind: ServiceKind) => {
    setBusy(kind);
    setError(null);
    try {
      onAdded(await api.addService(kind));
    } catch (e) {
      setError((e as AppError).message);
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="grid flex-1 place-items-center overflow-y-auto px-8 py-10">
      <div className="w-full max-w-[560px]">
        <Switchboard />
        <h1 className="mt-8 text-[26px] leading-tight font-semibold tracking-[-0.015em]">
          Todos tus chats y correos, en una sola ventana.
        </h1>
        <p className="mt-3 max-w-[460px] text-[14px] leading-relaxed text-muted">
          Elige un servicio para empezar. Iniciarás sesión en su web oficial, como en el navegador, y la sesión se queda
          guardada en este equipo.
        </p>

        <div className="mt-7 grid grid-cols-2 gap-2 lg:grid-cols-3">
          {CATALOG.filter((c) => c.kind !== "custom").map((c) => (
            <button
              key={c.kind}
              type="button"
              disabled={busy !== null}
              onClick={() => void add(c.kind)}
              className="flex items-center gap-3 rounded-lg border border-line bg-surface p-3 text-left transition-colors hover:border-ink disabled:opacity-60"
            >
              <ServiceIcon kind={c.kind} label={c.name} size={30} />
              <span className="text-[13.5px] font-medium">{busy === c.kind ? "Añadiendo…" : c.name}</span>
            </button>
          ))}
        </div>
        {error && (
          <p role="alert" className="mt-3 text-[12.5px] text-bad">
            {error}
          </p>
        )}
      </div>
    </div>
  );
}

/** Ilustración: las líneas de cada servicio convergen en un único punto. */
function Switchboard() {
  const lines = CATALOG.filter((c) => c.kind !== "custom");
  const step = 20;
  const height = (lines.length - 1) * step + 24;
  const mid = height / 2;
  const ys = lines.map((_, i) => 12 + i * step);
  return (
    <svg viewBox={`0 0 300 ${height}`} className="w-full max-w-[300px]" aria-hidden>
      {lines.map((c, i) => (
        <g key={c.kind}>
          <circle cx="10" cy={ys[i]} r="6" fill="none" stroke={c.color} strokeWidth="3" />
          <path
            d={`M16 ${ys[i]} C 140 ${ys[i]}, 150 ${mid}, 262 ${mid}`}
            fill="none"
            stroke={c.color}
            strokeWidth="2.5"
            strokeLinecap="round"
          />
        </g>
      ))}
      <circle cx="274" cy={mid} r="12" fill="var(--color-ink)" />
      <circle cx="274" cy={mid} r="4.5" fill="var(--color-paper)" />
    </svg>
  );
}
