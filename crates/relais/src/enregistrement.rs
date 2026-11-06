//! L'enregistrement du relais (`--enregistrer <fichier>`) : **tout ce qu'il voit**, chaque
//! trame reçue et envoyée sur chaque lien, telle quelle, avec l'instant, le lien et le
//! sens, et l'ouverture et la fermeture de chaque lien. Le banc y cherche ce que le
//! relais ne doit jamais voir (PHASE-3, § 2.5 : le secret d'entrée, G, les jetons, les
//! gestes, les vues) : `le_relais_ne_voit_aucun_secret` (§ 8.1). Il y relit aussi la
//! chaîne entière des cases écrites dans une soirée, que la garde sur disque n'a plus
//! (elle n'en tient que les dernières).
//!
//! Chaque note s'écrit d'un seul `write` sur le fichier, sans tampon dans le processus :
//! un `kill -9` du relais n'en perd aucune de celles qui sont écrites, et au plus coupe
//! la dernière, que [`lire`] laisse.
//!
//! La forme : l'en-tête [`ENTETE`], puis des notes `instant (8, petit-boutiste) ‖ lien
//! (4) ‖ sens (1) ‖ longueur (4) ‖ octets`.

use std::fs::File;
use std::io::Write as _;
use std::path::Path;

use liaison::Lien;

/// L'en-tête d'un enregistrement.
pub const ENTETE: &[u8] = b"soiree-relais enregistrement 1\n";

/// Le sens d'une note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sens {
    /// Une trame reçue d'un bout.
    Recu,
    /// Une trame envoyée à un bout.
    Envoye,
    /// Le lien s'ouvre ; les octets sont l'adresse du pair, en texte.
    Ouvert,
    /// Le lien se ferme.
    Ferme,
}

impl Sens {
    const fn octet(self) -> u8 {
        match self {
            Sens::Recu => 0,
            Sens::Envoye => 1,
            Sens::Ouvert => 2,
            Sens::Ferme => 3,
        }
    }

    const fn depuis(o: u8) -> Option<Sens> {
        match o {
            0 => Some(Sens::Recu),
            1 => Some(Sens::Envoye),
            2 => Some(Sens::Ouvert),
            3 => Some(Sens::Ferme),
            _ => None,
        }
    }
}

/// Une note de l'enregistrement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// L'instant, en microsecondes depuis le lancement du relais.
    pub t_us: u64,
    /// Le lien.
    pub lien: Lien,
    /// Le sens.
    pub sens: Sens,
    /// Les octets.
    pub octets: Vec<u8>,
}

/// Celui qui écrit l'enregistrement.
#[derive(Debug)]
pub struct Enregistreur {
    fichier: File,
}

impl Enregistreur {
    /// Crée le fichier (il ne doit pas exister) et écrit l'en-tête.
    pub fn creer(chemin: &Path) -> std::io::Result<Enregistreur> {
        let mut fichier = File::options().write(true).create_new(true).open(chemin)?;
        fichier.write_all(ENTETE)?;
        Ok(Enregistreur { fichier })
    }

    /// Écrit une note, d'un seul `write`.
    pub fn noter(&mut self, t_us: u64, lien: Lien, sens: Sens, octets: &[u8]) {
        let longueur = u32::try_from(octets.len()).unwrap_or(u32::MAX);
        let mut n = Vec::with_capacity(octets.len().saturating_add(17));
        n.extend_from_slice(&t_us.to_le_bytes());
        n.extend_from_slice(&lien.0.to_le_bytes());
        n.push(sens.octet());
        n.extend_from_slice(&longueur.to_le_bytes());
        n.extend_from_slice(octets);
        let _: std::io::Result<()> = self.fichier.write_all(&n);
    }
}

/// Lit un enregistrement : ses notes, et `false` s'il finit sur une note coupée (un
/// `kill -9`) ou ne commence pas par l'en-tête.
pub fn lire(octets: &[u8]) -> (Vec<Note>, bool) {
    let Some(mut reste) = octets.strip_prefix(ENTETE) else {
        return (Vec::new(), false);
    };
    let mut notes = Vec::new();
    loop {
        if reste.is_empty() {
            return (notes, true);
        }
        let Some((tete, apres)) = reste.split_first_chunk::<17>() else {
            return (notes, false);
        };
        let (instant, suite) = tete.split_first_chunk::<8>().unwrap_or((&[0; 8], &[]));
        let (lien, suite) = suite.split_first_chunk::<4>().unwrap_or((&[0; 4], &[]));
        let (sens, suite) = suite.split_first_chunk::<1>().unwrap_or((&[0; 1], &[]));
        let (longueur, _) = suite.split_first_chunk::<4>().unwrap_or((&[0; 4], &[]));
        let [sens] = *sens;
        let Some(sens) = Sens::depuis(sens) else {
            return (notes, false);
        };
        let longueur = usize::try_from(u32::from_le_bytes(*longueur)).unwrap_or(usize::MAX);
        let Some((corps, suite)) = apres.split_at_checked(longueur) else {
            return (notes, false);
        };
        notes.push(Note {
            t_us: u64::from_le_bytes(*instant),
            lien: Lien(u32::from_le_bytes(*lien)),
            sens,
            octets: corps.to_vec(),
        });
        reste = suite;
    }
}
