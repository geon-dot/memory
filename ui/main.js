import {
  percent,
  pushHistory,
  sparklinePoints,
  gpuLabel,
  usageLabel,
} from './format.js';

// Matches the sparkline SVG viewBox in index.html.
const SPARK_W = 100;
const SPARK_H = 20;
// A meter at or above this percentage is drawn in the warning colour.
const HOT_PERCENT = 90;
const PIN_KEY = 'pinned';

const $ = (id) => document.getElementById(id);
const el = {
  cpuValue: $('cpu-value'),
  cpuSpark: $('cpu-spark'),
  cpuCores: $('cpu-cores'),
  gpuValue: $('gpu-value'),
  gpuSpark: $('gpu-spark'),
  gpuSparkBox: $('gpu-spark-box'),
  memValue: $('mem-value'),
  memFill: $('mem-fill'),
  diskValue: $('disk-value'),
  diskFill: $('disk-fill'),
  pin: $('pin'),
  hide: $('hide'),
};

let cpuHistory = [];
let gpuHistory = [];

function renderCores(cores) {
  // Rebuild only when the core count changes; otherwise just resize the bars.
  if (el.cpuCores.childElementCount !== cores.length) {
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

function renderMeter(valueEl, fillEl, used, total) {
  const pct = percent(used, total);
  valueEl.textContent = usageLabel(used, total);
  fillEl.style.width = `${pct}%`;
  fillEl.classList.toggle('hot', pct >= HOT_PERCENT);
}

function render(s) {
  cpuHistory = pushHistory(cpuHistory, s.cpu_total);
  el.cpuValue.textContent = `${Math.round(s.cpu_total)}%`;
  el.cpuSpark.setAttribute('points', sparklinePoints(cpuHistory, SPARK_W, SPARK_H));
  renderCores(s.cpu_cores);

  const hasGpu = s.gpu != null;
  el.gpuValue.textContent = gpuLabel(s.gpu);
  el.gpuSparkBox.classList.toggle('off', !hasGpu);
  if (hasGpu) {
    gpuHistory = pushHistory(gpuHistory, s.gpu);
    el.gpuSpark.setAttribute('points', sparklinePoints(gpuHistory, SPARK_W, SPARK_H));
  }

  renderMeter(el.memValue, el.memFill, s.mem_used, s.mem_total);
  renderMeter(el.diskValue, el.diskFill, s.disk_used, s.disk_total);
}

// localStorage can be unavailable; the pin preference is a convenience only.
function loadPinned() {
  try {
    return localStorage.getItem(PIN_KEY) === '1';
  } catch {
    return false;
  }
}

function savePinned(pinned) {
  try {
    localStorage.setItem(PIN_KEY, pinned ? '1' : '0');
  } catch {
    // Nothing to do: the window still pins for this session.
  }
}

const tauri = window.__TAURI__;

async function setPinned(pinned) {
  el.pin.setAttribute('aria-pressed', String(pinned));
  savePinned(pinned);
  if (tauri) await tauri.core.invoke('set_pinned', { pinned });
}

el.pin.addEventListener('click', () => {
  setPinned(el.pin.getAttribute('aria-pressed') !== 'true');
});

el.hide.addEventListener('click', () => {
  if (tauri) tauri.window.getCurrentWindow().hide();
});

// Opened directly in a browser there is no backend; the rows keep their
// placeholder dashes.
if (tauri) {
  tauri.event.listen('snapshot', (event) => render(event.payload));
  if (loadPinned()) setPinned(true);
}
