import { ChevronRight } from "lucide-react";
import type { ReactNode } from "react";

export type UaMode = "" | "mobile" | "chrome" | "custom";

export interface AdvancedValues {
  uaMode: UaMode;
  uaCustom: string;
  proxy: string;
  extensions: boolean;
  css: string;
  js: string;
}

export function advancedFrom(s: {
  userAgent: string | null;
  proxy: string | null;
  extensions: boolean;
  customCss: string | null;
  customJs: string | null;
}): AdvancedValues {
  const ua = s.userAgent ?? "";
  const preset = ua === "" || ua === "mobile" || ua === "chrome";
  return {
    uaMode: preset ? (ua as UaMode) : "custom",
    uaCustom: preset ? "" : ua,
    proxy: s.proxy ?? "",
    extensions: s.extensions,
    css: s.customCss ?? "",
    js: s.customJs ?? "",
  };
}

export function advancedPatch(v: AdvancedValues) {
  return {
    userAgent: v.uaMode === "custom" ? v.uaCustom.trim() : v.uaMode,
    proxy: v.proxy.trim(),
    extensions: v.extensions,
    customCss: v.css,
    customJs: v.js,
  };
}

/** Opciones poco habituales de un servicio, plegadas por defecto. */
export function AdvancedServiceSettings({
  value,
  onChange,
  extensionCount,
}: {
  value: AdvancedValues;
  onChange: (v: AdvancedValues) => void;
  extensionCount: number | null;
}) {
  const set = (patch: Partial<AdvancedValues>) => onChange({ ...value, ...patch });
  const used = [
    value.uaMode && "vista",
    value.proxy.trim() && "proxy",
    value.extensions && "extensiones",
    (value.css.trim() || value.js.trim()) && "código propio",
  ].filter(Boolean);

  return (
    <details className="group rounded-lg border border-line">
      <summary className="flex cursor-pointer list-none items-center gap-2 px-3 py-2.5 text-[13px] font-medium select-none">
        <ChevronRight size={14} className="text-muted transition-transform group-open:rotate-90" />
        Avanzado
        {used.length > 0 && <span className="text-[12px] font-normal text-muted">({used.join(", ")})</span>}
      </summary>
      <div className="flex flex-col gap-4 border-t border-line px-3 py-3">
        <Field label="Vista" hint="Algunas webs se ven mejor (o sólo funcionan) identificándose como otro navegador.">
          <select value={value.uaMode} onChange={(e) => set({ uaMode: e.target.value as UaMode })} className="field">
            <option value="">Normal (Edge de escritorio)</option>
            <option value="mobile">Móvil</option>
            <option value="chrome">Chrome de escritorio</option>
            <option value="custom">Agente de usuario personalizado…</option>
          </select>
          {value.uaMode === "custom" && (
            <input
              value={value.uaCustom}
              onChange={(e) => set({ uaCustom: e.target.value })}
              placeholder="Mozilla/5.0 (…)"
              maxLength={400}
              className="field mt-1.5 font-mono text-[12px]"
            />
          )}
        </Field>

        <Field label="Proxy" hint="Sólo para este servicio. Formato http://servidor:puerto o socks5://servidor:puerto, sin usuario ni contraseña.">
          <input
            value={value.proxy}
            onChange={(e) => set({ proxy: e.target.value })}
            placeholder="Sin proxy"
            className="field font-mono text-[12px]"
          />
        </Field>

        <label className="flex items-start gap-2.5 text-[13px]">
          <input
            type="checkbox"
            checked={value.extensions}
            onChange={(e) => set({ extensions: e.target.checked })}
            className="mt-0.5 h-4 w-4 accent-[var(--color-ink)]"
          />
          <span>
            Cargar las extensiones de Chrome
            <span className="block text-[12px] text-muted">
              {extensionCount === null
                ? "Se gestionan en los ajustes de OmniChat."
                : extensionCount === 0
                  ? "Todavía no hay ninguna instalada: añádelas desde los ajustes de OmniChat."
                  : `${extensionCount === 1 ? "1 extensión instalada" : `${extensionCount} extensiones instaladas`} (p. ej. un bloqueador de anuncios).`}
            </span>
          </span>
        </label>

        <Field label="CSS propio" hint="Se aplica sobre la web. Útil para ocultar elementos o cambiar colores.">
          <textarea
            value={value.css}
            onChange={(e) => set({ css: e.target.value })}
            rows={4}
            spellCheck={false}
            placeholder={'/* p. ej. */\n[aria-label="Stories"] { display: none !important; }'}
            className="field font-mono text-[12px]"
          />
        </Field>

        <Field
          label="JavaScript propio"
          hint="Se ejecuta dentro de la web con tu sesión iniciada. Pega sólo código que entiendas: un script malicioso podría leer o enviar tus mensajes."
          warn
        >
          <textarea
            value={value.js}
            onChange={(e) => set({ js: e.target.value })}
            rows={4}
            spellCheck={false}
            placeholder="// Se ejecuta al cargar la página"
            className="field font-mono text-[12px]"
          />
        </Field>
        <p className="text-[12px] text-faint">Al guardar, el servicio se recarga para aplicar estos cambios.</p>
      </div>
    </details>
  );
}

function Field({ label, hint, warn, children }: { label: string; hint: string; warn?: boolean; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1">
      <span className="text-[12.5px] font-medium text-ink-3">{label}</span>
      {children}
      <span className={`text-[12px] ${warn ? "text-warn" : "text-muted"}`}>{hint}</span>
    </div>
  );
}
