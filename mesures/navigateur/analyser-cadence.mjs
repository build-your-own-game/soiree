// L'analyse de la mesure 6 : pour chaque navigateur et chaque variante, la cadence de
// chaque horloge au premier plan, puis minute par minute en arrière-plan, puis au
// retour. Les bornes des phases sont les `visibilitychange` notés par la page (la
// page cachée, puis de nouveau visible), à défaut les heures du pilote.
//
//   node analyser-cadence.mjs

import { readdirSync, readFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import { DONNEES } from "./serveur.mjs";

const dossier = join(DONNEES, "cadence");
const fichiers = existsSync(dossier) ? readdirSync(dossier).filter((f) => f.endsWith(".json")).sort() : [];
const HORLOGES = ["timeout", "raf", "worker", "audio"];

function q(trie, p) {
  if (!trie.length) return NaN;
  return trie[Math.min(trie.length, Math.max(1, Math.ceil(trie.length * p))) - 1];
}

/** Les réveils dans `[a, b)` : leur nombre par seconde, et leurs intervalles. */
function fenetre(reveils, a, b) {
  const dedans = reveils.filter((x) => x >= a && x < b);
  const iv = [];
  for (let k = 1; k < dedans.length; k++) iv.push(dedans[k] - dedans[k - 1]);
  const t = Float64Array.from(iv).sort();
  return { n: dedans.length, hz: dedans.length / ((b - a) / 1000), mediane: q(t, 0.5), p99: q(t, 0.99), max: t.length ? t[t.length - 1] : NaN };
}

const f = (x, d = 0) => (Number.isFinite(x) ? x.toFixed(d) : "—");
const out = [];
for (const nom of fichiers) {
  const r = JSON.parse(readFileSync(join(dossier, nom), "utf8"));
  if (r.non_mesure || r.echec) {
    out.push(`\n**${nom.replace(".json", "")}** : non mesuré — ${r.non_mesure ?? r.echec}`);
    continue;
  }
  const ev = r.evenements;
  const charge = ev[0].quand;
  const cachee = ev.find((e) => e.quoi === "visibilitychange" && e.visibilite === "hidden")?.quand ?? r.phases.cache;
  const visible = ev.find((e) => e.quoi === "visibilitychange" && e.visibilite === "visible" && e.quand > cachee)?.quand ?? r.phases.montre;
  const fin = ev.find((e) => e.quoi === "fin")?.quand ?? r.phases.fin;
  const fenetres = [["premier plan", charge + 2000, cachee]];
  // Une minute entière par ligne ; un reste de moins de 10 s ne dit rien.
  for (let m = 0; cachee + m * 60000 + 10000 < visible; m++) {
    fenetres.push([`caché, minute ${m + 1}`, cachee + m * 60000, Math.min(visible, cachee + (m + 1) * 60000)]);
  }
  fenetres.push(["caché, en tout", cachee, visible]);
  fenetres.push(["de retour", visible + 1000, fin]);
  const autres = ev.filter((e) => !["chargee", "visibilitychange", "fin", "focus", "blur"].includes(e.quoi)).map((e) => e.quoi);
  out.push(`\n**${nom.replace(".json", "")}** — ${r.agent}`);
  out.push(`\nCachée ${((visible - cachee) / 1000).toFixed(0)} s ; visibilité vue par le pilote : avant ${r.phases.visibilite_avant}, cachée ${r.phases.visibilite_cachee}, retour ${r.phases.visibilite_retour}${r.audio ? ` ; son : ${JSON.stringify(r.audio.etats?.map((e) => e.etat))}` : ""}${autres.length ? ` ; autres événements : ${autres.join(", ")}` : ""}.\n`);
  const horloges = HORLOGES.filter((h) => r.reveils[h]?.length);
  out.push(`| Phase | ${horloges.map((h) => `${h} : réveils ; par seconde ; intervalle médian / 99ᵉ / max (ms)`).join(" | ")} |`);
  out.push(`| --- | ${horloges.map(() => "---").join(" | ")} |`);
  for (const [nomF, a, b] of fenetres) {
    out.push(`| ${nomF} | ${horloges.map((h) => {
      const s = fenetre(r.reveils[h], a, b);
      return `${s.n} ; ${s.hz >= 1 ? f(s.hz, 1) : f(s.hz, 3)} ; ${f(s.mediane)} / ${f(s.p99)} / ${f(s.max)}`;
    }).join(" | ")} |`);
  }
}
console.log(out.join("\n"));
