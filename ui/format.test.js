import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  formatBytes,
  percent,
  pushHistory,
  sparklinePoints,
  gpuLabel,
  usageLabel,
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
