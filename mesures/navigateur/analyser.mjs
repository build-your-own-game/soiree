// L'analyse du banc : lit les traces, le rejeu dans wasmtime et les résultats des
// navigateurs, et écrit les tableaux de `conception/MESURES-PHASE-3.md` (en
// Markdown, sur la sortie) et leur source chiffrée (`analyse.json`).
//
//   node analyser.mjs [--tour 1] [--passe 2]
//
// Ce que chaque chiffre mesure est dit dans MESURES-PHASE-3.md ; ici, les calculs :
//
// - **Coût fixe d'un moteur** : la médiane de la durée des deux mille `Bonjour` de la
//   section « bonjours » de la trace (environ 3 100 unités chacun), chronométrés un
//   par un.
// - **Durée d'une unité** d'un appel d'au moins `seuil` unités : `(durée − coût fixe
//   du même moteur) / carburant`, en picosecondes.
// - **Facteur navigateur/wasmtime** d'un appel : sa durée dans le navigateur sur sa
//   durée dans wasmtime, appel par appel (la même trace, le même ordre) ; et le même
//   rapport sur la seule part du carburant (durée moins coût fixe, de chaque côté).

import { readFileSync, readdirSync, existsSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { gzipSync, brotliCompressSync, constants } from "node:zlib";
import { DONNEES } from "./serveur.mjs";
import { APPEL, NOMS, lireTrace } from "./page/trace.js";

function option(nom, defaut) {
  const k = process.argv.indexOf(`--${nom}`);
  return k > 0 ? process.argv[k + 1] : defaut;
}
const TOUR = option("tour", "1");
const PASSE = Number(option("passe", "2")) - 1;
const LOURD = 1e5;
const TRES_LOURD = 1e6;
const SECTION = { accueil: 0, bonjours: 1, pires_cas: 2, soiree: 3 };

const modules = ["devine", "ardoise", "punchline", "sidequest", "tier-list"]
  .filter((m) => existsSync(join(DONNEES, m, "trace.bin")));
const RES = join(DONNEES, "resultats");
const moteurs = existsSync(RES)
  ? readdirSync(RES).filter((d) => !d.includes(".")).sort((a, b) => ordre(a) - ordre(b) || a.localeCompare(b))
  : [];
function ordre(n) {
  return ["chrome", "firefox", "safari"].indexOf(n) >= 0 ? ["chrome", "firefox", "safari"].indexOf(n) : 10;
}

const lire = (f) => JSON.parse(readFileSync(f, "utf8"));

/** Le rang le plus proche, comme `centile` d'`etalonner`. */
function q(trie, p) {
  if (!trie.length) return NaN;
  const rang = Math.max(1, Math.ceil(trie.length * p));
  return trie[Math.min(trie.length, rang) - 1];
}
const trier = (v) => Float64Array.from(v).sort();
function stats(v) {
  const t = trier(v);
  return { n: t.length, mediane: q(t, 0.5), p90: q(t, 0.9), p99: q(t, 0.99), max: t.length ? t[t.length - 1] : NaN, min: t.length ? t[0] : NaN };
}

const f1 = (x) => (Number.isFinite(x) ? x.toFixed(1) : "—");
const f2 = (x) => (Number.isFinite(x) ? x.toFixed(2) : "—");
const f0 = (x) => (Number.isFinite(x) ? Math.round(x).toLocaleString("fr-FR").replace(/ /g, " ") : "—");
const ko = (o) => `${(o / 1000).toFixed(0)} ko`;

const analyse = { tour: TOUR, passe: PASSE + 1, modules: {}, moteurs };
// Tous les appels lourds des cinq modules ensemble, par moteur : la distribution
// d'ensemble du facteur et de la durée d'une unité.
const ensemble = {};
const verser = (n, cle, v) => {
  ensemble[n] ??= {};
  (ensemble[n][cle] ??= []).push(...v);
};
const tables = [];

for (const m of modules) {
  const d = join(DONNEES, m);
  const meta = lire(join(d, "meta.json"));
  const t = readFileSync(join(d, "trace.bin"));
  const appels = lireTrace(t.buffer.slice(t.byteOffset, t.byteOffset + t.byteLength)).filter((e) => e.t === APPEL);
  const carb = appels.map((e) => Number(e.carburant));
  const wt = lire(join(d, "wasmtime.json"));
  const livre = readFileSync(meta.module_livre.startsWith("/") ? meta.module_livre : join(DONNEES, "../..", meta.module_livre));
  const reecrit = readFileSync(join(d, "reecrit.wasm"));
  const tailles = {
    livre: livre.length,
    reecrit: reecrit.length,
    livre_gzip: gzipSync(livre, { level: 9 }).length,
    reecrit_gzip: gzipSync(reecrit, { level: 9 }).length,
    livre_brotli: brotliCompressSync(livre, { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }).length,
    reecrit_brotli: brotliCompressSync(reecrit, { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }).length,
  };
  const durees = { wasmtime: wt.passes[PASSE].durees_ns.map((x) => x / 1000) };
  const passesTotales = { wasmtime: wt.passes.map((p) => p.durees_ns.reduce((a, b) => a + b, 0) / 1e6) };
  const ecarts = { wasmtime: wt.passes.map((p) => p.ecarts) };
  const res = {};
  for (const n of moteurs) {
    const f = join(RES, n, `${m}-${TOUR}.json`);
    if (!existsSync(f)) continue;
    const r = lire(f);
    res[n] = r;
    if (r.passes) {
      durees[n] = r.passes[PASSE].durees_us;
      passesTotales[n] = r.passes.map((p) => p.durees_us.reduce((a, b) => a + b, 0) / 1000);
      ecarts[n] = r.passes.map((p) => p.ecarts);
    }
  }
  const indices = (pred) => appels.map((e, k) => (pred(e, k) ? k : -1)).filter((k) => k >= 0);
  const bonjours = indices((e) => e.section === SECTION.bonjours);
  const fixe = {};
  const fixeStats = {};
  for (const [n, v] of Object.entries(durees)) {
    fixeStats[n] = stats(bonjours.map((k) => v[k]));
    fixe[n] = fixeStats[n].mediane;
  }
  const parUnite = {};
  for (const [n, v] of Object.entries(durees)) {
    for (const seuil of [LOURD, TRES_LOURD]) {
      const ks = indices((e, k) => (e.section === SECTION.pires_cas || e.section === SECTION.soiree) && carb[k] >= seuil);
      verser(n, `ps_${seuil}`, ks.map((k) => ((v[k] - fixe[n]) * 1e6) / carb[k]));
    }
    parUnite[n] = {};
    for (const [quoi, sec] of [["pires_cas", [SECTION.pires_cas]], ["soiree", [SECTION.soiree]], ["ensemble", [SECTION.pires_cas, SECTION.soiree]]]) {
      for (const seuil of [LOURD, TRES_LOURD]) {
        const ks = indices((e, k) => sec.includes(e.section) && carb[k] >= seuil);
        parUnite[n][`${quoi}_${seuil}`] = stats(ks.map((k) => ((v[k] - fixe[n]) * 1e6) / carb[k]));
      }
    }
  }
  const facteurs = {};
  for (const n of Object.keys(durees)) {
    if (n === "wasmtime") continue;
    facteurs[n] = {};
    for (const seuil of [LOURD, TRES_LOURD]) {
      const ks = indices((e, k) => (e.section === SECTION.pires_cas || e.section === SECTION.soiree) && carb[k] >= seuil);
      facteurs[n][`appel_${seuil}`] = stats(ks.map((k) => durees[n][k] / durees.wasmtime[k]));
      // La part du carburant n'a de sens que si elle dépasse nettement le bruit du
      // coût fixe et la granularité des horloges : au moins 50 µs dans wasmtime.
      const mesurables = ks.filter((k) => durees.wasmtime[k] - fixe.wasmtime >= 50);
      facteurs[n][`carburant_${seuil}`] = stats(mesurables.map((k) => (durees[n][k] - fixe[n]) / (durees.wasmtime[k] - fixe.wasmtime)));
    }
    for (const seuil of [LOURD, TRES_LOURD]) {
      const ks = indices((e, k) => (e.section === SECTION.pires_cas || e.section === SECTION.soiree) && carb[k] >= seuil);
      verser(n, `facteur_${seuil}`, ks.map((k) => durees[n][k] / durees.wasmtime[k]));
    }
    const tous = indices((e) => e.section === SECTION.soiree);
    facteurs[n].soiree_tous = stats(tous.map((k) => durees[n][k] / durees.wasmtime[k]));
  }
  // L'appel le plus lourd de la trace, dans chaque moteur.
  let kmax = 0;
  carb.forEach((c, k) => { if (c > carb[kmax]) kmax = k; });
  const lourdes = indices((e, k) => carb[k] === carb[kmax] && e.nom === appels[kmax].nom);
  const plusLourd = { nom: NOMS[appels[kmax].nom], carburant: carb[kmax], repetitions: lourdes.length, durees_us: {} };
  for (const [n, v] of Object.entries(durees)) plusLourd.durees_us[n] = stats(lourdes.map((k) => v[k]));
  // Les appels par sorte, carburant au 99ᵉ centile et au maximum, dans la soirée.
  const sortes = {};
  for (const e of appels) if (e.section === SECTION.soiree) sortes[NOMS[e.nom]] = (sortes[NOMS[e.nom]] ?? 0) + 1;

  analyse.modules[m] = {
    meta: { octets_remis_a_zero: meta.octets_remis_a_zero, appels: appels.length, pires_cas: meta.pires_cas, reecrire_ms: meta.reecrire_ms, compiler_wasmtime_ms: meta.compiler_wasmtime_ms, charger_ms: meta.charger_ms, remise_a_zero_ns: meta.remise_a_zero_ns, empreinte_livree: meta.empreinte_livree },
    tailles, fixeStats, parUnite, facteurs, plusLourd, passesTotales, ecarts, sortes,
    lourds: { pires_cas: indices((e, k) => e.section === SECTION.pires_cas && carb[k] >= LOURD).length, soiree: indices((e, k) => e.section === SECTION.soiree && carb[k] >= LOURD).length, soiree_tres: indices((e, k) => e.section === SECTION.soiree && carb[k] >= TRES_LOURD).length, pires_tres: indices((e, k) => e.section === SECTION.pires_cas && carb[k] >= TRES_LOURD).length },
    navigateurs: Object.fromEntries(Object.entries(res).map(([n, r]) => [n, r.non_mesure ? { non_mesure: r.non_mesure } : r.echec ? { echec: r.echec } : {
      agent: r.agent, isole: r.isole, granularite_ms: r.granularite_ms, compilation: r.compilation, remise_a_zero: r.remise_a_zero, bonjours: r.bonjours, charge: r.charge, drapeaux: r.drapeaux,
    }])),
  };
}

analyse.ensemble = Object.fromEntries(Object.entries(ensemble).map(([n, x]) => [n, Object.fromEntries(Object.entries(x).map(([c, v]) => [c, stats(v)]))]));
writeFileSync(join(DONNEES, "analyse.json"), JSON.stringify(analyse, null, 1));

// Les tableaux. Les navigateurs d'abord ; Chrome sans compilation paresseuse, node,
// deno et bun à part : ce ne sont pas des navigateurs dans leurs réglages par défaut.
const A = analyse.modules;
const mesure = (n) => modules.some((m) => A[m].navigateurs[n] && !A[m].navigateurs[n].non_mesure && !A[m].navigateurs[n].echec);
const NAV = moteurs.filter((n) => ["chrome", "firefox", "safari"].includes(n) && mesure(n));
const AUTRES = moteurs.filter((n) => !NAV.includes(n) && mesure(n));
const TOUS = [...NAV, ...AUTRES];
const out = [];
const ligne = (cells) => out.push(`| ${cells.join(" | ")} |`);
const entete = (cells) => { ligne(cells); ligne(cells.map(() => "---")); };
const med = (v) => q(trier(v), 0.5);
const st = (s, f = f2) => (s && s.n ? `${s.n} ; ${f(s.mediane)} ; ${f(s.p99)} ; ${f(s.max)}` : "—");

out.push("### Tailles des modules (mesure 4)\n");
entete(["Module", "livré", "livré, gzip -9", "livré, brotli 11", "réécrit", "réécrit, gzip -9", "réécrit, brotli 11"]);
for (const m of modules) {
  const s = A[m].tailles;
  ligne([m, ko(s.livre), ko(s.livre_gzip), ko(s.livre_brotli), ko(s.reecrit), ko(s.reecrit_gzip), ko(s.reecrit_brotli)]);
}

out.push("\n### Compilation (mesure 4), en ms : `WebAssembly.compile` 1ʳᵉ ; médiane des 5 suivantes ; mise en route (compile, instancie, Bonjour, Decrire), médiane de 3\n");
const comp = TOUS.filter((n) => n !== "node" && n !== "deno");
entete(["Module", "réécriture, médiane de 5", "wasmtime : compilation Cranelift, médiane de 5", ...comp]);
for (const m of modules) {
  const x = A[m];
  ligne([m, f1(med(x.meta.reecrire_ms)), f1(med(x.meta.compiler_wasmtime_ms)), ...comp.map((n) => {
    const c = x.navigateurs[n]?.compilation;
    return c ? `${f1(c.compile_ms.premiere)} ; ${f1(c.compile_ms.mediane_suivantes)} ; ${f1(med(c.mise_en_route.map((r) => r.total_ms)))}` : "—";
  })]);
}

out.push("\n### Coût fixe d'un appel (mesure 1), en µs\n");
out.push("Bonjour chronométré un par un (médiane / 99ᵉ centile / maximum de 2 000) ; dans les navigateurs, la même chose d'un bloc (2 000 Bonjour, durée moyenne, médiane de trois blocs), avec puis sans la remise à zéro des zones.\n");
entete(["Module", "zones (Kio)", "wasmtime : Bonjour", ...NAV.flatMap((n) => [`${n} : Bonjour`, `${n} : bloc, avec / sans remise à zéro`])]);
for (const m of modules) {
  const x = A[m];
  const w = x.fixeStats.wasmtime;
  ligne([m, f0(x.meta.octets_remis_a_zero / 1024), `${f1(w.mediane)} / ${f1(w.p99)} / ${f1(w.max)}`, ...NAV.flatMap((n) => {
    const s = x.fixeStats[n];
    const b = x.navigateurs[n]?.bonjours;
    if (!s || !b) return ["—", "—"];
    return [`${f1(s.mediane)} / ${f1(s.p99)} / ${f1(s.max)}`, `${f2(b.avec_mediane_ms * 1000)} / ${f2(b.sans_mediane_ms * 1000)}`];
  })]);
}

out.push("\n### La remise à zéro d'un Kio de zones (mesure 1), en ns par Kio\n");
out.push("wasmtime : `fill(0)` d'un tampon de la taille des zones, médiane de quinze, sur le Kio. Navigateurs et hors navigateur : (bloc avec − bloc sans remise à zéro) sur le Kio, puis `fill(0)` seul sur le Kio (horloge du navigateur : 5 ou 20 µs de granularité, d'où le bloc).\n");
entete(["Module", "wasmtime", ...TOUS]);
for (const m of modules) {
  const x = A[m];
  const kio = x.meta.octets_remis_a_zero / 1024;
  ligne([m, f2(x.meta.remise_a_zero_ns / kio), ...TOUS.map((n) => {
    const b = x.navigateurs[n]?.bonjours;
    const z = x.navigateurs[n]?.remise_a_zero;
    return b ? `${f2(((b.avec_mediane_ms - b.sans_mediane_ms) * 1e6) / kio)} ; ${f2((z.seul_ms * 1e6) / kio)}` : "—";
  })]);
}

out.push("\n### Durée d'une unité de carburant, appels d'au moins 1 000 000 unités (mesure 1), en ps : n ; médiane ; 99ᵉ centile ; maximum\n");
entete(["Module", "Appels", "wasmtime", ...TOUS]);
for (const m of modules) {
  for (const quoi of ["pires_cas", "soiree"]) {
    const cells = ["wasmtime", ...TOUS].map((n) => st(A[m].parUnite[n]?.[`${quoi}_${TRES_LOURD}`], f0));
    if (cells.every((c) => c === "—")) continue;
    ligne([m, quoi === "pires_cas" ? "pires cas" : "soirée", ...cells]);
  }
}

out.push("\n### Facteur navigateur/wasmtime, appel par appel (mesure 1) : n ; médiane ; 99ᵉ centile ; maximum\n");
out.push("La durée de l'appel dans le moteur sur sa durée dans wasmtime, pour le même appel de la même trace. « ≥ 10⁶ » : les appels d'au moins un million d'unités ; « ≥ 10⁵ » : d'au moins cent mille, où le coût fixe pèse. Part du carburant : (durée − coût fixe du moteur) sur (durée − coût fixe de wasmtime), pour les appels dont cette part dépasse 50 µs dans wasmtime.\n");
entete(["Module", ...NAV.flatMap((n) => [`${n} ≥ 10⁶`, `${n}, part du carburant ≥ 10⁶`, `${n} ≥ 10⁵`])]);
for (const m of modules) {
  ligne([m, ...NAV.flatMap((n) => [st(A[m].facteurs[n]?.[`appel_${TRES_LOURD}`]), st(A[m].facteurs[n]?.[`carburant_${TRES_LOURD}`]), st(A[m].facteurs[n]?.[`appel_${LOURD}`])])]);
}

out.push("\n### Les cinq modules ensemble (mesure 1) : n ; médiane ; 99ᵉ centile ; maximum\n");
entete(["Moteur", "facteur ≥ 10⁶", "facteur ≥ 10⁵", "ps par unité ≥ 10⁶", "ps par unité ≥ 10⁵"]);
for (const n of ["wasmtime", ...TOUS]) {
  const e = analyse.ensemble[n] ?? {};
  ligne([n, n === "wasmtime" ? "1" : st(e[`facteur_${TRES_LOURD}`]), n === "wasmtime" ? "1" : st(e[`facteur_${LOURD}`]), st(e[`ps_${TRES_LOURD}`], f0), st(e[`ps_${LOURD}`], f0)]);
}

out.push("\n### Facteur sur tous les appels de la soirée, légers compris : n ; médiane ; 99ᵉ centile ; maximum\n");
entete(["Module", ...TOUS]);
for (const m of modules) ligne([m, ...TOUS.map((n) => st(A[m].facteurs[n]?.soiree_tous))]);

out.push("\n### L'appel le plus lourd de chaque trace, en µs : médiane ; maximum de ses répétitions\n");
entete(["Module", "Appel", "Carburant", "Répétitions", "wasmtime", ...TOUS]);
for (const m of modules) {
  const p = A[m].plusLourd;
  const cell = (n) => { const s = p.durees_us[n]; return s ? `${f0(s.mediane)} ; ${f0(s.max)}` : "—"; };
  ligne([m, p.nom, f0(p.carburant), String(p.repetitions), cell("wasmtime"), ...TOUS.map(cell)]);
}

out.push("\n### Durée totale de chaque rejeu de la trace, en ms (somme des appels chronométrés) : passe 1 / passe 2\n");
entete(["Module", "Appels", "wasmtime", ...TOUS]);
for (const m of modules) {
  const p = A[m].passesTotales;
  ligne([m, f0(A[m].meta.appels), ...["wasmtime", ...TOUS].map((n) => (p[n] ? p[n].map(f0).join(" / ") : "—"))]);
}

out.push("\n### Écarts de carburant ou de retour avec l'enregistrement, par passe\n");
entete(["Module", "wasmtime", ...TOUS]);
for (const m of modules) {
  const e = A[m].ecarts;
  ligne([m, ...["wasmtime", ...TOUS].map((n) => (e[n] ? e[n].join(" / ") : "—"))]);
}

out.push("\n### Les appels de chaque trace\n");
entete(["Module", "appels", "pires cas", "pires cas ≥ 10⁵ / ≥ 10⁶", "soirée ≥ 10⁵ / ≥ 10⁶", "appels de la soirée, par sorte"]);
for (const m of modules) {
  const l = A[m].lourds;
  ligne([m, f0(A[m].meta.appels), String(A[m].meta.pires_cas), `${l.pires_cas} / ${l.pires_tres}`, `${l.soiree} / ${l.soiree_tres}`, Object.entries(A[m].sortes).map(([k, v]) => `${k} ${v}`).join(", ")]);
}

out.push("\n### Horloges\n");
entete(["Moteur", "agent", "isolé (COOP/COEP)", "granularité de performance.now()"]);
for (const n of TOUS) {
  const x = A[modules[0]].navigateurs[n];
  ligne([n, x?.agent ?? "—", String(x?.isole), x ? `${(x.granularite_ms * 1000).toFixed(3)} µs` : "—"]);
}

// Par sorte d'appel lourd, tous les tours de chaque navigateur ensemble : quel appel
// fait la queue de la distribution.
out.push("\n### Facteur par sorte d'appel, appels d'au moins 1 000 000 unités, tous les tours ensemble : tours ; n ; médiane ; 99ᵉ centile ; maximum\n");
entete(["Appels", ...NAV]);
const parSorte = {};
const toursDe = {};
for (const m of modules) {
  const t = readFileSync(join(DONNEES, m, "trace.bin"));
  const ap = lireTrace(t.buffer.slice(t.byteOffset, t.byteOffset + t.byteLength)).filter((e) => e.t === APPEL);
  const w = lire(join(DONNEES, m, "wasmtime.json")).passes[PASSE].durees_ns.map((x) => x / 1000);
  for (const n of NAV) {
    for (let tour = 1; tour <= 20; tour++) {
      const f = join(RES, n, `${m}-${tour}.json`);
      if (!existsSync(f)) continue;
      const r = lire(f);
      if (!r.passes) continue;
      (toursDe[n] ??= new Set()).add(tour);
      const d = r.passes[PASSE].durees_us;
      ap.forEach((e, k) => {
        if (Number(e.carburant) >= TRES_LOURD && (e.section === SECTION.pires_cas || e.section === SECTION.soiree)) {
          const cle = `${e.section === SECTION.pires_cas ? "pires cas" : "soirée"} : ${NOMS[e.nom]}`;
          ((parSorte[cle] ??= {})[n] ??= []).push(d[k] / w[k]);
        }
      });
    }
  }
}
for (const [cle, x] of Object.entries(parSorte).sort()) {
  ligne([cle, ...NAV.map((n) => (x[n] ? `${toursDe[n].size} ; ${st(stats(x[n]))}` : "—"))]);
}

for (const n of moteurs.filter((x) => modules.some((m) => A[m].navigateurs[x]?.non_mesure))) {
  const m = modules.find((x) => A[x].navigateurs[n]?.non_mesure);
  out.push(`\n${n} : non mesuré — ${A[m].navigateurs[n].non_mesure}`);
}

console.log(out.join("\n"));
