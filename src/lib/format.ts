const mb = new Intl.NumberFormat("es", { maximumFractionDigits: 0 });
const gb = new Intl.NumberFormat("es", { maximumFractionDigits: 1 });

/** 412 MB, 1,3 GB… */
export function formatBytes(bytes: number): string {
  if (bytes >= 1e9) return `${gb.format(bytes / 1e9)} GB`;
  return `${mb.format(Math.max(1, bytes / 1e6))} MB`;
}
