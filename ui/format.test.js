import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  formatBytes,
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

test('formatBytes', () => {
  assert.equal(formatBytes(0), '0 MB');
  assert.equal(formatBytes(512 * 1024 ** 2), '512 MB');
  assert.equal(formatBytes(9.2 * 1024 ** 3), '9.2 GB');
});

test('percent guards zero total', () => {
  assert.equal(percent(5, 0), 0);
  assert.equal(percent(58, 100), 58);
  assert.equal(percent(200, 100), 100);
});

test('pushHistory caps length and does not mutate', () => {
  const a = [1, 2, 3];
  const b = pushHistory(a, 4, 3);
  assert.deepEqual(b, [2, 3, 4]);
  assert.deepEqual(a, [1, 2, 3]);
});

test('sparklinePoints maps range', () => {
  assert.equal(sparklinePoints([0, 100], 10, 20), '0,20 10,0');
  assert.equal(sparklinePoints([], 10, 20), '');
  assert.equal(sparklinePoints([50], 10, 20), '0,10');
});

test('labels', () => {
  assert.equal(gpuLabel(null), 'N/A');
  assert.equal(gpuLabel(12.4), '12%');
  assert.equal(usageLabel(0, 0), 'N/A');
  assert.equal(usageLabel(9.2 * 1024 ** 3, 16 * 1024 ** 3), '9.2 / 16.0 GB');
  assert.equal(usageLabel(1.5 * 1024 ** 4, 2 * 1024 ** 4), '1.5 / 2.0 TB');
  assert.equal(usageLabel(5, -1), 'N/A');
});

test('recentAverage takes the newest values', () => {
  assert.equal(recentAverage([], 5), 0);
  assert.equal(recentAverage([40], 5), 40);
  assert.equal(recentAverage([0, 0, 10, 20, 30], 3), 20);
  assert.equal(recentAverage([10, 20, 30, 40, 50, 60]), 40);
});

test('moodOf: calm when nothing is loaded', () => {
  assert.equal(moodOf({ cpu: 12, gpu: 6, memPercent: 57 }), 'calm');
  assert.equal(moodOf({ cpu: 44.9, gpu: 59.9, memPercent: 89 }), 'calm');
  assert.equal(moodOf({ cpu: 0, gpu: null, memPercent: 0 }), 'calm');
});

test('moodOf: busy from CPU or GPU load', () => {
  assert.equal(moodOf({ cpu: 45, gpu: 0, memPercent: 50 }), 'busy');
  assert.equal(moodOf({ cpu: 10, gpu: 60, memPercent: 50 }), 'busy');
  assert.equal(moodOf({ cpu: 84.9, gpu: null, memPercent: 50 }), 'busy');
});

test('moodOf: hot from sustained CPU or nearly full memory', () => {
  assert.equal(moodOf({ cpu: 85, gpu: 0, memPercent: 50 }), 'hot');
  assert.equal(moodOf({ cpu: 5, gpu: null, memPercent: 90 }), 'hot');
  // A busy GPU alone does not make it hot.
  assert.equal(moodOf({ cpu: 5, gpu: 100, memPercent: 50 }), 'busy');
});

test('holdMood: the first mood is taken at once', () => {
  assert.deepEqual(holdMood(undefined, 'busy', 1000), { mood: 'busy', since: 1000 });
});

test('holdMood: a mood stays for at least the hold time', () => {
  const shown = { mood: 'busy', since: 1000 };
  assert.equal(holdMood(shown, 'calm', 2000, 5000), shown);
  assert.equal(holdMood(shown, 'hot', 5999, 5000), shown);
  assert.deepEqual(holdMood(shown, 'calm', 6000, 5000), { mood: 'calm', since: 6000 });
});

test('holdMood: the same mood again does not restart the hold', () => {
  const shown = { mood: 'busy', since: 1000 };
  assert.equal(holdMood(shown, 'busy', 9000, 5000), shown);
});

test('holdMood: readings that flip every second change the mood once per hold', () => {
  let shown;
  const seen = [];
  for (let s = 0; s < 12; s++) {
    const next = holdMood(shown, s % 2 ? 'busy' : 'calm', s * 1000, 5000);
    if (next !== shown) seen.push([s, next.mood]);
    shown = next;
  }
  assert.deepEqual(seen, [[0, 'calm'], [5, 'busy'], [10, 'calm']]);
});

test('coreLayout: up to 32 cores are one row of bars', () => {
  assert.deepEqual(coreLayout(10), { columns: 10, rows: 1, gapX: 3, gapY: 0 });
  assert.deepEqual(coreLayout(32), { columns: 32, rows: 1, gapX: 3, gapY: 0 });
});

test('coreLayout: more cores become two half-height rows, then narrower gaps', () => {
  assert.deepEqual(coreLayout(33), { columns: 17, rows: 2, gapX: 3, gapY: 2 });
  assert.deepEqual(coreLayout(48), { columns: 24, rows: 2, gapX: 3, gapY: 2 });
  assert.deepEqual(coreLayout(64), { columns: 32, rows: 2, gapX: 3, gapY: 2 });
  assert.deepEqual(coreLayout(96), { columns: 48, rows: 2, gapX: 1, gapY: 2 });
  assert.deepEqual(coreLayout(128), { columns: 64, rows: 2, gapX: 1, gapY: 2 });
});

test('coreLayout: beyond 128 cores rows are added, 64 bars to a row', () => {
  assert.deepEqual(coreLayout(192), { columns: 64, rows: 3, gapX: 1, gapY: 1 });
  assert.deepEqual(coreLayout(256), { columns: 64, rows: 4, gapX: 1, gapY: 1 });
});

test('coreLayout: no cores still gives a valid grid', () => {
  assert.deepEqual(coreLayout(0), { columns: 1, rows: 1, gapX: 3, gapY: 0 });
});
