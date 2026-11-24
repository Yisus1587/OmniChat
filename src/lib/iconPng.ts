// Dibuja el icono de un servicio como PNG para los avisos de Windows (Windows necesita un
// fichero de imagen; los logos de la app son SVG). Mismo aspecto que `ServiceIcon`.
import { initials } from "../components/ServiceIcon";
import { colorFor } from "./catalog";
import { LOGOS, type Logo } from "./logos";
import { siteFor } from "./sites";
import type { Service } from "./types";

const SIZE = 128;

function roundedRect(ctx: CanvasRenderingContext2D, size: number, radius: number) {
  ctx.beginPath();
  ctx.roundRect(0, 0, size, size, radius);
  ctx.closePath();
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = reject;
    img.src = src;
  });
}

function svgUrl(logo: Logo): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 ${logo.width} ${logo.height}" width="${logo.width}" height="${logo.height}">${logo.body}</svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

/** Dibuja una imagen centrada dentro de un cuadro de `box` px. */
function drawCentered(ctx: CanvasRenderingContext2D, img: HTMLImageElement, w: number, h: number, box: number) {
  const scale = Math.min(box / w, box / h);
  const [dw, dh] = [w * scale, h * scale];
  ctx.drawImage(img, (SIZE - dw) / 2, (SIZE - dh) / 2, dw, dh);
}

export async function servicePng(service: Pick<Service, "kind" | "label" | "url" | "icon">): Promise<number[]> {
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = SIZE;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("canvas no disponible");
  const radius = SIZE * 0.24;

  const logo = service.kind === "custom" ? siteFor(service.url)?.logo : LOGOS[service.kind];
  const downloaded = service.kind === "custom" && !logo ? service.icon : null;
  const downloadedImg = downloaded ? await loadImage(downloaded).catch(() => null) : null;

  if (logo || downloadedImg) {
    roundedRect(ctx, SIZE, radius);
    ctx.fillStyle = "#fff";
    ctx.fill();
    if (logo) {
      drawCentered(ctx, await loadImage(svgUrl(logo)), logo.width, logo.height, SIZE * 0.7);
    } else if (downloadedImg) {
      ctx.save();
      roundedRect(ctx, SIZE, radius);
      ctx.clip();
      const w = downloadedImg.naturalWidth || SIZE;
      const h = downloadedImg.naturalHeight || SIZE;
      drawCentered(ctx, downloadedImg, w, h, SIZE * 0.76);
      ctx.restore();
    }
  } else {
    await document.fonts.ready;
    const letters = initials(service.label);
    roundedRect(ctx, SIZE, radius);
    ctx.fillStyle = colorFor(service.label || "web");
    ctx.fill();
    ctx.fillStyle = "#fff";
    ctx.font = `600 ${Math.round(SIZE * (letters.length > 1 ? 0.4 : 0.5))}px "Instrument Sans Variable", system-ui, sans-serif`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(letters, SIZE / 2, SIZE / 2 + SIZE * 0.03);
  }

  const blob = await new Promise<Blob | null>((r) => canvas.toBlob(r, "image/png"));
  if (!blob) throw new Error("no se pudo generar el icono");
  return Array.from(new Uint8Array(await blob.arrayBuffer()));
}
