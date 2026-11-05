// Le cœur du banc : le module réécrit appelé comme `moteur-wasmtime` l'appelle
// (`crates/moteur-wasmtime/src/interieur.rs`, `preparer` et `derouler`), la trace
// rejouée appel par appel et chronométrée, la compilation chronométrée. Il ne
// dépend que de WebAssembly et de `performance` : la page (`banc.js`) le lance dans
// un navigateur, `../hors-navigateur.mjs` dans node, deno ou bun.

import {
  APPEL, INSTANTANE, LACHER, NEUVE, OUBLIER, RESTAURER, fnv, lireTrace,
} from "./trace.js";

const PAGE = 65536;

/**
 * Une instance du module réécrit, appelée selon § 3.4 comme le fait le moteur :
 * avant chaque appel, le budget dans `soiree_carburant`, la profondeur et les
 * drapeaux de croissance à zéro, les quatre zones remises à zéro en entier (la
 * pile, la boîte, le brouillon, la zone de panique) ; puis `soiree_boite`, la copie
 * de l'appel, `soiree_appel`, et la copie du retour.
 */
export class InstanceJs {
  constructor(module, meta) {
    this.module = module;
    this.meta = meta;
    this.zones = meta.remises_a_zero;
    this.boiteCapacite = meta.boite[1];
    this.globalesDOrigine = WebAssembly.Module.exports(module)
      .map((e) => e.name)
      .filter((n) => n.startsWith("soiree_globale_"))
      .sort((a, b) => Number(a.slice(15)) - Number(b.slice(15)));
    this.creer();
  }

  /** Une instance neuve, `soiree_memoire_max` posé (comme `Interieur::new`). */
  creer() {
    const x = new WebAssembly.Instance(this.module, {}).exports;
    const g = this.meta.globales;
    this.memoire = x.memory;
    this.boite = x.soiree_boite;
    this.appel = x.soiree_appel;
    this.carburant = x[g.carburant];
    this.profondeur = x[g.profondeur];
    this.refusee = x[g.croissance_refusee];
    this.bornee = x[g.croissance_bornee];
    this.globales = this.globalesDOrigine.map((n) => x[n]);
    x[g.memoire_max].value = this.meta.memoire_max;
  }

  pages() {
    return this.memoire.buffer.byteLength / PAGE;
  }

  /**
   * Un appel. `remettre` à faux saute la remise à zéro des zones : seulement pour
   * mesurer ce qu'elle coûte, jamais dans le rejeu.
   * Rend `{ issue, carburant, sortie }` : issue 0 rendu, 1 faute (un piège), 2 panne.
   */
  appeler(sorte, octets, budget, remettre = true) {
    const n = octets.length;
    if (sorte === 0 && n > this.boiteCapacite) {
      return { issue: 2, carburant: 0n, sortie: null };
    }
    this.carburant.value = budget;
    this.profondeur.value = 0;
    this.refusee.value = 0;
    this.bornee.value = 0;
    if (remettre) {
      const m = new Uint8Array(this.memoire.buffer);
      for (const [a, l] of this.zones) m.fill(0, a, a + l);
    }
    let adresse;
    try {
      adresse = this.boite(n, sorte) >>> 0;
    } catch {
      return this.piege(budget);
    }
    if (adresse === 0) return { issue: 2, carburant: budget - this.carburant.value, sortie: null };
    new Uint8Array(this.memoire.buffer).set(octets, adresse);
    let r;
    try {
      r = this.appel(n);
    } catch {
      return this.piege(budget);
    }
    if (this.refusee.value !== 0 || this.profondeur.value !== 0) {
      return { issue: 2, carburant: budget - this.carburant.value, sortie: null };
    }
    r = BigInt.asUintN(64, r);
    const a = Number(r >> 32n);
    const l = Number(r & 0xffffffffn);
    const sortie = new Uint8Array(this.memoire.buffer).slice(a, a + l);
    return { issue: 0, carburant: budget - this.carburant.value, sortie };
  }

  piege(budget) {
    const issue = this.refusee.value !== 0 ? 2 : 1;
    return { issue, carburant: budget - this.carburant.value, sortie: null };
  }

  /** La mémoire entière et les globales mutables d'origine (§ 4.5, D23). `reserve`
   *  prête un tampon déjà alloué de la bonne taille : copier 3 à 25 Mio dans un
   *  tampon neuf à chaque point ferait travailler le ramasse-miettes du banc, dont
   *  les pauses tomberaient sur l'appel suivant. */
  instantane(reserve = null) {
    const source = new Uint8Array(this.memoire.buffer);
    const memoire = reserve ?? new Uint8Array(source.length);
    memoire.set(source);
    return {
      pages: this.pages(),
      memoire,
      globales: this.globales.map((g) => g.value),
    };
  }

  /** Comme `InstanceWasm::restaurer` : une instance neuve si la mémoire est plus grande. */
  restaurer(s) {
    if (this.pages() > s.pages) this.creer();
    if (this.pages() < s.pages) this.memoire.grow(s.pages - this.pages());
    new Uint8Array(this.memoire.buffer).set(s.memoire);
    this.globales.forEach((g, k) => { g.value = s.globales[k]; });
    this.carburant.value = 0n;
    this.profondeur.value = 0;
    this.refusee.value = 0;
    this.bornee.value = 0;
  }
}

/** La plus petite différence non nulle entre deux lectures de `performance.now()`. */
export function granularite() {
  let min = Infinity;
  for (let k = 0; k < 200; k++) {
    const a = performance.now();
    let b = a;
    while (b === a) b = performance.now();
    min = Math.min(min, b - a);
  }
  return min;
}

/** Les octets du module suivis d'une section personnalisée unique : le moteur ne
 *  peut pas reprendre une compilation faite sur d'autres octets. Une section
 *  personnalisée ne change rien au code. */
function avecNonce(octets, k) {
  const nom = new TextEncoder().encode("banc-nonce");
  const nonce = new TextEncoder().encode(`${k}-${Math.random()}-${Date.now()}`);
  const contenu = nom.length + 1 + nonce.length;
  if (contenu > 127) throw new Error("nonce trop long");
  const s = new Uint8Array(2 + contenu);
  s[0] = 0; // section personnalisée
  s[1] = contenu; // LEB128 sur un octet
  s[2] = nom.length;
  s.set(nom, 3);
  s.set(nonce, 3 + nom.length);
  const o = new Uint8Array(octets.length + s.length);
  o.set(octets, 0);
  o.set(s, octets.length);
  return o;
}

/** La médiane d'une suite (copiée, triée). */
function mediane(v) {
  const t = Float64Array.from(v).sort();
  return t.length ? t[Math.floor(t.length / 2)] : NaN;
}

/**
 * La compilation (mesure 4) : `WebAssembly.compile` sur les octets réécrits, puis
 * `compilations` fois sur des octets rendus uniques ; l'instanciation.
 */
export async function mesurerCompilation(octets, compilations = 5) {
  const t0 = performance.now();
  const module = await WebAssembly.compile(octets);
  const premiere = performance.now() - t0;
  const suivantes = [];
  for (let k = 0; k < compilations; k++) {
    const o = avecNonce(octets, k);
    const t = performance.now();
    await WebAssembly.compile(o);
    suivantes.push(performance.now() - t);
  }
  const instanciations = [];
  for (let k = 0; k < 5; k++) {
    const t = performance.now();
    new WebAssembly.Instance(module, {});
    instanciations.push(performance.now() - t);
  }
  return { module, compile_ms: { premiere, suivantes, mediane_suivantes: mediane(suivantes) }, instancier_ms: instanciations };
}

/**
 * La mise en route d'un module neuf : compiler, instancier, puis l'accueil
 * (`Bonjour`, `Decrire`), chacun chronométré, sur des octets rendus uniques. Un
 * moteur qui compile ses fonctions au premier appel (V8 le fait par défaut,
 * `--wasm-lazy-compilation`) rend un `WebAssembly.compile` court et paie le reste
 * ici, et dans le premier rejeu.
 */
export async function mesurerMiseEnRoute(octets, meta, bonjour, decrire, fois = 3) {
  const budget = 1n << 28n;
  const mesures = [];
  for (let k = 0; k < fois; k++) {
    const o = avecNonce(octets, 1000 + k);
    const t0 = performance.now();
    const m = await WebAssembly.compile(o);
    const t1 = performance.now();
    const i = new InstanceJs(m, meta);
    const t2 = performance.now();
    const b = i.appeler(0, bonjour, budget);
    const t3 = performance.now();
    const d = i.appeler(0, decrire, budget);
    const t4 = performance.now();
    if (b.issue !== 0 || d.issue !== 0) throw new Error(`accueil : issues ${b.issue}, ${d.issue}`);
    mesures.push({ compile_ms: t1 - t0, instancier_ms: t2 - t1, bonjour_ms: t3 - t2, decrire_ms: t4 - t3, total_ms: t4 - t0 });
  }
  return mesures;
}

/** La remise à zéro des zones : `fill(0)` d'un tampon de leur taille, déjà écrit,
 *  médiane de quinze (comme `etalonner`) ; puis les quatre zones dans la mémoire
 *  d'une instance, comme le fait `appeler`. */
export function mesurerRemiseAZero(module, meta) {
  const n = meta.octets_remis_a_zero;
  const tampon = new Uint8Array(n);
  const seul = [];
  for (let k = 0; k < 15; k++) {
    tampon.fill(k + 1);
    const t = performance.now();
    tampon.fill(0);
    seul.push(performance.now() - t);
  }
  const i = new InstanceJs(module, meta);
  const dansLaMemoire = [];
  for (let k = 0; k < 15; k++) {
    const m = new Uint8Array(i.memoire.buffer);
    for (const [a, l] of meta.remises_a_zero) m.fill(k + 1, a, a + l);
    const t = performance.now();
    for (const [a, l] of meta.remises_a_zero) m.fill(0, a, a + l);
    dansLaMemoire.push(performance.now() - t);
  }
  return { octets: n, seul_ms: mediane(seul), memoire_ms: mediane(dansLaMemoire) };
}

/** Deux mille `Bonjour` avec et sans remise à zéro, chronométrés d'un bloc (la
 *  résolution de l'horloge d'un navigateur ne permet pas de chronométrer un
 *  appel de quelques microsecondes). */
export function mesurerBonjours(module, meta, bonjour, fois = 2000) {
  const i = new InstanceJs(module, meta);
  const budget = 1n << 28n;
  const bloc = (remettre) => {
    const t = performance.now();
    for (let k = 0; k < fois; k++) i.appeler(0, bonjour, budget, remettre);
    return (performance.now() - t) / fois;
  };
  // Une mise en route, puis trois blocs de chaque, alternés.
  bloc(true);
  const avec = [], sans = [];
  for (let k = 0; k < 3; k++) { avec.push(bloc(true)); sans.push(bloc(false)); }
  return { fois, avec_ms: avec, sans_ms: sans, avec_mediane_ms: mediane(avec), sans_mediane_ms: mediane(sans) };
}

/** Rejoue la trace : chaque appel chronométré, son carburant et son retour
 *  comparés à ceux de wasmtime. */
export function rejouer(module, meta, evenements) {
  const instances = new Map();
  const instantanes = new Map();
  // Les tampons des instantanés oubliés, par taille, pour les suivants.
  const reserve = new Map();
  const prendre = (octets) => reserve.get(octets)?.pop() ?? null;
  const rendre = (t) => {
    if (!reserve.has(t.length)) reserve.set(t.length, []);
    reserve.get(t.length).push(t);
  };
  const durees = [];
  let ecarts = 0;
  let premierEcart = null;
  const pannes = [];
  const t0 = performance.now();
  evenements.forEach((e, k) => {
    switch (e.t) {
      case NEUVE:
        instances.set(e.inst, new InstanceJs(module, meta));
        break;
      case APPEL: {
        const i = instances.get(e.inst);
        if (!i) { pannes.push(`événement ${k} : instance absente`); durees.push(0); return; }
        const t = performance.now();
        const r = i.appeler(e.sorte, e.octets, e.budget);
        durees.push(performance.now() - t);
        const longueur = r.sortie ? r.sortie.length : 0;
        const empreinte = r.sortie ? fnv(r.sortie) : 0;
        const memeCarburant = e.issue === 2 || r.carburant === e.carburant;
        if (!memeCarburant || r.issue !== e.issue || longueur !== e.sortieLongueur || empreinte !== e.sortieFnv) {
          ecarts++;
          if (premierEcart === null) {
            premierEcart = `événement ${k} : carburant ${r.carburant} au lieu de ${e.carburant}, issue ${r.issue} au lieu de ${e.issue}, retour ${longueur} octets (${empreinte.toString(16)}) au lieu de ${e.sortieLongueur} (${e.sortieFnv.toString(16)})`;
          }
        }
        break;
      }
      case INSTANTANE: {
        const i = instances.get(e.inst);
        if (i) instantanes.set(e.id, i.instantane(prendre(i.memoire.buffer.byteLength)));
        break;
      }
      case RESTAURER: {
        const i = instances.get(e.inst);
        const s = instantanes.get(e.id);
        if (i && s) {
          try { i.restaurer(s); } catch (x) { pannes.push(`événement ${k} : restaurer : ${x}`); }
        }
        break;
      }
      case OUBLIER: {
        const s = instantanes.get(e.id);
        if (s) rendre(s.memoire);
        instantanes.delete(e.id);
        break;
      }
      case LACHER:
        instances.delete(e.inst);
        break;
      default:
        break;
    }
  });
  return {
    total_ms: performance.now() - t0,
    // En microsecondes, au millième : la résolution de l'horloge est plus grossière.
    durees_us: durees.map((d) => Math.round(d * 1e6) / 1e3),
    ecarts,
    premier_ecart: premierEcart,
    pannes,
  };
}

/** Tout le banc d'un module : compilation, remise à zéro, Bonjour, puis `passes`
 *  rejeux de la trace. */
export async function banc({ octets, meta, trace, passes = 2, journal = () => {} }) {
  const evenements = lireTrace(trace);
  const g = granularite();
  journal(`${meta.nom} : granularité de l'horloge ${g} ms ; compilation…`);
  const c = await mesurerCompilation(octets);
  journal(`${meta.nom} : compile ${c.compile_ms.premiere.toFixed(1)} ms ; remise à zéro…`);
  const bonjour = evenements.find((e) => e.t === APPEL && e.nom === 0).octets;
  const decrire = evenements.find((e) => e.t === APPEL && e.nom === 1).octets;
  const miseEnRoute = await mesurerMiseEnRoute(octets, meta, bonjour, decrire);
  journal(`${meta.nom} : mise en route ${miseEnRoute.map((m) => m.total_ms.toFixed(1)).join(", ")} ms ; remise à zéro…`);
  const zero = mesurerRemiseAZero(c.module, meta);
  const bonjours = mesurerBonjours(c.module, meta, bonjour);
  const resultats = [];
  for (let p = 0; p < passes; p++) {
    journal(`${meta.nom} : rejeu ${p + 1}…`);
    const r = rejouer(c.module, meta, evenements);
    journal(`${meta.nom} : rejeu ${p + 1} : ${r.total_ms.toFixed(0)} ms, ${r.ecarts} écarts, ${r.pannes.length} pannes`);
    resultats.push(r);
  }
  return {
    module: meta.nom,
    octets_reecrits: octets.length,
    granularite_ms: g,
    compilation: { compile_ms: c.compile_ms, instancier_ms: c.instancier_ms, mise_en_route: miseEnRoute },
    remise_a_zero: zero,
    bonjours,
    passes: resultats,
  };
}
