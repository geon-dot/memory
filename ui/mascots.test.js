import test from 'node:test';
import assert from 'node:assert/strict';
import { MASCOTS, DEFAULT_MASCOT, mascotOf, picturesOf } from './mascots.js';

test('the choices are the three picture mascots and 칩이', () => {
  assert.deepEqual(MASCOTS, ['chiikawa', 'hachiware', 'usagi', 'chip']);
});

test('with nothing chosen the mascot is 치이카와', () => {
  assert.equal(DEFAULT_MASCOT, 'chiikawa');
  assert.equal(mascotOf(undefined), 'chiikawa');
  assert.equal(mascotOf(null), 'chiikawa');
  assert.equal(mascotOf(''), 'chiikawa');
});

test('a known name is kept, anything else falls back to the default', () => {
  for (const name of MASCOTS) assert.equal(mascotOf(name), name);
  assert.equal(mascotOf(' usagi\n'), 'usagi');
  assert.equal(mascotOf('Usagi'), 'chiikawa');
  assert.equal(mascotOf('../secret'), 'chiikawa');
  assert.equal(mascotOf(3), 'chiikawa');
});

test('each mood has its own picture: calm is idle.png', () => {
  assert.deepEqual(picturesOf('chiikawa'), {
    calm: 'mascot/chiikawa/idle.png',
    busy: 'mascot/chiikawa/busy.png',
    hot: 'mascot/chiikawa/hot.png',
  });
  assert.equal(picturesOf('usagi').hot, 'mascot/usagi/hot.png');
  assert.equal(picturesOf('hachiware').calm, 'mascot/hachiware/idle.png');
});

test('칩이 is drawn, not a picture; unknown names have no pictures either', () => {
  assert.equal(picturesOf('chip'), null);
  assert.equal(picturesOf('nobody'), null);
});
