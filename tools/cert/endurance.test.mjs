import test from 'node:test';
import assert from 'node:assert/strict';
import { recurringScenarios, serverEndurance } from './endurance.mjs';

test('skipped phone faults cannot turn an idle solo page into a chaos endurance', () => {
  assert.deepEqual(recurringScenarios(['cut', 'freeze', 'hostile'], { phones: false, socket: false }), []);
  assert.deepEqual(recurringScenarios(['cut', 'monkey'], { phones: false, socket: false }), ['monkey']);
});

test('one additional retained thread fails; wall-clock changes do not alter the slope', () => {
  const samples = [
    { t: 1000, elapsed_ms: 300000, rss: 10240, threads: 8 },
    { t: -99999999, elapsed_ms: 1800000, rss: 10240, threads: 8 },
  ];
  assert.deepEqual(serverEndurance(samples).failures, []);
  assert.equal(serverEndurance([samples[0], { ...samples[1], threads: 9 }]).failures.length, 1);
  assert.equal(serverEndurance([samples[0], { ...samples[1], rss: 50000 }]).failures.length, 1);
  assert.throws(() => serverEndurance([{ ...samples[0], threads: NaN }, samples[1]]));
  assert.throws(() => serverEndurance([samples[0]]));
});
