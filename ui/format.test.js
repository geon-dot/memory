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
