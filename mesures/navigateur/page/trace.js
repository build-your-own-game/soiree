// La trace écrite par la capture (`../capture/src/main.rs`, `ecrire_trace`) : chaque
// appel fait au module, avec ses octets, son budget et ce qu'il a rendu dans
// wasmtime, et chaque instance neuve, instantané, restauration ou instance lâchée,
// dans l'ordre. Ce module se charge dans une page comme dans node, deno ou bun.

/** Le nom de chaque appel, par son indice. Le même tableau est dans la capture. */
export const NOMS = [
  "Bonjour", "Decrire", "Creer", "Valider", "Entree", "Regarder", "Etat",
  "Ressources", "Projeter", "Charger", "Canonique", "DepuisCanonique", "Garder",
  "Resumer", "Verifier", "Transition", "Bot", "PireCas", "Memoire", "Proprietes",
  "Hypothese",
];

/** Les sections de la trace. Le même tableau est dans la capture. */
export const SECTIONS = ["accueil", "bonjours", "pires_cas", "soiree"];

export const NEUVE = 1;
export const APPEL = 2;
export const INSTANTANE = 3;
export const RESTAURER = 4;
export const OUBLIER = 5;
export const LACHER = 6;

/**
 * Lit une trace. Rend la liste de ses événements ; un appel garde une vue sur ses
 * octets, sans copie.
 * @param {ArrayBuffer} tampon
 */
export function lireTrace(tampon) {
  const v = new DataView(tampon);
  const o = new Uint8Array(tampon);
  const magie = new TextDecoder().decode(o.subarray(0, 8));
  if (magie !== "SOIRTRC1") throw new Error(`trace illisible : « ${magie} »`);
  let p = 8;
  const u8 = () => v.getUint8(p++);
  const u32 = () => { const x = v.getUint32(p, true); p += 4; return x; };
  const i32 = () => { const x = v.getInt32(p, true); p += 4; return x; };
  const u64 = () => { const x = v.getBigUint64(p, true); p += 8; return x; };
  const evenements = [];
  for (;;) {
    const sorte = u8();
    switch (sorte) {
      case 0:
        return evenements;
      case NEUVE:
        evenements.push({ t: NEUVE, inst: u32() });
        break;
      case APPEL: {
        const inst = u32();
        const section = u8();
        const nom = u8();
        const code = i32();
        const budget = u64();
        const n = u32();
        const octets = o.subarray(p, p + n);
        p += n;
        const carburant = u64();
        const issue = u8();
        const sortieLongueur = u32();
        const sortieFnv = u32();
        evenements.push({
          t: APPEL, inst, section, nom, sorte: code, budget, octets, carburant,
          issue, sortieLongueur, sortieFnv,
        });
        break;
      }
      case INSTANTANE:
        evenements.push({ t: INSTANTANE, inst: u32(), id: u32() });
        break;
      case RESTAURER:
        evenements.push({ t: RESTAURER, inst: u32(), id: u32() });
        break;
      case OUBLIER:
        evenements.push({ t: OUBLIER, id: u32() });
        break;
      case LACHER:
        evenements.push({ t: LACHER, inst: u32() });
        break;
      default:
        throw new Error(`trace : événement inconnu ${sorte} à l'octet ${p - 1}`);
    }
  }
}

/** FNV-1a sur 32 bits, comme la capture : l'empreinte courte d'un retour. */
export function fnv(octets) {
  let h = 0x811c9dc5;
  for (let k = 0; k < octets.length; k++) {
    h ^= octets[k];
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h >>> 0;
}
