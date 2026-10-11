/**
 * Small display helpers shared by the views and charts.
 */

/** Formats a metric with up to four significant digits; `null` as an em dash. */
export function formatMetric(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return "—";
  if (value === 0) return "0";
  const abs = Math.abs(value);
  if (abs >= 1e5 || abs < 1e-3) return value.toExponential(2);
  return Number(value.toPrecision(4)).toString();
}

/** Formats a signed improvement estimate, e.g. `+0.31`. */
export function formatSigned(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return "—";
  const text = formatMetric(value);
  return value > 0 ? `+${text}` : text;
}

/** Formats milliseconds as a compact duration: `850 ms`, `12.3 s`, `4m 05s`. */
export function formatDuration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)} ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)} s`;
  const minutes = Math.floor(ms / 60_000);
  const seconds = Math.floor((ms % 60_000) / 1000);
  if (minutes < 60) return `${minutes}m ${String(seconds).padStart(2, "0")}s`;
  return `${Math.floor(minutes / 60)}h ${String(minutes % 60).padStart(2, "0")}m`;
}

/** Shortens a content id for display; the full id stays in titles. */
export function shortId(id: string, length = 8): string {
  return id.length > length ? id.slice(0, length) : id;
}

/** "Nice" axis ticks covering [min, max], about `count` of them. */
export function niceTicks(min: number, max: number, count = 5): number[] {
  if (!Number.isFinite(min) || !Number.isFinite(max)) return [];
  if (min === max) {
    const pad = min === 0 ? 1 : Math.abs(min) * 0.1;
    min -= pad;
    max += pad;
  }
  if (min > max) [min, max] = [max, min];
  const raw = (max - min) / Math.max(1, count);
  const magnitude = 10 ** Math.floor(Math.log10(raw));
  const residual = raw / magnitude;
  const step = (residual > 5 ? 10 : residual > 2 ? 5 : residual > 1 ? 2 : 1) * magnitude;
  const start = Math.floor(min / step) * step;
  const end = Math.ceil(max / step) * step;
  const ticks: number[] = [];
  for (let i = 0; start + i * step <= end + step * 1e-9 && i <= 50; i++) {
    ticks.push(Number((start + i * step).toPrecision(12)));
  }
  return ticks;
}
