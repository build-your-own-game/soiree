// Le même banc hors navigateur, pour comparaison seulement : node et deno portent
// V8, bun porte JavaScriptCore, le moteur de Safari. Ce ne sont pas des navigateurs :
// ni leur version de moteur, ni leurs réglages, ni leur horloge ne sont ceux de
// Chrome ou de Safari. Un chiffre d'ici ne remplace pas une mesure dans Safari.
//
//   node hors-navigateur.mjs node [modules…]
//   deno run --allow-read --allow-write --allow-env hors-navigateur.mjs deno [modules…]
//   bun hors-navigateur.mjs bun [modules…]

import { readFileSync, readdirSync, existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import os from "node:os";
import { banc } from "./page/banc-coeur.js";

const ICI = dirname(fileURLToPath(import.meta.url));
const DONNEES = resolve(ICI, "../../target/mesures-navigateur");
const [etiquette, ...demandes] = process.argv.slice(2);
if (!etiquette) {
  console.error("usage : hors-navigateur.mjs <étiquette> [modules…]");
  process.exit(64);
}
const modules = demandes.length
  ? demandes
  : readdirSync(DONNEES).filter((d) => existsSync(join(DONNEES, d, "trace.bin")));

const version = typeof Bun !== "undefined" ? `bun ${Bun.version}`
  : typeof Deno !== "undefined" ? `deno ${Deno.version.deno} (V8 ${Deno.version.v8})`
  : `node ${process.version} (V8 ${process.versions.v8})`;

for (const m of modules) {
  const d = join(DONNEES, m);
  const meta = JSON.parse(readFileSync(join(d, "meta.json"), "utf8"));
  const octets = new Uint8Array(readFileSync(join(d, "reecrit.wasm")));
  const t = readFileSync(join(d, "trace.bin"));
  const trace = t.buffer.slice(t.byteOffset, t.byteOffset + t.byteLength);
  const avant = { loadavg: os.loadavg(), date: new Date().toISOString() };
  const r = await banc({ octets, meta, trace, passes: 2, journal: (l) => console.log(`  [${etiquette}] ${l}`) });
  r.navigateur = etiquette;
  r.agent = version;
  r.isole = null;
  r.date = new Date().toISOString();
  r.charge = { avant, apres: { loadavg: os.loadavg(), date: new Date().toISOString() } };
  const f = join(DONNEES, "resultats", etiquette, `${m}-1.json`);
  mkdirSync(dirname(f), { recursive: true });
  writeFileSync(f, JSON.stringify(r));
}
