import { Globe } from "lucide-react";
import { useState, type FormEvent } from "react";
import { api } from "../lib/api";
import { CATALOG, byKind } from "../lib/catalog";
import { SITES, siteFor } from "../lib/sites";
import type { AppError, Service, ServiceKind, Workspace } from "../lib/types";
import { Modal } from "./Modal";
import { ServiceIcon } from "./ServiceIcon";

export function AddServiceDialog({
  existing,
  workspaces,
  defaultWorkspace,
  onClose,
  onAdded,
}: {
  existing: Service[];
  workspaces: Workspace[];
  defaultWorkspace: string | null;
  onClose: () => void;
  onAdded: (s: Service) => void;
}) {
  const [kind, setKind] = useState<ServiceKind | null>(null);
  const [label, setLabel] = useState("");
  const [url, setUrl] = useState("");
  const [workspaceId, setWorkspaceId] = useState<string>(defaultWorkspace ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isCustom = kind === "custom";
  const sameKind = kind ? existing.filter((s) => s.kind === kind).length : 0;
  const site = isCustom ? siteFor(url) : undefined;
  const sameSite = site ? existing.filter((s) => s.kind === "custom" && siteFor(s.url)?.id === site.id).length : 0;
  const suggested = isCustom
    ? site
      ? sameSite
        ? `${site.name} ${sameSite + 1}`
        : site.name
      : ""
    : !kind
      ? ""
      : sameKind
        ? `${byKind[kind].name} ${sameKind + 1}`
        : byKind[kind].name;

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!kind) return;
    setBusy(true);
    setError(null);
    try {
      onAdded(
        await api.addService(kind, {
          label: label.trim() || suggested || undefined,
          url: isCustom ? url : undefined,
          workspaceId: workspaceId || null,
        }),
      );
    } catch (err) {
      setError((err as AppError).message);
      setBusy(false);
    }
  };

  return (
    <Modal title="Añadir servicio" onClose={onClose} width={540}>
      <form onSubmit={submit}>
        <div className="grid grid-cols-2 gap-2" role="radiogroup" aria-label="Servicio">
          {CATALOG.map((c) => {
            const selected = kind === c.kind;
            return (
              <button
                key={c.kind}
                type="button"
                role="radio"
                aria-checked={selected}
                onClick={() => setKind(c.kind)}
                className={`flex items-center gap-3 rounded-lg border p-3 text-left transition-colors ${
                  selected ? "border-ink bg-surface ring-1 ring-ink" : "border-line hover:border-ink-3"
                }`}
              >
                <ServiceIcon kind={c.kind} label={c.kind === "custom" ? "+" : c.name} size={34} />
                <span className="min-w-0">
                  <span className="block text-[14px] font-medium">{c.name}</span>
                  <span className="block text-[12px] leading-snug text-muted">{c.detail}</span>
                </span>
              </button>
            );
          })}
        </div>

        {kind && (
          <div className="mt-4 grid gap-3">
            {isCustom && (
              <>
                <div>
                  <p className="mb-1.5 text-[12.5px] font-medium text-ink-3">Webs populares</p>
                  <div className="scroll-quiet grid max-h-[180px] grid-cols-3 gap-1 overflow-y-auto pr-1">
                    {SITES.filter((x) => x.url).map((x) => {
                      const picked = site?.id === x.id;
                      return (
                        <button
                          key={x.id}
                          type="button"
                          aria-pressed={picked}
                          onClick={() => setUrl(x.url!)}
                          className={`flex min-w-0 items-center gap-2 rounded-md border px-2 py-1.5 text-left text-[12.5px] transition-colors ${
                            picked ? "border-ink bg-surface ring-1 ring-ink" : "border-transparent hover:bg-fog"
                          }`}
                        >
                          <ServiceIcon kind="custom" url={x.url} label={x.name} size={22} />
                          <span className="truncate">{x.name}</span>
                        </button>
                      );
                    })}
                  </div>
                </div>
                <label className="flex flex-col gap-1">
                  <span className="text-[12.5px] font-medium text-ink-3">
                    Dirección web{" "}
                    <span className="font-normal text-faint">(o elige una de arriba; las demás usan el icono de su web)</span>
                  </span>
                  <span className="flex items-center gap-2">
                    {url.trim() ? (
                      <ServiceIcon kind="custom" url={url} label={label || suggested || url} size={30} />
                    ) : (
                      <span className="grid h-[30px] w-[30px] shrink-0 place-items-center rounded-lg border border-dashed border-line text-faint">
                        <Globe size={15} />
                      </span>
                    )}
                    <input
                      value={url}
                      onChange={(e) => setUrl(e.target.value)}
                      placeholder="discord.com/app"
                      required
                      autoFocus
                      className="field"
                    />
                  </span>
                </label>
              </>
            )}
            <div className="grid grid-cols-[1fr_180px] gap-2">
              <label className="flex flex-col gap-1">
                <span className="text-[12.5px] font-medium text-ink-3">
                  Nombre <span className="font-normal text-faint">(útil si tienes varias cuentas)</span>
                </span>
                <input
                  value={label}
                  onChange={(e) => setLabel(e.target.value)}
                  placeholder={suggested || "Nombre del servicio"}
                  maxLength={40}
                  className="field"
                />
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
          </div>
        )}

        {error && (
          <p role="alert" className="mt-3 text-[12.5px] text-bad">
            {error}
          </p>
        )}

        <div className="mt-5 flex items-center justify-end gap-2">
          <p className="mr-auto text-[12px] text-muted">Iniciarás sesión en la web oficial del servicio.</p>
          <button type="button" onClick={onClose} className="btn-ghost">
            Cancelar
          </button>
          <button type="submit" disabled={!kind || busy || (isCustom && !url.trim())} className="btn-primary">
            {busy ? "Añadiendo…" : "Añadir"}
          </button>
        </div>
      </form>
    </Modal>
  );
}
