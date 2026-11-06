// Le Worker de la page `etalonner.html` (PHASE-3, lot 3c) : il charge la colle de
// `etalonnage-navigateur`, lit ce que la mesure demande, la joue et rend ce qu'elle a
// constaté. Rien du protocole d'appel n'est ici : il est dans `moteur-commun`.
import init, { boucle_vide, etalonner, profondeur_tenue } from "/paquet/etalonnage_navigateur.js";

async function lire(chemin) {
  const r = await fetch(chemin, { cache: "no-store" });
  if (!r.ok) throw new Error(`${chemin} : ${r.status}`);
  return new Uint8Array(await r.arrayBuffer());
}

self.onmessage = async (m) => {
  try {
    await init();
    const d = m.data;
    let r;
    if (d.quoi === "etalonner") {
      const base = `/donnees/${encodeURIComponent(d.module)}`;
      const [livre, trace] = await Promise.all([lire(`${base}/livre.wasm`), lire(`${base}/trace.bin`)]);
      r = etalonner(livre, trace, d.passes, d.fautif);
    } else if (d.quoi === "boucle") {
      r = boucle_vide(d.fois, d.budget);
    } else if (d.quoi === "profondeur") {
      r = profondeur_tenue();
    } else {
      r = { erreur: `mesure inconnue : ${d.quoi}` };
    }
    self.postMessage(r);
  } catch (e) {
    self.postMessage({ erreur: String(e && e.stack ? e.stack : e) });
  }
};
