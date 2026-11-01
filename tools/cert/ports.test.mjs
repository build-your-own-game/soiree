import test from 'node:test';
import assert from 'node:assert/strict';
import dgram from 'node:dgram';
import { once } from 'node:events';
import { freeHostPort } from './ports.mjs';

test('a free TCP port occupied by UDP is rejected before starting the game', async () => {
  const occupied = dgram.createSocket('udp4');
  occupied.bind(0, '0.0.0.0'); await once(occupied, 'listening');
  try {
    const busy = occupied.address().port;
    let first = true;
    const rejected = [];
    const port = await freeHostPort({ candidate: () => { if (first) { first = false; return busy; } return 0; }, onBusy: value => rejected.push(value) });
    assert.notEqual(port, busy);
    assert.deepEqual(rejected, [{ port: busy, code: 'EADDRINUSE' }]);
    await assert.rejects(freeHostPort({ candidate: () => busy, attempts: 1 }), /aucun port libre/);
  } finally { occupied.close(); }
});
