// Le Worker de la page `rejouer.html` (PHASE-3, lot 3b) : il charge la colle de
// `rejeu-navigateur`, lit le module livré, les soirées et le relevé de wasmtime, puis
// joue chaque passage demandé et rend ce que `rejouer` a constaté.
import init, { rejouer } from "/paquet/rejeu_navigateur.js";

async function lire(chemin) {
  const r = await fetch(chemin);
  if (!r.ok) throw new Error(`${chemin} : ${r.status}`);
  return new Uint8Array(await r.arrayBuffer());
}

self.onmessage = async (m) => {
  try {
    const debut = performance.now();
    await init();
    const [module, soirees, releve] = await Promise.all(
      ["/e20/module.wasm", "/e20/soirees.bin", "/e20/releve.bin"].map(lire));
    const preparation_ms = performance.now() - debut;
    const passages = [];
    for (const fautif of m.data.passes) {
      const t = performance.now();
      const r = rejouer(module, soirees, releve, fautif);
      if (r.erreur !== undefined) {
        passages.push({ fautif, erreur: r.erreur });
        continue;
      }
      r.total_ms = performance.now() - t;
      passages.push(r);
    }
    self.postMessage({ passages, preparation_ms });
  } catch (e) {
    self.postMessage({ erreur: String(e && e.stack ? e.stack : e) });
  }
};
