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
  detailView,
} from './format.js';
import { mountMascot } from './mascot.js';

// Matches the sparkline SVG viewBox in index.html.
const SPARK_W = 100;
const SPARK_H = 24;
// A reading at or above this percentage is drawn in the warning colour.
const HOT_PERCENT = 90;
// The mascot's constant movement (breathing, bobbing, blinking). The one switch
// for it. How often readings arrive is the app's side: SAMPLE_INTERVAL in
// lib.rs.
const IDLE_MOTION = true;
// A mood that has just appeared stays at least this long, so readings near
// a limit do not swap the face and the line every second.
const MOOD_HOLD_MS = 5000;
// What the mascot says in each mood.
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
  panel: $('panel'),
  panelTag: $('panel-tag'),
  panelTotal: $('panel-total'),
  panelRows: $('panel-rows'),
  panelMsg: $('panel-msg'),
  panelFoot: $('panel-foot'),
  panelClose: $('panel-close'),
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

const tauri = window.__TAURI__;

// Which mascot: the app keeps the choice made in its tray menu (this web
// view has no storage that outlives it). Opened in a browser,
// `?mascot=<name>` picks one. No answer, or an unknown name, gives the
// default one.
async function chosenMascot() {
  if (!tauri) return new URLSearchParams(location.search).get('mascot');
  try {
    return await tauri.core.invoke('mascot');
  } catch {
    return null;
  }
}

const mascot = mountMascot($('mascot'), {
  idleMotion: IDLE_MOTION,
  name: await chosenMascot(),
  report: (line) => tauri?.core.invoke('diag', { line }),
});
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

// --- Per-program list ---------------------------------------------------
// Clicking the CPU, GPU or MEM card opens the list for it in a panel to the
// right; the app widens the window and starts reading that list
// (`set_detail`), then sends it as `detail` events. Clicking the same card
// again, or the panel's ✕, closes it.
const DETAIL_TAG = { cpu: 'CPU', gpu: 'GPU', mem: 'MEM' };
const DETAIL_MSG = {
  loading: '읽는 중…',
  empty: '지금 쓰는 프로그램이 없어요',
  unavailable: '이 기기에서는 프로그램별 사용량을 읽을 수 없어요',
};
const picks = [...document.querySelectorAll('.card.pick')];
// The list on screen: 'cpu', 'gpu', 'mem' or 'off'.
let detail = 'off';

function panelMessage(text) {
  el.panelMsg.hidden = !text;
  el.panelRows.hidden = Boolean(text);
  setText(el.panelMsg, text ?? '');
}

function newRow() {
  const row = document.createElement('li');
  row.className = 'row';
  row.innerHTML =
    '<div class="row-line"><span class="row-name"></span><span class="row-note"></span>' +
    '<span class="row-value"></span></div><div class="row-bar"><i></i></div>';
  return row;
}

function renderDetail(list) {
  // A list of another kind is a late one from before the last click.
  if (list.kind !== detail) return;
  const view = detailView(list);
  setText(el.panelTotal, view.total);
  setText(el.panelFoot, view.foot);
  if (view.state !== 'list') return panelMessage(DETAIL_MSG[view.state]);
  panelMessage(null);
  while (el.panelRows.childElementCount < view.rows.length) el.panelRows.appendChild(newRow());
  while (el.panelRows.childElementCount > view.rows.length) el.panelRows.lastChild.remove();
  view.rows.forEach((r, i) => {
    const row = el.panelRows.children[i];
    const [name, note, value] = row.firstChild.children;
    row.classList.toggle('rest', r.rest);
    setText(name, r.name);
    setText(note, r.note);
    setText(value, r.value);
    if (name.title !== r.name) name.title = r.name;
    row.lastChild.firstChild.style.width = `${Math.round(r.ratio * 100)}%`;
  });
}

async function setDetail(kind) {
  detail = kind;
  const open = kind !== 'off';
  for (const pick of picks) {
    pick.setAttribute('aria-expanded', String(pick.dataset.detail === kind));
  }
  if (open) {
    el.panel.dataset.kind = kind;
    setText(el.panelTag, DETAIL_TAG[kind] ?? 'GPU');
    setText(el.panelTotal, '');
    setText(el.panelFoot, '');
    el.panelRows.replaceChildren();
    panelMessage(DETAIL_MSG.loading);
  }
  // Closing hides the panel before the window shrinks, opening shows it
  // after the window has grown, so the widget itself is never squeezed.
  if (!open) el.panel.hidden = true;
  try {
    if (tauri) await tauri.core.invoke('set_detail', { kind });
  } catch {
    // The window could not be resized; the list still fills what there is.
  }
  if (open && detail === kind) el.panel.hidden = false;
}

for (const pick of picks) {
  const toggle = () => setDetail(detail === pick.dataset.detail ? 'off' : pick.dataset.detail);
  pick.addEventListener('click', toggle);
  pick.addEventListener('keydown', (e) => {
    if (e.key !== 'Enter' && e.key !== ' ') return;
    e.preventDefault();
    toggle();
  });
}
el.panelClose.addEventListener('click', () => setDetail('off'));

if (tauri) {
  tauri.event.listen('snapshot', (event) => render(event.payload));
  tauri.event.listen('detail', (event) => renderDetail(event.payload));
  // Chosen in the tray menu while this widget is open.
  tauri.event.listen('mascot', (event) => mascot.setMascot(event.payload));
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
  if (demo) {
    import('./demo.js').then((m) => {
      m.run(render, demo, location.search);
      // `&detail=cpu|gpu|mem` opens that list with made-up programs.
      const kind = new URLSearchParams(location.search).get('detail');
      if (kind) setDetail(kind).then(() => renderDetail(m.detail(kind)));
    });
  }
}
