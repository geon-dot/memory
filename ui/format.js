// Pure formatting and chart helpers for the widget. No DOM access here, so
// these run under `node --test` as well as in the webview.

const MB = 1024 ** 2;
const GB = 1024 ** 3;
const TB = 1024 ** 4;

/** Bytes as "512 MB" or "9.2 GB" (1024-based). */
export function formatBytes(n) {
  if (n >= GB) return `${(n / GB).toFixed(1)} GB`;
  return `${Math.round(n / MB)} MB`;
}

/** Whole-number percentage of `used` in `total`, 0..100. A non-positive total gives 0. */
export function percent(used, total) {
  if (!(total > 0)) return 0;
  return Math.min(100, Math.max(0, Math.round((used / total) * 100)));
}

/** Returns a new history with `v` appended, keeping only the newest `max` values. */
export function pushHistory(arr, v, max = 60) {
  return [...arr, v].slice(-max);
}

/**
 * SVG polyline points for percentage values (0..100) spread across `w`,
 * with 0 at the bottom (`y = h`) and 100 at the top (`y = 0`).
 */
export function sparklinePoints(arr, w, h) {
  const step = arr.length > 1 ? w / (arr.length - 1) : 0;
  const round = (n) => Number(n.toFixed(1));
  return arr
    .map((v, i) => {
      const clamped = Math.min(100, Math.max(0, v));
      return `${round(i * step)},${round(h - (clamped / 100) * h)}`;
    })
    .join(' ');
}

/** GPU utilization as "12%", or "N/A" when the OS reports none. */
export function gpuLabel(v) {
  return v == null ? 'N/A' : `${Math.round(v)}%`;
}

/** "9.2 / 16.0 GB" (TB from 1024 GB up), or "N/A" when the total is unknown. */
export function usageLabel(used, total) {
  if (!(total > 0)) return 'N/A';
  const [unit, name] = total >= TB ? [TB, 'TB'] : [GB, 'GB'];
  return `${(used / unit).toFixed(1)} / ${(total / unit).toFixed(1)} ${name}`;
}

/** Mean of the newest `n` values; 0 for an empty list. */
export function recentAverage(arr, n = 5) {
  const tail = arr.slice(-n);
  if (tail.length === 0) return 0;
  return tail.reduce((a, b) => a + b, 0) / tail.length;
}

/**
 * How the machine is doing, for the mascot: 'hot', 'busy' or 'calm'.
 * `cpu` is a smoothed CPU percentage, `gpu` a percentage or null,
 * `memPercent` the share of memory in use.
 */
export function moodOf({ cpu, gpu, memPercent }) {
  if (cpu >= 85 || memPercent >= 90) return 'hot';
  if (cpu >= 45 || (gpu ?? 0) >= 60) return 'busy';
  return 'calm';
}

/**
 * The mood to show, given the one showing (`{ mood, since }`, or nothing
 * yet), the mood the newest reading asks for, and the time now in
 * milliseconds. A mood that has just appeared stays for `holdMs`, so
 * readings wavering around a limit do not swap the mascot and its line every
 * second. Returns `shown` itself when nothing changes.
 */
export function holdMood(shown, wanted, now, holdMs = 5000) {
  if (!shown) return { mood: wanted, since: now };
  if (wanted === shown.mood || now - shown.since < holdMs) return shown;
  return { mood: wanted, since: now };
}

/**
 * How the per-core bars are laid out so that any core count fits the height
 * of one row of bars: one row up to 32 cores, then two half-height rows
 * (with 1px gaps from 65 cores), then more rows of 64. Gaps are in px.
 */
export function coreLayout(count) {
  if (count <= 32) return { columns: Math.max(1, count), rows: 1, gapX: 3, gapY: 0 };
  if (count <= 128) {
    return { columns: Math.ceil(count / 2), rows: 2, gapX: count <= 64 ? 3 : 1, gapY: 2 };
  }
  return { columns: 64, rows: Math.ceil(count / 64), gapX: 1, gapY: 1 };
}
