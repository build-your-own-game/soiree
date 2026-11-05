// Mesures 1 et 4 dans les navigateurs : pour chaque navigateur et chaque module
// capturé, un navigateur neuf (profil neuf) ouvre la page du banc, qui rejoue la
// trace et renvoie ses durées. Un module par lancement : chaque mesure part d'un
// processus froid.
//
//   node mesurer.mjs [--navigateurs chrome,firefox,safari] [--modules devine,…] [--passes 2]
//
// Les résultats vont dans `soiree/target/mesures-navigateur/resultats/<navigateur>/<module>.json`,
// avec la charge de la machine avant et après chaque lancement (OUT-89, HIS-08 : une
// machine chargée fausse une mesure sans un mot).

import { readdirSync, existsSync } from "node:fs";
import { join } from "node:path";
import os from "node:os";
import { DONNEES, demarrerServeur, ecrireJson } from "./serveur.mjs";
import { ouvrirChrome, ouvrirFirefox, ouvrirSafari, versions } from "./navigateurs.mjs";

function option(nom, defaut) {
  const k = process.argv.indexOf(`--${nom}`);
  return k > 0 ? process.argv[k + 1] : defaut;
}

const navigateurs = option("navigateurs", "chrome,firefox,safari").split(",");
const capturés = readdirSync(DONNEES).filter((d) => existsSync(join(DONNEES, d, "trace.bin")));
const modules = option("modules", capturés.join(",")).split(",");
const passes = Number(option("passes", "2"));
// Un tour suivant (`--tour 2`) s'écrit à part, pour voir ce qui bouge d'un lancement
// à l'autre ; des drapeaux de Chrome hors défaut (`--drapeaux-chrome "--js-flags=…"`)
// s'écrivent sous une autre étiquette (`--etiquette chrome-…`).
const tour = option("tour", "1");
const drapeauxChrome = option("drapeaux-chrome", "").split(" ").filter(Boolean);
const etiquette = option("etiquette", null);
const DELAI = 9 * 60 * 1000;

function charge() {
  return { loadavg: os.loadavg(), libre_octets: os.freemem(), date: new Date().toISOString() };
}

const attentes = new Map();
const serveur = await demarrerServeur({
  surResultat: (sorte, params, json) => {
    const cle = `${params.navigateur}/${params.module}`;
    const a = attentes.get(cle);
    if (a) a(json);
  },
  surJournal: (params, ligne) => console.log(`  [${params.navigateur}] ${ligne}`),
});

const env = { versions: versions(), machine: { modele: os.cpus()[0]?.model, coeurs: os.cpus().length, memoire_octets: os.totalmem(), systeme: `${os.type()} ${os.release()}` }, node: process.version };
console.log(JSON.stringify(env, null, 2));
ecrireJson(join(DONNEES, "resultats", "environnement.json"), env);

for (const nav of navigateurs) {
  for (const m of modules) {
    const nomNav = etiquette ?? nav;
    const cle = `${nomNav}/${m}`;
    const url = `${serveur.adresse}/page/banc.html?module=${encodeURIComponent(m)}&navigateur=${nomNav}&passes=${passes}`;
    const avant = charge();
    console.log(`${cle} : charge ${avant.loadavg.map((x) => x.toFixed(2)).join(" ")}`);
    const recu = new Promise((ok) => attentes.set(cle, ok));
    let b;
    if (nav === "chrome") b = await ouvrirChrome(url, { drapeaux: drapeauxChrome });
    else if (nav === "firefox") b = await ouvrirFirefox(url);
    else if (nav === "safari") {
      b = await ouvrirSafari(url);
      if (b.refus) {
        console.log(`${cle} : non mesuré : ${b.refus}`);
        ecrireJson(join(DONNEES, "resultats", nomNav, `${m}-${tour}.json`), { non_mesure: b.refus, navigateur: nav, module: m, avant });
        continue;
      }
    } else throw new Error(`navigateur inconnu : ${nav}`);
    let minuterie;
    const r = await Promise.race([recu, new Promise((ok) => { minuterie = setTimeout(() => ok({ echec: `rien reçu en ${DELAI} ms` }), DELAI); })]);
    clearTimeout(minuterie);
    await b.fermer();
    attentes.delete(cle);
    r.charge = { avant, apres: charge() };
    r.drapeaux = nav === "chrome" ? drapeauxChrome : [];
    ecrireJson(join(DONNEES, "resultats", nomNav, `${m}-${tour}.json`), r);
    console.log(`${cle} : ${r.echec ? `échec : ${r.echec}` : "reçu"}`);
  }
}
await serveur.fermer();
process.exit(0);
