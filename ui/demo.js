// Made-up readings for looking at the widget in a browser (`?demo=calm`,
// `busy` or `hot`; add `&gpu=none` for a machine without GPU readings,
// `&pin=1` for the pressed pin, `&cores=24` for another core count,
// `&live=1` to keep new readings coming once a second, `&mascot=usagi` for
// another mascot, which main.js reads, `&detail=cpu|gpu|mem|none|empty` for
// the per-program list).
// Not used inside the app.

const GB = 1024 ** 3;
const LEVELS = {
  calm: { cpu: 14, gpu: 8, mem: 9.2, disk: 312 },
  busy: { cpu: 63, gpu: 71, mem: 11.8, disk: 312 },
  hot: { cpu: 95, gpu: 88, mem: 14.9, disk: 462 },
};

// Small deterministic generator, so two screenshots of one state match.
function rng(seed) {
  let x = seed;
  return () => {
    x = (x * 1664525 + 1013904223) % 4294967296;
    return x / 4294967296;
  };
}

const clamp = (v) => Math.min(100, Math.max(0, v));

export function run(render, name, search = '') {
  const level = LEVELS[name] ?? LEVELS.calm;
  const params = new URLSearchParams(search);
  const cores = Number(params.get('cores')) || 10;
  const noGpu = params.get('gpu') === 'none';
  if (params.get('pin') === '1') document.getElementById('pin').setAttribute('aria-pressed', 'true');
  const rand = rng(7);
  let cpu = level.cpu;
  let gpu = level.gpu;
  const spread = { calm: 7, hot: 4 }[name] ?? 12;
  const step = () => {
    cpu = clamp(level.cpu + (cpu - level.cpu) * 0.6 + (rand() - 0.5) * spread * 2);
    gpu = clamp(level.gpu + (gpu - level.gpu) * 0.7 + (rand() - 0.5) * spread * 1.6);
    render({
      cpu_total: cpu,
      cpu_cores: Array.from({ length: cores }, () => clamp(cpu + (rand() - 0.5) * 60)),
      gpu: noGpu ? null : gpu,
      mem_used: (level.mem + (rand() - 0.5) * 0.4) * GB,
      mem_total: 16 * GB,
      disk_used: level.disk * GB,
      disk_total: 500 * GB,
    });
  };
  // A minute of history at once, then (with `&live=1`) one reading a second
  // as the app would send them.
  for (let i = 0; i < 60; i++) step();
  if (params.get('live') === '1') setInterval(step, 1000);
}

const row = (name, count, value, rss = false) => ({ name, count, value, rss });
const DETAILS = {
  cpu: {
    programs: [
      row('Xcode', 6, 231.4), row('Google Chrome', 14, 88.2), row('WindowServer', 1, 41.7),
      row('node', 3, 23.9), row('Slack', 5, 12.3), row('Finder', 1, 6.1),
      row('mds_stores', 1, 3.4), row('먼작귀모니터링 아주 긴 이름의 프로그램', 4, 2.2),
    ],
    others: { programs: 212, count: 418, value: 9.6 },
    unreadable: 0,
  },
  gpu: {
    programs: [row('WindowServer', 1, 21.3), row('Google Chrome', 2, 9.4), row('Xcode', 1, 0.6)],
    others: { programs: 31, count: 40, value: 0 },
    unreadable: 0,
  },
  mem: {
    programs: [
      row('Google Chrome', 14, 3.4 * GB), row('Xcode', 6, 2.1 * GB), row('Slack', 5, 912 * 1024 ** 2),
      row('WindowServer', 1, 640 * 1024 ** 2, true), row('node', 3, 410 * 1024 ** 2),
      row('Finder', 1, 188 * 1024 ** 2), row('Mail', 2, 150 * 1024 ** 2), row('Notes', 1, 96 * 1024 ** 2),
    ],
    others: { programs: 205, count: 402, value: 2.6 * GB },
    unreadable: 3,
  },
};

/** A made-up `detail` event: `none` cannot be read, `empty` has no users. */
export function detail(kind) {
  const off = { programs: [], others: { programs: 0, count: 0, value: 0 }, unreadable: 0 };
  if (kind === 'none') return { kind, available: false, ...off, total: 0, processes: 0 };
  const list = kind === 'empty' ? { ...off, others: { programs: 30, count: 44, value: 0 } } : DETAILS[kind];
  const total = list.programs.reduce((a, p) => a + p.value, list.others.value);
  const processes = list.programs.reduce((a, p) => a + p.count, list.others.count);
  return { kind, available: true, ...list, total, processes };
}
