//! Ce que le Worker garde dans `IndexedDB` (PHASE-3, § 2.4, P37) : la clé de garde G, la
//! clé privée de l'autorité, le secret d'entrée, le code de salle, le module joué et le
//! dernier numéro de case que le relais a dit écrit, **dans un seul enregistrement** : ils
//! se perdent ensemble (fenêtre privée fermée, données du site effacées, autre navigateur),
//! et sans G la garde ne se relit plus.
//!
//! Une forme fixe, sans serde : [`VERSION_ENREGISTREMENT`] (`u32` petit-boutiste), le code
//! de salle (4), la clé privée (32), le secret (16), G (32), le module (32), puis le
//! dernier numéro (`0`, ou `1` et un `u64` petit-boutiste). Relue strictement : une autre
//! version, une autre longueur, un octet de drapeau inconnu, sont un enregistrement
//! illisible, et le Worker le dit au lieu d'ouvrir une salle neuve en silence.

/// La version de la forme. Monte quand elle change ; `tests/enregistrement.rs` la fige.
pub const VERSION_ENREGISTREMENT: u32 = 1;

/// La longueur d'un enregistrement sans dernier numéro.
const COURT: usize = 4 + 4 + 32 + 16 + 32 + 32 + 1;

/// La longueur d'un enregistrement avec son dernier numéro.
const LONG: usize = COURT + 8;

/// Ce que le Worker garde d'une salle.
#[derive(Clone, PartialEq, Eq)]
pub struct Enregistrement {
    /// Le code de salle.
    pub salle: [u8; 4],
    /// La clé privée de l'autorité ; sa clé publique est dans le QR.
    pub privee: [u8; 32],
    /// Le secret d'entrée, dans le QR.
    pub secret: [u8; 16],
    /// La clé de garde G : jamais dans le QR, ni dans aucune adresse.
    pub garde: [u8; 32],
    /// Le BLAKE3 du module livré que la salle joue.
    pub module: [u8; 32],
    /// Le dernier numéro de case que le relais a dit écrit (`Autorite::dernier_ecrit`).
    pub dernier: Option<u64>,
}

impl core::fmt::Debug for Enregistrement {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Aucun secret dans un `Debug`.
        f.debug_struct("Enregistrement")
            .field("salle", &self.salle)
            .field("dernier", &self.dernier)
            .finish_non_exhaustive()
    }
}

/// Un enregistrement qui ne se relit pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Illisible {
    /// Une autre version de la forme.
    Version(u32),
    /// Une autre longueur.
    Longueur(usize),
    /// Le drapeau du dernier numéro n'est ni 0 ni 1, ou ne s'accorde pas à la longueur.
    Drapeau(u8),
}

impl Enregistrement {
    /// Les octets de l'enregistrement.
    pub fn encoder(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(LONG);
        o.extend_from_slice(&VERSION_ENREGISTREMENT.to_le_bytes());
        o.extend_from_slice(&self.salle);
        o.extend_from_slice(&self.privee);
        o.extend_from_slice(&self.secret);
        o.extend_from_slice(&self.garde);
        o.extend_from_slice(&self.module);
        match self.dernier {
            None => o.push(0),
            Some(n) => {
                o.push(1);
                o.extend_from_slice(&n.to_le_bytes());
            }
        }
        o
    }

    /// Relit un enregistrement, strictement.
    pub fn decoder(octets: &[u8]) -> Result<Enregistrement, Illisible> {
        let Some((version, reste)) = octets.split_first_chunk::<4>() else {
            return Err(Illisible::Longueur(octets.len()));
        };
        let version = u32::from_le_bytes(*version);
        if version != VERSION_ENREGISTREMENT {
            return Err(Illisible::Version(version));
        }
        if octets.len() != COURT && octets.len() != LONG {
            return Err(Illisible::Longueur(octets.len()));
        }
        let (salle, reste) = reste
            .split_first_chunk::<4>()
            .ok_or(Illisible::Longueur(octets.len()))?;
        let (privee, reste) = reste
            .split_first_chunk::<32>()
            .ok_or(Illisible::Longueur(octets.len()))?;
        let (secret, reste) = reste
            .split_first_chunk::<16>()
            .ok_or(Illisible::Longueur(octets.len()))?;
        let (garde, reste) = reste
            .split_first_chunk::<32>()
            .ok_or(Illisible::Longueur(octets.len()))?;
        let (module, reste) = reste
            .split_first_chunk::<32>()
            .ok_or(Illisible::Longueur(octets.len()))?;
        let dernier = match reste {
            [0] => None,
            [1, n @ ..] => {
                let n: [u8; 8] = n.try_into().map_err(|_| Illisible::Drapeau(1))?;
                Some(u64::from_le_bytes(n))
            }
            [d, ..] => return Err(Illisible::Drapeau(*d)),
            [] => return Err(Illisible::Longueur(octets.len())),
        };
        Ok(Enregistrement {
            salle: *salle,
            privee: *privee,
            secret: *secret,
            garde: *garde,
            module: *module,
            dernier,
        })
    }
}
