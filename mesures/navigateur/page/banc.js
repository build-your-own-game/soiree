// La page du banc : charge le module réécrit, sa trace et ses métadonnées depuis le
// serveur du banc (`../serveur.mjs`), lance `banc-coeur.js`, et renvoie le résultat
// au serveur. Paramètres : `?module=<nom>&navigateur=<nom>&passes=<n>`.

import { banc } from "./banc-coeur.js";

const p = new URLSearchParams(location.search);
const nom = p.get("module");
const navigateur = p.get("navigateur") ?? "inconnu";
const passes = Number(p.get("passes") ?? 2);
const sortie = document.getElementById("journal");
const journal = (l) => {
  sortie.textContent += `${l}\n`;
  // Le serveur garde aussi le journal : un navigateur sans interface ne le montre pas.
  fetch(`/journal?navigateur=${encodeURIComponent(navigateur)}`, { method: "POST", body: l }).catch(() => {});
};

async function octets(chemin) {
  const r = await fetch(chemin, { cache: "no-store" });
  if (!r.ok) throw new Error(`${chemin} : ${r.status}`);
  return r.arrayBuffer();
}

async function principal() {
  const base = `/donnees/${encodeURIComponent(nom)}`;
  const meta = JSON.parse(new TextDecoder().decode(await octets(`${base}/meta.json`)));
  const module = new Uint8Array(await octets(`${base}/reecrit.wasm`));
  const trace = await octets(`${base}/trace.bin`);
  const r = await banc({ octets: module, meta, trace, passes, journal });
  r.navigateur = navigateur;
  r.agent = navigator.userAgent;
  r.coeurs = navigator.hardwareConcurrency;
  r.isole = globalThis.crossOriginIsolated === true;
  r.date = new Date().toISOString();
  await fetch(`/resultat?navigateur=${encodeURIComponent(navigateur)}&module=${encodeURIComponent(nom)}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(r),
  });
  journal("fini");
}

principal().catch(async (e) => {
  journal(`échec : ${e && e.stack ? e.stack : e}`);
  await fetch(`/resultat?navigateur=${encodeURIComponent(navigateur)}&module=${encodeURIComponent(nom)}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ echec: String(e && e.stack ? e.stack : e), navigateur, module: nom, agent: navigator.userAgent }),
  }).catch(() => {});
});
