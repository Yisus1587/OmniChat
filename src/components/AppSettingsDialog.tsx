import { Download, Upload } from "lucide-react";
import { useRef, useState, type FormEvent, type ReactNode } from "react";
import { api } from "../lib/api";
import type { AppError, LockStatus, Service, Settings, Theme } from "../lib/types";
import { ExtensionsSection, MemorySection, SyncSection } from "./SettingsExtras";
import { Modal } from "./Modal";

const time = new Intl.DateTimeFormat("es", { hour: "2-digit", minute: "2-digit" });

const THEMES: { value: Theme; label: string }[] = [
  { value: "system", label: "Como Windows" },
  { value: "light", label: "Claro" },
  { value: "dark", label: "Oscuro" },
];

const SHORTCUTS: [string, string][] = [
  ["Ctrl + 1 … 9", "Ir al servicio 1…9 de la barra"],
  ["Ctrl + Tab", "Servicio siguiente (con Mayús, el anterior)"],
  ["Ctrl + K", "Buscar y saltar a un servicio"],
  ["Ctrl + / Ctrl −", "Ampliar o reducir el servicio"],
  ["Ctrl + 0", "Volver al tamaño normal"],
  ["Ctrl + L", "Bloquear OmniChat (si tienes PIN)"],
];

const HIBERNATE_AFTER = [15, 30, 60, 120, 240];
const LOCK_AFTER = [0, 5, 15, 30, 60];

function minutes(m: number): string {
  if (m === 0) return "Nunca";
  return m < 60 ? `${m} min` : m === 60 ? "1 hora" : `${m / 60} horas`;
}

export function AppSettingsDialog({
  settings,
  lock,
  services,
  hibernated,
  onChanged,
  onLockChanged,
  onImported,
  onClose,
}: {
  settings: Settings;
  lock: LockStatus;
  services: Service[];
  hibernated: Set<string>;
  onChanged: (s: Settings) => void;
  onLockChanged: (s: LockStatus) => void;
  /** Tras importar: recargar servicios y espacios. */
  onImported: () => void;
  onClose: () => void;
}) {
  const [error, setError] = useState<string | null>(null);
  const focusActive = settings.focusUntil !== null && settings.focusUntil > Date.now();

  const save = async (patch: Partial<Settings>) => {
    setError(null);
    try {
      onChanged(await api.updateSettings({ ...settings, ...patch }));
    } catch (e) {
      setError((e as AppError).message);
    }
  };

  const focus = async (opts: { minutes?: number; untilTomorrow?: boolean }) => {
    setError(null);
    try {
      onChanged(await api.setFocus(opts));
    } catch (e) {
      setError((e as AppError).message);
    }
  };

  return (
    <Modal title="Ajustes" onClose={onClose} width={500}>
      <div className="flex flex-col gap-6">
        <Section title="Concentración">
          <p className="text-[12.5px] text-muted">
            {focusActive
              ? `Notificaciones silenciadas hasta las ${time.format(settings.focusUntil!)}.`
              : "Silencia todas las notificaciones durante un rato. Los mensajes siguen llegando."}
          </p>
          <div className="flex flex-wrap gap-2">
            <button type="button" className="btn-ghost border border-line" onClick={() => focus({ minutes: 60 })}>
              1 hora
            </button>
            <button type="button" className="btn-ghost border border-line" onClick={() => focus({ minutes: 180 })}>
              3 horas
            </button>
            <button type="button" className="btn-ghost border border-line" onClick={() => focus({ untilTomorrow: true })}>
              Hasta mañana (8:00)
            </button>
            {focusActive && (
              <button type="button" className="btn-primary" onClick={() => focus({})}>
                Reactivar ahora
              </button>
            )}
          </div>

          <label className="mt-2 flex items-center gap-2.5 text-[13px]">
            <input
              type="checkbox"
              checked={settings.quietHours.enabled}
              onChange={(e) => save({ quietHours: { ...settings.quietHours, enabled: e.target.checked } })}
              className="h-4 w-4 accent-[var(--color-ink)]"
            />
            Silenciar todos los días de
            <input
              type="time"
              value={settings.quietHours.start}
              onChange={(e) => e.target.value && save({ quietHours: { ...settings.quietHours, start: e.target.value } })}
              className="field w-[136px] py-1"
              aria-label="Inicio del horario silencioso"
            />
            a
            <input
              type="time"
              value={settings.quietHours.end}
              onChange={(e) => e.target.value && save({ quietHours: { ...settings.quietHours, end: e.target.value } })}
              className="field w-[136px] py-1"
              aria-label="Fin del horario silencioso"
            />
          </label>
        </Section>

        <Section title="Notificaciones">
          <label className="flex items-start gap-2.5 text-[13px]">
            <input
              type="checkbox"
              checked={settings.hideContent}
              onChange={(e) => save({ hideContent: e.target.checked })}
              className="mt-0.5 h-4 w-4 accent-[var(--color-ink)]"
            />
            <span>
              Ocultar el remitente y el texto
              <span className="block text-[12px] text-muted">Mostrará sólo «Nuevo mensaje» y el nombre del servicio.</span>
            </span>
          </label>
          <p className="text-[12px] text-muted">El sonido y los avisos de cada servicio se cambian en sus ajustes (clic derecho en su icono).</p>
        </Section>

        <Section title="Ahorro de memoria">
          <label className="flex flex-wrap items-center gap-2.5 text-[13px]">
            <input
              type="checkbox"
              checked={settings.hibernation.enabled}
              onChange={(e) => save({ hibernation: { ...settings.hibernation, enabled: e.target.checked } })}
              className="h-4 w-4 accent-[var(--color-ink)]"
            />
            Hibernar los servicios que no abra en
            <select
              value={settings.hibernation.minutes}
              onChange={(e) => save({ hibernation: { ...settings.hibernation, minutes: Number(e.target.value) } })}
              className="field w-auto py-1"
              aria-label="Tiempo antes de hibernar"
            >
              {HIBERNATE_AFTER.map((m) => (
                <option key={m} value={m}>
                  {minutes(m)}
                </option>
              ))}
            </select>
          </label>
          <p className="text-[12px] text-muted">
            Un servicio hibernado se cierra para liberar memoria y vuelve a cargarse al abrirlo. Mientras tanto no
            avisa de mensajes nuevos: desactívalo para tus chats importantes en sus ajustes (clic derecho en su icono).
          </p>
        </Section>

        <MemorySection services={services} hibernated={hibernated} />

        <PinSection lock={lock} settings={settings} save={save} onLockChanged={onLockChanged} />

        <SyncSection settings={settings} onChanged={onChanged} />

        <BackupSection onImported={onImported} />

        <ExtensionsSection />

        <Section title="Apariencia">
          <div className="flex gap-1 rounded-lg border border-line p-1" role="radiogroup" aria-label="Tema">
            {THEMES.map((t) => (
              <button
                key={t.value}
                type="button"
                role="radio"
                aria-checked={settings.theme === t.value}
                onClick={() => save({ theme: t.value })}
                className={`flex-1 rounded-md px-3 py-1.5 text-[13px] font-medium transition-colors ${
                  settings.theme === t.value ? "bg-ink text-paper" : "text-muted hover:bg-fog hover:text-ink"
                }`}
              >
                {t.label}
              </button>
            ))}
          </div>
        </Section>

        <Section title="Inicio">
          <label className="flex items-start gap-2.5 text-[13px]">
            <input
              type="checkbox"
              checked={settings.autostart}
              onChange={(e) => save({ autostart: e.target.checked })}
              className="mt-0.5 h-4 w-4 accent-[var(--color-ink)]"
            />
            <span>
              Abrir OmniChat al iniciar Windows
              <span className="block text-[12px] text-muted">Arranca minimizado junto al reloj para no perder mensajes.</span>
            </span>
          </label>
        </Section>

        <Section title="Atajos de teclado">
          <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-[13px]">
            {SHORTCUTS.map(([keys, what]) => (
              <div key={keys} className="contents">
                <dt>
                  <kbd className="rounded border border-line bg-surface px-1.5 py-0.5 text-[12px]">{keys}</kbd>
                </dt>
                <dd className="text-muted">{what}</dd>
              </div>
            ))}
          </dl>
        </Section>

        {error && (
          <p role="alert" className="text-[12.5px] text-bad">
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
}

function PinSection({
  lock,
  settings,
  save,
  onLockChanged,
}: {
  lock: LockStatus;
  settings: Settings;
  save: (patch: Partial<Settings>) => Promise<void>;
  onLockChanged: (s: LockStatus) => void;
}) {
  const [mode, setMode] = useState<"idle" | "change" | "remove">("idle");
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [repeat, setRepeat] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);

  const digits = (v: string) => v.replace(/\D/g, "").slice(0, 8);
  const reset = () => {
    setMode("idle");
    setCurrent("");
    setNext("");
    setRepeat("");
  };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    setDone(null);
    const removing = mode === "remove";
    if (!removing) {
      if (next.length < 4) return setError("El PIN debe tener entre 4 y 8 cifras.");
      if (next !== repeat) return setError("Los dos PIN no coinciden.");
    }
    try {
      onLockChanged(await api.setPin(lock.hasPin ? current : null, removing ? null : next));
      setDone(removing ? "PIN quitado. OmniChat ya no se bloquea." : lock.hasPin ? "PIN cambiado." : "Bloqueo activado.");
      reset();
    } catch (err) {
      setError((err as AppError).message);
    }
  };

  const pinField = (value: string, set: (v: string) => void, label: string) => (
    <label className="flex flex-col gap-1">
      <span className="text-[12px] text-muted">{label}</span>
      <input
        type="password"
        inputMode="numeric"
        autoComplete="off"
        value={value}
        onChange={(e) => set(digits(e.target.value))}
        className="field w-[130px] tracking-[0.3em]"
      />
    </label>
  );

  const showForm = !lock.hasPin || mode !== "idle";

  return (
    <Section title="Bloqueo con PIN">
      <p className="text-[12.5px] text-muted">
        {lock.hasPin
          ? "OmniChat pide el PIN al abrirse y cuando se bloquea. Mientras está bloqueado, los avisos no muestran remitente ni texto."
          : "Pide un PIN para ver tus servicios. Protege frente a quien use tu equipo un momento; no cifra los datos."}
      </p>

      {lock.hasPin && (
        <>
          <label className="flex items-center gap-2.5 text-[13px]">
            <input
              type="checkbox"
              checked={settings.lock.onHide}
              onChange={(e) => save({ lock: { ...settings.lock, onHide: e.target.checked } })}
              className="h-4 w-4 accent-[var(--color-ink)]"
            />
            Bloquear al cerrar la ventana
          </label>
          <label className="flex items-center gap-2.5 text-[13px]">
            Bloquear tras no usar OmniChat durante
            <select
              value={settings.lock.idleMinutes}
              onChange={(e) => save({ lock: { ...settings.lock, idleMinutes: Number(e.target.value) } })}
              className="field w-auto py-1"
              aria-label="Bloquear por inactividad"
            >
              {LOCK_AFTER.map((m) => (
                <option key={m} value={m}>
                  {minutes(m)}
                </option>
              ))}
            </select>
          </label>
          {mode === "idle" && (
            <div className="flex flex-wrap gap-2">
              <button type="button" className="btn-primary" onClick={() => void api.lockNow().then(onLockChanged)}>
                Bloquear ahora
              </button>
              <button type="button" className="btn-ghost border border-line" onClick={() => setMode("change")}>
                Cambiar PIN
              </button>
              <button type="button" className="btn-ghost text-bad" onClick={() => setMode("remove")}>
                Quitar PIN
              </button>
            </div>
          )}
        </>
      )}

      {showForm && (
        <form onSubmit={submit} className="flex flex-col gap-3 rounded-lg border border-line p-3">
          <div className="flex flex-wrap gap-3">
            {lock.hasPin && pinField(current, setCurrent, "PIN actual")}
            {mode !== "remove" && pinField(next, setNext, lock.hasPin ? "PIN nuevo" : "PIN (4 a 8 cifras)")}
            {mode !== "remove" && pinField(repeat, setRepeat, "Repítelo")}
          </div>
          <div className="flex gap-2">
            <button type="submit" className={mode === "remove" ? "btn bg-bad text-white hover:bg-bad/90" : "btn-primary"}>
              {mode === "remove" ? "Quitar PIN" : lock.hasPin ? "Guardar PIN nuevo" : "Activar bloqueo"}
            </button>
            {lock.hasPin && (
              <button type="button" className="btn-ghost" onClick={reset}>
                Cancelar
              </button>
            )}
          </div>
        </form>
      )}
      {error && (
        <p role="alert" className="text-[12.5px] text-bad">
          {error}
        </p>
      )}
      {done && <p className="text-[12.5px] text-ok">{done}</p>}
    </Section>
  );
}

function BackupSection({ onImported }: { onImported: () => void }) {
  const file = useRef<HTMLInputElement>(null);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  const exportNow = async () => {
    setMessage(null);
    try {
      const path = await api.exportConfig();
      setMessage({ ok: true, text: `Guardado en ${path}` });
    } catch (e) {
      setMessage({ ok: false, text: (e as AppError).message });
    }
  };

  const importFile = async (f: File | undefined) => {
    if (!f) return;
    setMessage(null);
    try {
      const r = await api.importConfig(await f.text());
      onImported();
      const parts = [
        r.services === 1 ? "1 servicio añadido" : `${r.services} servicios añadidos`,
        r.workspaces > 0 && (r.workspaces === 1 ? "1 espacio" : `${r.workspaces} espacios`),
        r.skipped > 0 && `${r.skipped} ya estaban`,
      ].filter(Boolean);
      setMessage({
        ok: true,
        text: `${parts.join(", ")}. Ajustes aplicados.${r.services > 0 ? " Inicia sesión en los servicios nuevos." : ""}`,
      });
    } catch (e) {
      setMessage({ ok: false, text: (e as AppError).message });
    } finally {
      if (file.current) file.current.value = "";
    }
  };

  return (
    <Section title="Copia de seguridad">
      <p className="text-[12.5px] text-muted">
        Guarda tus servicios, espacios y ajustes en un fichero para llevarlos a otro equipo. No incluye las sesiones
        ni el PIN: al importar tendrás que iniciar sesión en cada servicio.
      </p>
      <div className="flex flex-wrap gap-2">
        <button type="button" className="btn-ghost border border-line" onClick={exportNow}>
          <Download size={14} /> Exportar configuración
        </button>
        <button type="button" className="btn-ghost border border-line" onClick={() => file.current?.click()}>
          <Upload size={14} /> Importar…
        </button>
        <input
          ref={file}
          type="file"
          accept=".json,application/json"
          className="hidden"
          onChange={(e) => void importFile(e.target.files?.[0])}
        />
      </div>
      {message && (
        <p role={message.ok ? "status" : "alert"} className={`break-all text-[12.5px] ${message.ok ? "text-ok" : "text-bad"}`}>
          {message.text}
        </p>
      )}
    </Section>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2">
      <h3 className="text-[13px] font-semibold">{title}</h3>
      {children}
    </section>
  );
}
