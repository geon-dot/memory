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

/** CPU of a program: percent of one core, and how many cores that is. */
export function cpuShare(v) {
  return `${Math.round(v)}% (${(v / 100).toFixed(1)}코어)`;
}

/** GPU of a program, as a share of the whole GPU; one decimal while small. */
export function gpuShare(v) {
  return v < 9.95 ? `${v.toFixed(1)}%` : `${Math.round(v)}%`;
}

const DETAIL_VALUE = { cpu: cpuShare, gpu: gpuShare, mem: formatBytes };
const DETAIL_NOTE = { cpu: '100% = 코어 1개', gpu: 'GPU 전체 = 100%' };

/**
 * What the detail panel shows for one `detail` event from the app (see
 * `DetailList` in procs.rs): `state` is 'unavailable' (this machine cannot
 * report the list), 'empty' (nothing is using it now) or 'list'. Each row
 * has its text and `ratio`, the bar length against the top program (0..1);
 * the last row, `rest: true`, is everything below the top programs.
 */
export function detailView(list) {
  const value = DETAIL_VALUE[list.kind] ?? String;
  if (!list.available) return { state: 'unavailable', total: '', rows: [], foot: '' };

  const top = list.programs[0]?.value ?? 0;
  const ratio = (v) => (top > 0 ? Number(Math.min(1, v / top).toFixed(2)) : 0);
  const rows = list.programs.map((p) => ({
    name: p.name,
    note: p.count > 1 ? `×${p.count}` : '',
    value: value(p.value) + (p.rss ? '*' : ''),
    ratio: ratio(p.value),
    rest: false,
  }));
  if (rows.length > 0 && list.others.programs > 0) {
    rows.push({
      name: `그 밖 ${list.others.programs}개`,
      note: '',
      value: value(list.others.value),
      ratio: ratio(list.others.value),
      rest: true,
    });
  }

  const foot = [`프로세스 ${list.processes}개`];
  if (list.unreadable > 0) foot.push(`읽지 못함 ${list.unreadable}개`);
  if (list.programs.some((p) => p.rss)) foot.push('* RSS 기준');
  if (DETAIL_NOTE[list.kind]) foot.push(DETAIL_NOTE[list.kind]);

  return {
    state: rows.length > 0 ? 'list' : 'empty',
    total: value(list.total),
    rows,
    foot: foot.join(' · '),
  };
}
