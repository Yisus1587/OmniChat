import { Minus, Moon, Plus } from "lucide-react";
import { useEffect, useState, type FormEvent } from "react";
import { api } from "../lib/api";
import { byKind } from "../lib/catalog";
import { formatBytes } from "../lib/format";
import { siteFor } from "../lib/sites";
import { AdvancedServiceSettings, advancedFrom, advancedPatch } from "./AdvancedServiceSettings";
import type { AppError, Service, Workspace } from "../lib/types";
import { Modal } from "./Modal";
import { ServiceIcon } from "./ServiceIcon";

export function ServiceSettingsDialog({
  service,
  workspaces,
  canMoveUp,
  canMoveDown,
  isActive,
  hibernated,
  hibernationOn,
  onHibernate,
  onMove,
  onClose,
  onChanged,
  onRemoved,
}: {
  service: Service;
  workspaces: Workspace[];
  canMoveUp: boolean;
  canMoveDown: boolean;
  /** Es el servicio que se está viendo. */
  isActive: boolean;
  hibernated: boolean;
  /** La hibernación automática está activada en los ajustes. */
  hibernationOn: boolean;
  onHibernate: () => Promise<void>;
  onMove: (dir: -1 | 1) => void;
  onClose: () => void;
  onChanged: (s: Service) => void;
  onRemoved: (id: string) => void;
}) {
  const [label, setLabel] = useState(service.label);
  const [url, setUrl] = useState(service.url ?? "");
  const [notify, setNotify] = useState(service.notify);
  const [sound, setSound] = useState(service.sound);
  const [workspaceId, setWorkspaceId] = useState(service.workspaceId ?? "");
  const [zoom, setZoom] = useState(service.zoom);
  const [hibernate, setHibernate] = useState(service.hibernate);
  const [advanced, setAdvanced] = useState(() => advancedFrom(service));
  const [extensionCount, setExtensionCount] = useState<number | null>(null);
  const [memory, setMemory] = useState<number | null>(null);

  useEffect(() => {
    api.extensionsInfo().then((i) => setExtensionCount(i.names.length)).catch(() => {});
    const load = () => api.serviceMemory().then((m) => setMemory(m[service.id] ?? 0)).catch(() => {});
    void load();
    const t = setInterval(load, 5000);
    return () => clearInterval(t);
  }, [service.id]);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const isCustom = service.kind === "custom";
  const site = isCustom ? siteFor(service.url) : undefined;
  const [iconMsg, setIconMsg] = useState<string | null>(null);

  const save = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      onChanged(
        await api.updateService(service.id, {
          label,
          notify,
          sound,
          workspaceId,
          zoom,
          hibernate,
          ...advancedPatch(advanced),
          url: isCustom && url !== service.url ? url : undefined,
        }),
      );
      onClose();
    } catch (err) {
      setError((err as AppError).message);
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setBusy(true);
    try {
      await api.removeService(service.id);
      onRemoved(service.id);
      onClose();
    } catch (err) {
      setError((err as AppError).message);
      setBusy(false);
    }
  };

  const stepZoom = (d: number) => setZoom((z) => Math.min(2, Math.max(0.5, Math.round((z + d) * 10) / 10)));

  return (
    <Modal title="Ajustes del servicio" onClose={onClose} width={460}>
      <form onSubmit={save} className="flex flex-col gap-4">
        <div className="flex items-center gap-3">
          <ServiceIcon kind={service.kind} label={service.label} url={service.url} icon={service.icon} size={36} />
          <div className="min-w-0">
            <p className="text-[13px] text-muted">
              {isCustom ? (site ? `${site.name} (web)` : "Web personalizada") : byKind[service.kind].name}
            </p>
            {isCustom && !site && (
              <button
                type="button"
                className="text-[12px] font-medium text-ink-3 hover:underline"
                onClick={() =>
                  api
                    .refreshServiceIcon(service.id)
                    .then(() => setIconMsg("Buscando el icono de la web… aparecerá en unos segundos si lo encuentra."))
                    .catch((e) => setIconMsg((e as AppError).message))
                }
              >
                {service.icon ? "Volver a descargar el icono de la web" : "Buscar el icono de la web"}
              </button>
            )}
            {iconMsg && <p className="text-[12px] text-faint">{iconMsg}</p>}
          </div>
          <p
            className="ml-auto shrink-0 text-right text-[12px] text-muted tabular-nums"
            title="Memoria que usan ahora los procesos de este servicio"
          >
            {hibernated ? "Hibernado" : memory ? `${formatBytes(memory)} de memoria` : ""}
          </p>
        </div>

        <div className="grid grid-cols-[1fr_170px] gap-2">
          <label className="flex flex-col gap-1">
            <span className="text-[12.5px] font-medium text-ink-3">Nombre</span>
            <input value={label} onChange={(e) => setLabel(e.target.value)} maxLength={40} className="field" />
          </label>
          <label className="flex flex-col gap-1">
            <span className="text-[12.5px] font-medium text-ink-3">Espacio</span>
            <select value={workspaceId} onChange={(e) => setWorkspaceId(e.target.value)} className="field">
              <option value="">Sin espacio</option>
              {workspaces.map((w) => (
                <option key={w.id} value={w.id}>
                  {w.name}
                </option>
              ))}
            </select>
          </label>
        </div>

        {isCustom && (
          <label className="flex flex-col gap-1">
            <span className="text-[12.5px] font-medium text-ink-3">Dirección web</span>
            <input value={url} onChange={(e) => setUrl(e.target.value)} className="field" />
          </label>
        )}

        <fieldset className="flex flex-col gap-2">
          <legend className="mb-1 text-[12.5px] font-medium text-ink-3">Notificaciones</legend>
          <label className="flex items-center gap-2.5 text-[13px]">
            <input
              type="checkbox"
              checked={notify}
              onChange={(e) => setNotify(e.target.checked)}
              className="h-4 w-4 accent-[var(--color-ink)]"
            />
            Avisarme cuando lleguen mensajes
          </label>
          <label className={`flex items-center gap-2.5 text-[13px] ${notify ? "" : "opacity-50"}`}>
            <input
              type="checkbox"
              checked={sound}
              disabled={!notify}
              onChange={(e) => setSound(e.target.checked)}
              className="h-4 w-4 accent-[var(--color-ink)]"
            />
            Con sonido
          </label>
        </fieldset>

        <fieldset className="flex flex-col gap-2">
          <legend className="mb-1 text-[12.5px] font-medium text-ink-3">Memoria</legend>
          <label className="flex items-start gap-2.5 text-[13px]">
            <input
              type="checkbox"
              checked={hibernate}
              onChange={(e) => setHibernate(e.target.checked)}
              className="mt-0.5 h-4 w-4 accent-[var(--color-ink)]"
            />
            <span>
              Hibernar si no lo uso durante un rato
              <span className="block text-[12px] text-muted">
                {hibernationOn
                  ? "Libera memoria, pero no avisa de mensajes nuevos hasta que lo abras."
                  : "La hibernación automática está desactivada en los ajustes de OmniChat."}
              </span>
            </span>
          </label>
          <div className="flex items-center gap-2">
            <button
              type="button"
              className="btn-ghost border border-line"
              disabled={busy || isActive || hibernated}
              onClick={async () => {
                setBusy(true);
                setError(null);
                try {
                  await onHibernate();
                  onClose();
                } catch (err) {
                  setError((err as AppError).message);
                  setBusy(false);
                }
              }}
            >
              <Moon size={14} /> Hibernar ahora
            </button>
            <span className="text-[12px] text-faint">
              {hibernated ? "Ya está hibernado." : isActive ? "Abre otro servicio para hibernar este." : ""}
            </span>
          </div>
        </fieldset>

        <div className="flex items-center gap-3">
          <span className="text-[12.5px] font-medium text-ink-3">Tamaño</span>
          <div className="flex items-center rounded-md border border-line">
            <button type="button" aria-label="Reducir" className="grid h-8 w-8 place-items-center hover:bg-fog" onClick={() => stepZoom(-0.1)}>
              <Minus size={14} />
            </button>
            <button
              type="button"
              className="h-8 w-14 border-x border-line text-[12.5px] tabular-nums hover:bg-fog"
              title="Volver al 100 %"
              onClick={() => setZoom(1)}
            >
              {Math.round(zoom * 100)} %
            </button>
            <button type="button" aria-label="Ampliar" className="grid h-8 w-8 place-items-center hover:bg-fog" onClick={() => stepZoom(0.1)}>
              <Plus size={14} />
            </button>
          </div>
          <span className="text-[12px] text-faint">También con Ctrl + / Ctrl −</span>
        </div>

        <AdvancedServiceSettings value={advanced} onChange={setAdvanced} extensionCount={extensionCount} />

        <div className="flex gap-2">
          <button type="button" className="btn-ghost border border-line" disabled={!canMoveUp} onClick={() => onMove(-1)}>
            Subir en la barra
          </button>
          <button type="button" className="btn-ghost border border-line" disabled={!canMoveDown} onClick={() => onMove(1)}>
            Bajar
          </button>
        </div>

        {error && (
          <p role="alert" className="text-[12.5px] text-bad">
            {error}
          </p>
        )}

        <div className="flex justify-end gap-2">
          <button type="button" onClick={onClose} className="btn-ghost">
            Cancelar
          </button>
          <button type="submit" disabled={busy} className="btn-primary">
            Guardar
          </button>
        </div>
      </form>

      <div className="mt-5 border-t border-line pt-4">
        {confirmRemove ? (
          <div className="flex flex-col gap-3">
            <p className="text-[13px]">
              Se cerrará la sesión de «{service.label}» en OmniChat y se borrarán sus datos de este equipo. Tus mensajes
              siguen en tu cuenta.
            </p>
            <div className="flex gap-2">
              <button type="button" className="btn-ghost" onClick={() => setConfirmRemove(false)}>
                Cancelar
              </button>
              <button type="button" className="btn bg-bad text-white hover:bg-bad/90" disabled={busy} onClick={remove}>
                Quitar servicio
              </button>
            </div>
          </div>
        ) : (
          <button
            type="button"
            className="text-[12.5px] font-medium text-bad hover:underline"
            onClick={() => setConfirmRemove(true)}
          >
            Quitar servicio y cerrar su sesión
          </button>
        )}
      </div>
    </Modal>
  );
}
