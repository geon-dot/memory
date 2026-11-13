import {
  percent,
  pushHistory,
  sparklinePoints,
  gpuLabel,
  usageLabel,
  recentAverage,
  moodOf,
  holdMood,
  coreLayout,
} from './format.js';
import { mountMascot } from './mascot.js';

// Matches the sparkline SVG viewBox in index.html.
const SPARK_W = 100;
const SPARK_H = 24;
// A reading at or above this percentage is drawn in the warning colour.
const HOT_PERCENT = 90;
// 칩이's constant movement (breathing, bobbing, blinking). The one switch
// for it. How often readings arrive is the app's side: SAMPLE_INTERVAL in
// lib.rs.
const IDLE_MOTION = true;
// A mood that has just appeared stays at least this long, so readings near
// a limit do not swap the face and the line every second.
const MOOD_HOLD_MS = 5000;
// What 칩이 says in each mood.
const MOOD_TEXT = { calm: '한가해요', busy: '바빠요!', hot: '앗, 뜨거워!' };

const $ = (id) => document.getElementById(id);
const el = {
  say: $('say'),
  moodText: $('mood-text'),
  cpuValue: $('cpu-value'),
  cpuSpark: $('cpu-spark'),
  cpuArea: $('cpu-area'),
  cpuCores: $('cpu-cores'),
  gpuValue: $('gpu-value'),
  gpuSpark: $('gpu-spark'),
  gpuArea: $('gpu-area'),
  gpuSparkBox: $('gpu-spark-box'),
  memPercent: $('mem-percent'),
  memValue: $('mem-value'),
  memFill: $('mem-fill'),
  diskPercent: $('disk-percent'),
  diskValue: $('disk-value'),
  diskFill: $('disk-fill'),
  pin: $('pin'),
  hide: $('hide'),
};
const card = (node) => node.closest('.card');

// Text that has not changed is left alone, so a quiet second repaints little.
function setText(node, text) {
  if (node.textContent !== text) node.textContent = text;
}

let cpuHistory = [];
let gpuHistory = [];

function renderSpark(lineEl, areaEl, history) {
  const points = sparklinePoints(history, SPARK_W, SPARK_H);
  lineEl.setAttribute('points', points);
  // The same line closed down to the baseline, for the fill under it.
  const right = history.length > 1 ? SPARK_W : 0;
  areaEl.setAttribute('points', points && `${points} ${right},${SPARK_H} 0,${SPARK_H}`);
}

function renderCores(cores) {
  // Rebuild only when the core count changes; otherwise just resize the bars.
  if (el.cpuCores.childElementCount !== cores.length) {
    const { columns, rows, gapX, gapY } = coreLayout(cores.length);
    el.cpuCores.style.setProperty('--columns', columns);
    el.cpuCores.style.gap = `${gapY}px ${gapX}px`;
    el.cpuCores.classList.toggle('rows', rows > 1);
    el.cpuCores.replaceChildren(
      ...cores.map(() => {
        const bar = document.createElement('div');
        bar.className = 'core';
        bar.appendChild(document.createElement('i'));
        return bar;
      }),
    );
  }
  cores.forEach((v, i) => {
    el.cpuCores.children[i].firstChild.style.height = `${Math.round(v)}%`;
  });
}

function renderMeter(percentEl, valueEl, fillEl, used, total) {
  const pct = percent(used, total);
  setText(percentEl, total > 0 ? `${pct}%` : 'N/A');
  setText(valueEl, usageLabel(used, total));
  fillEl.style.width = `${pct}%`;
  card(fillEl).classList.toggle('hot', pct >= HOT_PERCENT);
  return pct;
}

const mascot = mountMascot($('mascot'), { idleMotion: IDLE_MOTION });
// The mood on screen and when it appeared; it starts as the page is drawn.
let shown = { mood: 'calm', since: -Infinity };

function renderMood(wanted) {
  const next = holdMood(shown, wanted, performance.now(), MOOD_HOLD_MS);
  if (next === shown) return;
  shown = next;
  mascot.setMood(shown.mood);
  el.moodText.textContent = MOOD_TEXT[shown.mood];
  // Replay the bubble's little pop for the new line, when there is someone
  // to see it.
  el.say.classList.remove('pop');
  if (document.hidden) return;
  void el.say.offsetWidth;
  el.say.classList.add('pop');
}

function render(s) {
  cpuHistory = pushHistory(cpuHistory, s.cpu_total);
  setText(el.cpuValue, `${Math.round(s.cpu_total)}%`);
  card(el.cpuValue).classList.toggle('hot', s.cpu_total >= HOT_PERCENT);
  renderSpark(el.cpuSpark, el.cpuArea, cpuHistory);
  renderCores(s.cpu_cores);

  const hasGpu = s.gpu != null;
  setText(el.gpuValue, gpuLabel(s.gpu));
  card(el.gpuValue).classList.toggle('hot', hasGpu && s.gpu >= HOT_PERCENT);
  el.gpuSparkBox.classList.toggle('off', !hasGpu);
  if (hasGpu) {
    gpuHistory = pushHistory(gpuHistory, s.gpu);
    renderSpark(el.gpuSpark, el.gpuArea, gpuHistory);
  }

  const memPercent = renderMeter(el.memPercent, el.memValue, el.memFill, s.mem_used, s.mem_total);
  renderMeter(el.diskPercent, el.diskValue, el.diskFill, s.disk_used, s.disk_total);

  renderMood(moodOf({ cpu: recentAverage(cpuHistory), gpu: s.gpu, memPercent }));
}

const tauri = window.__TAURI__;

async function setPinned(pinned) {
  el.pin.setAttribute('aria-pressed', String(pinned));
  if (tauri) await tauri.core.invoke('set_pinned', { pinned });
}

el.pin.addEventListener('click', () => {
  setPinned(el.pin.getAttribute('aria-pressed') !== 'true');
});

// Closing releases the window and its web view; the tray brings back a new
// one, which asks the app below whether it is pinned.
el.hide.addEventListener('click', () => {
  if (tauri) tauri.window.getCurrentWindow().close();
});

if (tauri) {
  tauri.event.listen('snapshot', (event) => render(event.payload));
  // The app keeps the pin (this web view has no storage that outlives it)
  // and has already applied it to the window; only the button needs it.
  tauri.core.invoke('pinned').then((pinned) => {
    el.pin.setAttribute('aria-pressed', String(pinned));
  });
} else {
  // Opened directly in a browser there is no backend. `?demo=calm|busy|hot`
  // fills the widget with made-up readings, for screenshots; without it the
  // rows keep their placeholder dashes.
  const demo = new URLSearchParams(location.search).get('demo');
  if (demo) import('./demo.js').then((m) => m.run(render, demo, location.search));
}
