import test from 'node:test';
import assert from 'node:assert/strict';
import { capacityActivityFailures, ordinaryPhones } from './capacity.mjs';

test('a large global gesture total cannot hide the untouched or absent phone', () => {
  const a = { index: 1, label: 'phone 1', gestures: 100, joined: true, live: true };
  const b = { index: 2, label: 'phone 2', gestures: 1, joined: true, live: true };
  assert.deepEqual(capacityActivityFailures([a, b], 2), []);
  for (const changed of [{ ...b, gestures: 0 }, { ...b, live: false }, { ...b, joined: false }, { ...b, index: 1 }]) {
    assert.ok(capacityActivityFailures([a, changed], 2).length);
  }
  assert.ok(capacityActivityFailures([a], 2).length);
});

test('a smaller room seats its own players, and no room invents a limit', () => {
  assert.equal(ordinaryPhones(8, 4), 4);
  assert.equal(ordinaryPhones(8, 71), 8);
  assert.equal(ordinaryPhones(12, 8), 8);
  assert.equal(ordinaryPhones(8, undefined), 8);
  assert.equal(ordinaryPhones(8, 0), 8);
});
