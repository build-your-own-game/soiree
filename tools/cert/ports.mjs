import net from 'node:net';
import dgram from 'node:dgram';
import { FixtureFailure } from './performance.mjs';

/** Select the fixture before starting a game. TCP availability alone says
 * nothing about the UDP port the same host must bind. This is not a retry of
 * a failed product. There remains the usual release-to-bind OS race. */
export async function freeHostPort({ candidate = () => 0, attempts = 32, onBusy = () => {} } = {}) {
  for (let attempt = 0; attempt < attempts; attempt++) {
    const tcp = net.createServer();
    const udp = dgram.createSocket('udp4');
    let port;
    let failure;
    try {
      await new Promise((resolve, reject) => { tcp.once('error', reject); tcp.listen(candidate(), '0.0.0.0', resolve); });
      port = tcp.address().port;
      await new Promise((resolve, reject) => { udp.once('error', reject); udp.bind(port, '0.0.0.0', resolve); });
    } catch (error) { failure = error; }
    finally {
      if (tcp.listening) await new Promise(resolve => tcp.close(resolve));
      try { await new Promise(resolve => udp.close(resolve)); }
      catch (error) { if (error.code !== 'ERR_SOCKET_DGRAM_NOT_RUNNING') throw error; }
    }
    if (!failure) return port;
    if (failure.code !== 'EADDRINUSE') throw failure;
    onBusy({ port: port ?? null, code: failure.code });
  }
  throw new FixtureFailure('aucun port libre à la fois pour TCP et UDP avant lancement');
}
