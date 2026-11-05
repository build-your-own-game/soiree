// Mesure 6 : la partie qui fait foi en arrière-plan. Pour chaque variante de la page
// (`temoin` : setTimeout, requestAnimationFrame, un Worker ; `audio` : les mêmes et
// un AudioContext muet), un navigateur neuf ouvre la page au premier plan, puis un
// autre onglet passe devant pendant `--arriere` secondes, puis la page revient
// devant et renvoie ses réveils. Les deux variantes tournent en même temps, chacune
// dans son propre navigateur.
//
//   node cadence.mjs --navigateur chrome|firefox|safari [--variantes temoin,audio]
//                    [--avant 30] [--arriere 420] [--retour 15]
//
// 420 s en arrière-plan : Chrome documente un bridage plus fort des chaînes de
// minuteries après cinq minutes cachées (« intensive wake up throttling ») ; il faut
// dépasser ces cinq minutes pour le voir, ou voir qu'il n'a pas lieu.
//
// Le son est permis sans geste (Chrome : `--autoplay-policy=no-user-gesture-required` ;
// Firefox : `media.autoplay.default` à 0) : le soir de la partie, l'hôte aura
// cliqué pour lancer le jeu, et un AudioContext aura eu son geste. Les deux variantes
// ont les mêmes réglages.
//
// Résultats : `soiree/target/mesures-navigateur/cadence/<navigateur>-<variante>.json`.

import { join } from "node:path";
import os from "node:os";
import { DONNEES, demarrerServeur, ecrireJson } from "./serveur.mjs";
import { ouvrirChrome, ouvrirFirefox, ouvrirSafari, versions } from "./navigateurs.mjs";

function option(nom, defaut) {
  const k = process.argv.indexOf(`--${nom}`);
  return k > 0 ? process.argv[k + 1] : defaut;
}
const navigateur = option("navigateur", "chrome");
const variantes = option("variantes", "temoin,audio").split(",");
const AVANT = Number(option("avant", "30")) * 1000;
const ARRIERE = Number(option("arriere", "420")) * 1000;
const RETOUR = Number(option("retour", "15")) * 1000;
const attendre = (ms) => new Promise((f) => setTimeout(f, ms));

const recus = new Map();
const serveur = await demarrerServeur({
  surResultat: (sorte, params, json) => {
    if (sorte === "cadence") recus.get(params.variante)?.(json);
  },
});

/** Chrome : la page dans un onglet, puis un autre onglet activé devant elle, par le
 *  protocole DevTools. */
async function chrome(url) {
  const b = await ouvrirChrome("about:blank", { pilote: true, drapeaux: ["--autoplay-policy=no-user-gesture-required"] });
  const c = b.client;
  const { targetId: page } = await c.envoyer("Target.createTarget", { url });
  const { sessionId } = await c.envoyer("Target.attachToTarget", { targetId: page, flatten: true });
  const evaluer = async (expr) => {
    const r = await c.envoyer("Runtime.evaluate", { expression: expr, returnByValue: true, awaitPromise: true }, { sessionId });
    if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails));
    return r.result.value;
  };
  return {
    fermer: () => b.fermer(),
    evaluer,
    async cacher() {
      const { targetId } = await c.envoyer("Target.createTarget", { url: "about:blank" });
      await c.envoyer("Target.activateTarget", { targetId });
    },
    async montrer() {
      await c.envoyer("Target.activateTarget", { targetId: page });
    },
  };
}

/** Firefox : la page dans le premier onglet, puis un autre onglet activé devant
 *  elle, par WebDriver BiDi. */
async function firefox(url) {
  const b = await ouvrirFirefox(url, {
    pilote: true,
    preferences: { "media.autoplay.default": 0, "media.autoplay.block-webaudio": false },
  });
  const c = b.client;
  let page = null;
  for (let k = 0; k < 100 && !page; k++) {
    const arbre = await c.envoyer("browsingContext.getTree", {});
    page = arbre.contexts.find((x) => x.url.includes("cadence.html"))?.context ?? null;
    if (!page) await attendre(100);
  }
  if (!page) throw new Error("Firefox : la page de cadence ne s'est pas ouverte");
  const evaluer = async (expr) => {
    const r = await c.envoyer("script.evaluate", { expression: expr, target: { context: page }, awaitPromise: true, resultOwnership: "none" });
    if (r.type === "exception") throw new Error(JSON.stringify(r.exceptionDetails));
    return r.result.value;
  };
  return {
    fermer: () => b.fermer(),
    evaluer,
    async cacher() {
      const { context } = await c.envoyer("browsingContext.create", { type: "tab" });
      await c.envoyer("browsingContext.activate", { context });
    },
    async montrer() {
      await c.envoyer("browsingContext.activate", { context: page });
    },
  };
}

/** Safari : la page, puis une nouvelle fenêtre ou un nouvel onglet devant elle, par
 *  WebDriver (Safari n'a pas de mode sans interface). */
async function safari(url) {
  const b = await ouvrirSafari(url);
  if (b.refus) return { refus: b.refus };
  const fenetre = (await b.wd("GET", "/window")).value;
  const evaluer = async (expr) => (await b.wd("POST", "/execute/async", {
    script: `const fini = arguments[arguments.length - 1]; Promise.resolve(${expr}).then((v) => fini(typeof v === "string" ? v : JSON.stringify(v)));`,
    args: [],
  })).value;
  return {
    fermer: () => b.fermer(),
    evaluer,
    async cacher() {
      const n = (await b.wd("POST", "/window/new", { type: "tab" })).value;
      await b.wd("POST", "/window", { handle: n.handle });
    },
    async montrer() {
      await b.wd("POST", "/window", { handle: fenetre });
    },
  };
}

async function variante(v) {
  const url = `${serveur.adresse}/page/cadence.html?variante=${v}&navigateur=${navigateur}`;
  const ouvrir = { chrome, firefox, safari }[navigateur];
  if (!ouvrir) throw new Error(`navigateur inconnu : ${navigateur}`);
  const recu = new Promise((ok) => recus.set(v, ok));
  const b = await ouvrir(url);
  const fichier = join(DONNEES, "cadence", `${navigateur}-${v}.json`);
  if (b.refus) {
    console.log(`${navigateur}/${v} : non mesuré : ${b.refus}`);
    ecrireJson(fichier, { non_mesure: b.refus, navigateur, variante: v });
    return;
  }
  const phases = { charge_avant: os.loadavg() };
  await attendre(AVANT);
  phases.visibilite_avant = await b.evaluer("JSON.stringify(__visibilite())");
  phases.cache = Date.now();
  await b.cacher();
  await attendre(2000);
  phases.visibilite_cachee = await b.evaluer("JSON.stringify(__visibilite())");
  console.log(`${navigateur}/${v} : cachée : ${phases.visibilite_cachee}`);
  await attendre(ARRIERE - 2000);
  phases.montre = Date.now();
  await b.montrer();
  await attendre(RETOUR);
  phases.visibilite_retour = await b.evaluer("JSON.stringify(__visibilite())");
  const compte = await b.evaluer("__fin().then((r) => JSON.stringify(r))");
  const r = await Promise.race([recu, attendre(30000).then(() => ({ echec: "rien reçu" }))]);
  phases.fin = Date.now();
  phases.charge_apres = os.loadavg();
  await b.fermer();
  r.phases = phases;
  r.durees_ms = { avant: AVANT, arriere: ARRIERE, retour: RETOUR };
  ecrireJson(fichier, r);
  console.log(`${navigateur}/${v} : ${compte}`);
}

console.log(JSON.stringify(versions()));
await Promise.all(variantes.map((v) => variante(v).catch((e) => console.log(`${navigateur}/${v} : échec : ${e.stack ?? e}`))));
await serveur.fermer();
process.exit(0);
