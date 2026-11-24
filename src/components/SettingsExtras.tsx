import { FolderOpen, RefreshCw } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { api, events } from "../lib/api";
import { formatBytes } from "../lib/format";
import type { AppError, ExtensionsInfo, Service, Settings } from "../lib/types";
import { ServiceIcon } from "./ServiceIcon";

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2">
      <h3 className="text-[13px] font-semibold">{title}</h3>
      {children}
    </section>
  );
}

/** Sincronizar con otros equipos a través de una carpeta de OneDrive, Dropbox… */
export function SyncSection({ settings, onChanged }: { settings: Settings; onChanged: (s: Settings) => void }) {
  const [suggestions, setSuggestions] = useState<{ name: string; path: string }[]>([]);
  const [path, setPath] = useState("");
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const folder = settings.sync.folder;

  useEffect(() => {
    api.syncSuggestions().then(setSuggestions).catch(() => {});
    return events.syncError((m) => setMessage({ ok: false, text: `No se pudo sincronizar: ${m}` }));
  }, []);

  const choose = async (f: string | null) => {
    setMessage(null);
    try {
      onChanged(await api.setSyncFolder(f));
      setPath("");
      setMessage(f ? { ok: true, text: "Sincronizado. Activa lo mismo en tus otros equipos con la misma carpeta." } : null);
    } catch (e) {
      setMessage({ ok: false, text: (e as AppError).message });
    }
  };

  return (
    <Section title="Sincronizar con otros equipos">
      <p className="text-[12.5px] text-muted">
        Usa una carpeta que ya se sincronice (OneDrive, Dropbox, Google Drive) para tener los mismos servicios, espacios
        y ajustes en todos tus PC. Sin servidores de OmniChat. Las sesiones, el PIN, el proxy, el código propio y las
        extensiones no se sincronizan. Quitar un servicio aquí lo quita también en los demás equipos.
      </p>
      {folder ? (
        <div className="flex flex-col gap-2 rounded-lg border border-line p-3">
          <p className="text-[12.5px] break-all">
            <span className="text-muted">Carpeta: </span>
            {folder}
          </p>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              className="btn-ghost border border-line"
              onClick={() =>
                void api
                  .syncNow()
                  .then(() => setMessage({ ok: true, text: "Sincronizado." }))
                  .catch((e) => setMessage({ ok: false, text: (e as AppError).message }))
              }
            >
              <RefreshCw size={14} /> Sincronizar ahora
            </button>
            <button type="button" className="btn-ghost text-bad" onClick={() => void choose(null)}>
              Dejar de sincronizar
            </button>
          </div>
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          {suggestions.length > 0 && (
            <div className="flex flex-wrap gap-2">
              {suggestions.map((s) => (
                <button
                  key={s.path}
                  type="button"
                  className="btn-ghost border border-line"
                  title={s.path}
                  onClick={() => void choose(s.path)}
                >
                  Usar {s.name}
                </button>
              ))}
            </div>
          )}
          <div className="flex gap-2">
            <input
              value={path}
              onChange={(e) => setPath(e.target.value)}
              placeholder="O escribe la ruta de una carpeta sincronizada"
              className="field text-[12.5px]"
            />
            <button type="button" className="btn-primary shrink-0" disabled={!path.trim()} onClick={() => void choose(path)}>
              Activar
            </button>
          </div>
        </div>
      )}
      {message && (
        <p role={message.ok ? "status" : "alert"} className={`text-[12.5px] ${message.ok ? "text-ok" : "text-bad"}`}>
          {message.text}
        </p>
      )}
    </Section>
  );
}

/** Extensiones de Chrome descomprimidas en una carpeta de OmniChat. */
export function ExtensionsSection() {
  const [info, setInfo] = useState<ExtensionsInfo | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const load = () => api.extensionsInfo().then(setInfo).catch(() => {});

  useEffect(() => {
    void load();
  }, []);

  return (
    <Section title="Extensiones de Chrome">
      <p className="text-[12.5px] text-muted">
        Por ejemplo, un bloqueador de anuncios. Descarga la extensión descomprimida (en uBlock Origin Lite: el .zip
        «chromium» de su página de GitHub), extrae cada una en su propia subcarpeta dentro de la carpeta de extensiones y
        actívalas en cada servicio desde sus ajustes (Avanzado). Instala sólo extensiones de fuentes de confianza: pueden
        leer todo lo que ves en esos servicios.
      </p>
      <div className="flex flex-wrap items-center gap-2">
        <button type="button" className="btn-ghost border border-line" onClick={() => void api.openExtensionsFolder()}>
          <FolderOpen size={14} /> Abrir carpeta de extensiones
        </button>
        <button
          type="button"
          className="btn-ghost border border-line"
          onClick={async () => {
            setMessage(null);
            await load();
            try {
              await api.reloadExtensions();
              setMessage("Servicios con extensiones recargados.");
            } catch (e) {
              setMessage((e as AppError).message);
            }
          }}
        >
          <RefreshCw size={14} /> Aplicar cambios
        </button>
      </div>
      <p className="text-[12.5px]">
        {info === null
          ? ""
          : info.names.length === 0
            ? "No hay extensiones instaladas."
            : `Instaladas: ${info.names.join(", ")}.`}
      </p>
      {message && <p className="text-[12.5px] text-muted">{message}</p>}
    </Section>
  );
}

/** Memoria que usa cada servicio, actualizada cada pocos segundos. */
export function MemorySection({ services, hibernated }: { services: Service[]; hibernated: Set<string> }) {
  const [usage, setUsage] = useState<Record<string, number> | null>(null);

  useEffect(() => {
    const load = () => api.serviceMemory().then(setUsage).catch(() => {});
    void load();
    const t = setInterval(load, 5000);
    return () => clearInterval(t);
  }, []);

  const rows = services
    .map((s) => ({ s, bytes: usage?.[s.id] ?? 0 }))
    .sort((a, b) => b.bytes - a.bytes);
  const total = rows.reduce((n, r) => n + r.bytes, 0);
  const max = Math.max(1, ...rows.map((r) => r.bytes));

  return (
    <Section title="Memoria por servicio">
      {usage === null ? (
        <p className="text-[12.5px] text-muted">Midiendo…</p>
      ) : (
        <ul className="flex flex-col gap-1.5">
          {rows.map(({ s, bytes }) => (
            <li key={s.id} className="grid grid-cols-[22px_minmax(0,9rem)_1fr_auto] items-center gap-2.5 text-[12.5px]">
              <ServiceIcon kind={s.kind} label={s.label} url={s.url} icon={s.icon} size={22} />
              <span className="truncate">{s.label}</span>
              <span className="h-1.5 rounded-full bg-fog-2">
                <span
                  className="block h-full rounded-full bg-ink-3"
                  style={{ width: `${(bytes / max) * 100}%` }}
                />
              </span>
              <span className="w-[72px] text-right text-muted tabular-nums">
                {hibernated.has(s.id) ? "hibernado" : bytes ? formatBytes(bytes) : "—"}
              </span>
            </li>
          ))}
          <li className="mt-1 flex justify-between border-t border-line pt-2 text-[12.5px] font-medium">
            <span>Total de los servicios</span>
            <span className="tabular-nums">{formatBytes(total)}</span>
          </li>
        </ul>
      )}
      <p className="text-[12px] text-muted">Si alguno usa mucha, hibérnalo desde sus ajustes o activa la hibernación automática.</p>
    </Section>
  );
}
