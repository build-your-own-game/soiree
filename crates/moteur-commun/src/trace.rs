//! La trace d'étalonnage (M1, PHASE-3, § 8.6) : chaque appel fait au module, avec ses
//! octets, son budget et ce qu'il a rendu dans wasmtime, et chaque instance neuve,
//! instantané, restauration ou instance lâchée, dans l'ordre.
//!
//! La capture du harnais (`mesures/navigateur/capture`) l'écrit dans wasmtime ;
//! `soiree-banc etalonner` la rejoue dans chaque navigateur, appel par appel, sur le
//! protocole du commun (l'exemple `etalonnage-navigateur` de `web/moteur-navigateur`).
//! Une seule définition du format, ici, pour l'une et l'autre : la page du harnais la
//! relisait en JavaScript (`page/trace.js`), que le lot 3c a retirée avec
//! `page/banc-coeur.js`.
//!
//! Le format, petit-boutiste :
//!
//! ```text
//! "SOIRTRC1"
//! puis des événements, chacun introduit par un octet :
//!   1 Neuve       u32 instance
//!   2 Appel       u32 instance, u8 section, u8 nom, i32 sorte (code de soiree_boite),
//!                 u64 budget, u32 longueur, octets de l'appel,
//!                 u64 carburant brûlé, u8 issue (0 rendu, 1 faute, 2 panne),
//!                 u32 longueur du retour, u32 FNV-1a du retour
//!   3 Instantane  u32 instance, u32 numéro
//!   4 Restaurer   u32 instance, u32 numéro
//!   5 Oublier     u32 numéro
//!   6 Lacher      u32 instance
//!   0 Fin
//! ```

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use jeu_abi::{Appel, SorteAppel};
use partie::Echec;

/// L'en-tête d'une trace.
pub const MAGIE: &[u8; 8] = b"SOIRTRC1";

/// Le nom de chaque appel, par son indice dans la trace ([`indice_du_nom`]).
pub const NOMS: [&str; 22] = [
    "Bonjour",
    "Decrire",
    "Creer",
    "Valider",
    "Entree",
    "Regarder",
    "Etat",
    "Ressources",
    "Projeter",
    "Charger",
    "Canonique",
    "DepuisCanonique",
    "Garder",
    "Resumer",
    "Verifier",
    "Transition",
    "Bot",
    "PireCas",
    "Memoire",
    "Proprietes",
    "Hypothese",
    "Page",
];

/// Les sections de la trace, par leur indice.
pub const SECTIONS: [&str; 4] = ["accueil", "bonjours", "pires_cas", "soiree"];
/// L'accueil d'une instance nue : `Bonjour`, `Decrire`.
pub const ACCUEIL: u8 = 0;
/// Les deux mille `Bonjour` du coût fixe.
pub const BONJOURS: u8 = 1;
/// Les pires cas et leurs lectures.
pub const PIRES_CAS: u8 = 2;
/// La soirée du plan court.
pub const SOIREE: u8 = 3;

/// L'issue d'un appel rendu.
pub const ISSUE_RENDU: u8 = 0;
/// L'issue d'un appel en faute.
pub const ISSUE_FAUTE: u8 = 1;
/// L'issue d'un appel en panne.
pub const ISSUE_PANNE: u8 = 2;

/// L'indice d'un appel dans [`NOMS`].
pub const fn indice_du_nom(a: &Appel<'_>) -> u8 {
    match a {
        Appel::Bonjour => 0,
        Appel::Decrire => 1,
        Appel::Creer { .. } => 2,
        Appel::Valider { .. } => 3,
        Appel::Entree { .. } => 4,
        Appel::Regarder { .. } => 5,
        Appel::Etat { .. } => 6,
        Appel::Ressources { .. } => 7,
        Appel::Projeter { .. } => 8,
        Appel::Charger { .. } => 9,
        Appel::Canonique { .. } => 10,
        Appel::DepuisCanonique { .. } => 11,
        Appel::Garder => 12,
        Appel::Resumer => 13,
        Appel::Verifier => 14,
        Appel::Transition { .. } => 15,
        Appel::Bot { .. } => 16,
        Appel::PireCas { .. } => 17,
        Appel::Memoire => 18,
        Appel::Proprietes => 19,
        Appel::Hypothese { .. } => 20,
        Appel::Page { .. } => 21,
    }
}

/// FNV-1a sur 32 bits : l'empreinte courte d'un retour. Elle ne sert qu'à voir qu'un
/// rejeu rend la même chose.
pub fn fnv(octets: &[u8]) -> u32 {
    octets.iter().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(*b)).wrapping_mul(0x0100_0193)
    })
}

/// L'issue d'un appel, sa longueur et l'empreinte courte de son retour, comme la trace
/// les écrit.
pub fn issue_de(r: &Result<Vec<u8>, Echec>) -> (u8, u32, u32) {
    match r {
        Ok(o) => (
            ISSUE_RENDU,
            u32::try_from(o.len()).unwrap_or(u32::MAX),
            fnv(o),
        ),
        Err(Echec::Faute(_)) => (ISSUE_FAUTE, 0, 0),
        Err(Echec::Panne(_)) => (ISSUE_PANNE, 0, 0),
    }
}

/// Un événement de la trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evenement {
    /// Une instance neuve, sous son numéro.
    Neuve {
        /// Son numéro.
        inst: u32,
    },
    /// Un appel, et ce qu'il a rendu dans wasmtime.
    Appel {
        /// L'instance.
        inst: u32,
        /// La section ([`SECTIONS`]).
        section: u8,
        /// Le nom ([`NOMS`]), ou `u8::MAX` s'il ne se décode pas.
        nom: u8,
        /// La sorte de l'appel.
        sorte: SorteAppel,
        /// Le budget.
        budget: u64,
        /// Les octets de l'appel.
        octets: Vec<u8>,
        /// Le carburant brûlé.
        carburant: u64,
        /// [`ISSUE_RENDU`], [`ISSUE_FAUTE`] ou [`ISSUE_PANNE`].
        issue: u8,
        /// La longueur du retour.
        sortie_longueur: u32,
        /// L'empreinte courte du retour ([`fnv`]).
        sortie_fnv: u32,
    },
    /// Un instantané de l'instance, sous son numéro.
    Instantane {
        /// L'instance.
        inst: u32,
        /// Le numéro de l'instantané.
        id: u32,
    },
    /// L'instance remise à un instantané.
    Restaurer {
        /// L'instance.
        inst: u32,
        /// Le numéro de l'instantané.
        id: u32,
    },
    /// Un instantané qui ne sert plus.
    Oublier {
        /// Son numéro.
        id: u32,
    },
    /// Une instance lâchée.
    Lacher {
        /// Son numéro.
        inst: u32,
    },
}

/// Écrit une trace.
pub fn ecrire(evenements: &[Evenement]) -> Vec<u8> {
    let mut o = Vec::new();
    o.extend_from_slice(MAGIE);
    for e in evenements {
        match e {
            Evenement::Neuve { inst } => {
                o.push(1);
                o.extend_from_slice(&inst.to_le_bytes());
            }
            Evenement::Appel {
                inst,
                section,
                nom,
                sorte,
                budget,
                octets,
                carburant,
                issue,
                sortie_longueur,
                sortie_fnv,
            } => {
                o.push(2);
                o.extend_from_slice(&inst.to_le_bytes());
                o.push(*section);
                o.push(*nom);
                o.extend_from_slice(&sorte.code().to_le_bytes());
                o.extend_from_slice(&budget.to_le_bytes());
                o.extend_from_slice(&u32::try_from(octets.len()).unwrap_or(0).to_le_bytes());
                o.extend_from_slice(octets);
                o.extend_from_slice(&carburant.to_le_bytes());
                o.push(*issue);
                o.extend_from_slice(&sortie_longueur.to_le_bytes());
                o.extend_from_slice(&sortie_fnv.to_le_bytes());
            }
            Evenement::Instantane { inst, id } => {
                o.push(3);
                o.extend_from_slice(&inst.to_le_bytes());
                o.extend_from_slice(&id.to_le_bytes());
            }
            Evenement::Restaurer { inst, id } => {
                o.push(4);
                o.extend_from_slice(&inst.to_le_bytes());
                o.extend_from_slice(&id.to_le_bytes());
            }
            Evenement::Oublier { id } => {
                o.push(5);
                o.extend_from_slice(&id.to_le_bytes());
            }
            Evenement::Lacher { inst } => {
                o.push(6);
                o.extend_from_slice(&inst.to_le_bytes());
            }
        }
    }
    o.push(0);
    o
}

/// Un curseur sur les octets d'une trace.
struct Lecteur<'a> {
    reste: &'a [u8],
    lus: usize,
}

impl<'a> Lecteur<'a> {
    fn prendre(&mut self, n: usize) -> Result<&'a [u8], String> {
        let (t, r) = self
            .reste
            .split_at_checked(n)
            .ok_or_else(|| format!("trace coupée à l'octet {}", self.lus))?;
        self.reste = r;
        self.lus = self.lus.saturating_add(n);
        Ok(t)
    }

    fn u8(&mut self) -> Result<u8, String> {
        self.prendre(1)?
            .first()
            .copied()
            .ok_or_else(|| String::from("trace coupée"))
    }

    fn u32(&mut self) -> Result<u32, String> {
        let t = self.prendre(4)?;
        Ok(u32::from_le_bytes(
            t.try_into().map_err(|_| String::from("trace coupée"))?,
        ))
    }

    fn i32(&mut self) -> Result<i32, String> {
        self.u32().map(u32::cast_signed)
    }

    fn u64(&mut self) -> Result<u64, String> {
        let t = self.prendre(8)?;
        Ok(u64::from_le_bytes(
            t.try_into().map_err(|_| String::from("trace coupée"))?,
        ))
    }
}

/// Lit une trace, strictement : l'en-tête, des événements connus, la fin, et rien après.
pub fn lire(octets: &[u8]) -> Result<Vec<Evenement>, String> {
    let mut l = Lecteur {
        reste: octets,
        lus: 0,
    };
    if l.prendre(MAGIE.len())? != MAGIE {
        return Err(String::from("pas une trace d'étalonnage (en-tête)"));
    }
    let mut v = Vec::new();
    loop {
        let debut = l.lus;
        let e = match l.u8()? {
            0 => {
                if !l.reste.is_empty() {
                    return Err(format!("{} octets après la fin de la trace", l.reste.len()));
                }
                return Ok(v);
            }
            1 => Evenement::Neuve { inst: l.u32()? },
            2 => {
                let inst = l.u32()?;
                let section = l.u8()?;
                let nom = l.u8()?;
                let code = l.i32()?;
                let sorte = SorteAppel::depuis_code(code)
                    .ok_or_else(|| format!("sorte d'appel inconnue {code} à l'octet {debut}"))?;
                let budget = l.u64()?;
                let n = usize::try_from(l.u32()?).map_err(|_| String::from("appel trop long"))?;
                let octets = l.prendre(n)?.to_vec();
                Evenement::Appel {
                    inst,
                    section,
                    nom,
                    sorte,
                    budget,
                    octets,
                    carburant: l.u64()?,
                    issue: l.u8()?,
                    sortie_longueur: l.u32()?,
                    sortie_fnv: l.u32()?,
                }
            }
            3 => Evenement::Instantane {
                inst: l.u32()?,
                id: l.u32()?,
            },
            4 => Evenement::Restaurer {
                inst: l.u32()?,
                id: l.u32()?,
            },
            5 => Evenement::Oublier { id: l.u32()? },
            6 => Evenement::Lacher { inst: l.u32()? },
            autre => return Err(format!("événement inconnu {autre} à l'octet {debut}")),
        };
        v.push(e);
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use super::{Evenement, ecrire, fnv, lire};
    use jeu_abi::SorteAppel;

    fn exemple() -> Vec<Evenement> {
        vec![
            Evenement::Neuve { inst: 0 },
            Evenement::Appel {
                inst: 0,
                section: 1,
                nom: 0,
                sorte: SorteAppel::Lecture,
                budget: 1 << 28,
                octets: vec![1, 2, 3],
                carburant: 3104,
                issue: 0,
                sortie_longueur: 42,
                sortie_fnv: 0xdead_beef,
            },
            Evenement::Instantane { inst: 0, id: 7 },
            Evenement::Restaurer { inst: 0, id: 7 },
            Evenement::Oublier { id: 7 },
            Evenement::Lacher { inst: 0 },
        ]
    }

    #[test]
    fn la_trace_se_relit_a_l_identique() {
        let o = ecrire(&exemple());
        assert_eq!(lire(&o).unwrap(), exemple());
        // Octets figés de l'en-tête et du premier événement.
        assert_eq!(o.get(..13).unwrap(), b"SOIRTRC1\x01\0\0\0\0");
    }

    #[test]
    fn une_trace_abimee_ne_se_lit_pas() {
        let o = ecrire(&exemple());
        for n in 0..o.len() {
            assert!(lire(o.get(..n).unwrap()).is_err(), "coupée à {n}");
        }
        let mut plus = o.clone();
        plus.push(0);
        assert!(lire(&plus).is_err());
        let mut inconnu = o;
        *inconnu.get_mut(8).unwrap() = 9;
        assert!(lire(&inconnu).is_err());
    }

    #[test]
    fn fnv_est_celle_du_harnais() {
        // FNV-1a 32 bits de « a » : la valeur publiée de l'algorithme.
        assert_eq!(fnv(b"a"), 0xe40c_292c);
        assert_eq!(fnv(b""), 0x811c_9dc5);
    }
}
