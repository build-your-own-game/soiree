// Le démarrage (PHASE-3, § 0 et § 2.8 ; lot 10a) : le seul JavaScript écrit à la main du
// commun. Le même fichier sert la page et le Worker de l'autorité : la page le charge par
// la page de démarrage du relais, puis le relance comme Worker (`new Worker(…, { type:
// "module" })`). Il ne fait que charger la colle `wasm-bindgen` du bon rôle et lui passer
// la main ; tout le reste est en Rust (`web/ecran`, `web/autorite`, `web/telephone`).
//
// Sans script en ligne, ni `eval`, ni `new Function` : la politique de contenu du relais
// (`default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'`) n'en
// permet pas.
import initEcran, { demarrer_la_page } from "./ecran.js";
import initAutorite, { demarrer_le_worker } from "./autorite.js";
import initTelephone, { demarrer_le_telephone } from "./telephone.js";

const dansUnWorker =
  typeof WorkerGlobalScope !== "undefined" && self instanceof WorkerGlobalScope;

if (dansUnWorker) {
  // Le Worker de l'autorité : la partie qui fait foi.
  await initAutorite();
  demarrer_le_worker();
} else if (location.pathname.startsWith("/s/")) {
  // La page d'un téléphone (lot 12) : elle ne charge que sa colle et son `.wasm`.
  await initTelephone();
  demarrer_le_telephone();
} else {
  // La page de la télé : elle lance le Worker sur ce même fichier.
  await initEcran();
  demarrer_la_page(import.meta.url);
}
