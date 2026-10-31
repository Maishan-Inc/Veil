/** Format a byte count as a human-readable string. */
export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || !Number.isFinite(bytes) || bytes < 0) return "—";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"] as const;
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const precision = value >= 100 ? 0 : value >= 10 ? 1 : 2;
  return `${value.toFixed(precision)} ${units[unit]}`;
}

/** Format a bytes-per-second rate. */
export function formatRate(bytesPerSec: number | null | undefined): string {
  if (bytesPerSec == null || bytesPerSec <= 0) return "—";
  return formatBytes(bytesPerSec);
}

/** Compute a 0-100 percent for a partial download or extraction. */
export function percentOf(done: number, total: number | null | undefined): number | null {
  if (total == null || total <= 0) return null;
  if (done <= 0) return 0;
  if (done >= total) return 100;
  return Math.floor((done / total) * 100);
}

/** Truncated path for tight columns (keeps the leading drive + tail). */
export function shortenPath(path: string, max = 60): string {
  if (path.length <= max) return path;
  const head = path.slice(0, 18);
  const tail = path.slice(-(max - head.length - 1));
  return `${head}…${tail}`;
}

/** Locale-aware short timestamp. */
export function formatTimestamp(value: string | null | undefined): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "—";
  return date.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}
