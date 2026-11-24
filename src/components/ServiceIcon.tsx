import { useId, useMemo, type ReactNode } from "react";
import { colorFor } from "../lib/catalog";
import { LOGOS, type Logo as LogoData } from "../lib/logos";
import { siteFor } from "../lib/sites";
import type { ServiceKind } from "../lib/types";

/**
 * Logo del servicio sobre una baldosa blanca redondeada (como un icono de app).
 *
 * Webs personalizadas, por orden: logo oficial si es una web conocida (Discord, Slack…),
 * el icono descargado de la propia web, o sus iniciales sobre un color derivado del nombre.
 */
export function ServiceIcon({
  kind,
  size = 36,
  label = "",
  url,
  icon,
}: {
  kind: ServiceKind;
  size?: number;
  label?: string;
  url?: string | null;
  icon?: string | null;
}) {
  const radius = Math.round(size * 0.28);

  if (kind !== "custom") return <Logo logo={LOGOS[kind]} size={size} radius={radius} />;

  const site = siteFor(url);
  if (site) return <Logo logo={site.logo} size={size} radius={radius} />;

  if (icon) {
    return (
      <Tile size={size} radius={radius}>
        <img src={icon} alt="" width={Math.round(size * 0.72)} height={Math.round(size * 0.72)} className="object-contain" draggable={false} />
      </Tile>
    );
  }

  const letters = initials(label);
  return (
    <span
      aria-hidden
      className="grid shrink-0 place-items-center font-semibold text-white"
      style={{
        width: size,
        height: size,
        borderRadius: radius,
        background: colorFor(label || "web"),
        fontSize: Math.round(size * (letters.length > 1 ? 0.36 : 0.46)),
      }}
    >
      {letters}
    </span>
  );
}

export function initials(label: string): string {
  const words = label.trim().split(/[\s._-]+/).filter(Boolean);
  if (words.length === 0) return "?";
  if (words.length === 1) return words[0].slice(0, 2).toUpperCase();
  return (words[0][0] + words[1][0]).toUpperCase();
}

function Tile({ size, radius, children }: { size: number; radius: number; children: ReactNode }) {
  return (
    <span
      aria-hidden
      className="grid shrink-0 place-items-center overflow-hidden bg-white shadow-[inset_0_0_0_1px_rgba(23,32,44,0.08)]"
      style={{ width: size, height: size, borderRadius: radius }}
    >
      {children}
    </span>
  );
}

function Logo({ logo, size, radius }: { logo: LogoData; size: number; radius: number }) {
  const uid = useId().replace(/[^a-zA-Z0-9_-]/g, "");

  // Los SVG con degradados usan ids internos (`id="a"` … `url(#a)`): los hacemos únicos por
  // instancia para que varios iconos en pantalla no se pisen.
  const body = useMemo(
    () =>
      logo.body
        .replace(/id="([^"]+)"/g, (_, id) => `id="${id}-${uid}"`)
        .replace(/url\(#([^)]+)\)/g, (_, id) => `url(#${id}-${uid})`)
        .replace(/href="#([^"]+)"/g, (_, id) => `href="#${id}-${uid}"`),
    [logo.body, uid],
  );

  return (
    <Tile size={size} radius={radius}>
      <svg
        viewBox={`0 0 ${logo.width} ${logo.height}`}
        width={Math.round(size * 0.66)}
        height={Math.round(size * 0.66)}
        aria-hidden
        // Contenido estático incluido en la app (src/lib/logos.ts y sites.ts), nunca datos externos.
        dangerouslySetInnerHTML={{ __html: body }}
      />
    </Tile>
  );
}
