// La page de la mesure 6 : un onglet de l'ordinateur de la soirée qui n'est plus au
// premier plan continue-t-il de tourner, et à quelle cadence ? Elle note chaque
// réveil de ses horloges, en temps absolu (`performance.timeOrigin + now()`, en ms
// depuis l'époque) :
//
// - `timeout` : une chaîne de setTimeout à 16 ms (la boucle d'une partie à 60 pas
//   par seconde, sur le fil de la page) ;
// - `raf` : requestAnimationFrame ;
// - `worker` : la même chaîne de setTimeout dans un Worker dédié ;
// - `audio` (variante « audio » seulement) : un AudioContext qui joue un oscillateur
//   à travers un gain nul, plus un ScriptProcessorNode dont chaque rappel est noté :
//   l'horloge du son, et la question de savoir si un son muet change le bridage des
//   autres horloges.
//
// Les changements de visibilité (`visibilitychange`), de cycle de vie (`freeze`,
// `resume`) et de focus sont notés aussi. Le pilote (`../cadence.mjs`) appelle
// `__fin()` à la fin : la page renvoie tout au serveur.

const p = new URLSearchParams(location.search);
const variante = p.get("variante") ?? "temoin";
const navigateur = p.get("navigateur") ?? "inconnu";
const t = () => performance.timeOrigin + performance.now();
const etat = document.getElementById("etat");

const reveils = { timeout: [], raf: [], audio: [] };
const evenements = [{ quand: t(), quoi: "chargee", visibilite: document.visibilityState, focus: document.hasFocus() }];
const noterEvenement = (quoi) => evenements.push({ quand: t(), quoi, visibilite: document.visibilityState, focus: document.hasFocus() });
document.addEventListener("visibilitychange", () => noterEvenement("visibilitychange"));
document.addEventListener("freeze", () => noterEvenement("freeze"));
document.addEventListener("resume", () => noterEvenement("resume"));
window.addEventListener("focus", () => noterEvenement("focus"));
window.addEventListener("blur", () => noterEvenement("blur"));
window.addEventListener("pagehide", () => noterEvenement("pagehide"));
window.addEventListener("pageshow", () => noterEvenement("pageshow"));

const minuterie = () => {
  reveils.timeout.push(t());
  setTimeout(minuterie, 16);
};
minuterie();

const image = () => {
  reveils.raf.push(t());
  requestAnimationFrame(image);
};
requestAnimationFrame(image);

const worker = new Worker("cadence-worker.js");

let audio = null;
if (variante === "audio") {
  try {
    const ctx = new AudioContext();
    const osc = ctx.createOscillator();
    const muet = ctx.createGain();
    muet.gain.value = 0;
    const sonde = ctx.createScriptProcessor(1024, 1, 1);
    sonde.onaudioprocess = () => { reveils.audio.push(t()); };
    osc.connect(muet);
    muet.connect(ctx.destination);
    osc.connect(sonde);
    sonde.connect(muet);
    osc.start();
    audio = { ctx, etats: [{ quand: t(), etat: ctx.state }] };
    ctx.onstatechange = () => audio.etats.push({ quand: t(), etat: ctx.state });
    ctx.resume().catch((e) => audio.etats.push({ quand: t(), etat: `resume refusé : ${e}` }));
  } catch (e) {
    audio = { erreur: String(e) };
  }
}

setInterval(() => {
  etat.textContent = `${variante} : ${reveils.timeout.length} timeout, ${reveils.raf.length} raf, ${reveils.audio.length} audio ; ${document.visibilityState}`;
}, 1000);

window.__visibilite = () => ({ visibilite: document.visibilityState, cache: document.hidden, focus: document.hasFocus(), audio: audio?.ctx?.state ?? null });

window.__fin = async () => {
  noterEvenement("fin");
  const deWorker = await new Promise((ok) => {
    worker.onmessage = (m) => ok(m.data);
    worker.postMessage("fin");
  });
  const r = {
    navigateur, variante, agent: navigator.userAgent,
    reveils: { ...reveils, worker: deWorker },
    evenements,
    audio: audio ? { etats: audio.etats ?? null, erreur: audio.erreur ?? null, frequence: audio.ctx?.sampleRate ?? null } : null,
  };
  await fetch(`/cadence?navigateur=${encodeURIComponent(navigateur)}&variante=${encodeURIComponent(variante)}`, {
    method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(r),
  });
  return { timeout: reveils.timeout.length, raf: reveils.raf.length, worker: deWorker.length, audio: reveils.audio.length };
};
