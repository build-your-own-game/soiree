//! `soiree-banc rejouer-garde` (PHASE-3, § 2.4, lot 10a) : « Ce qui casse se rejoue »
//! (MANIFESTE, « La méthode »). La garde d'une vraie soirée, déchiffrée par G, rejouée par
//! `Partie::reprendre` dans wasmtime jusqu'à chaque marque F8 que l'hôte a posée.
//!
//! La garde tient au plus deux points et les cases qui les suivent ; la plus vieille ne se
//! déchiffre pas seule (sa précédente est effacée). Une marque se rejoue donc depuis le
//! dernier point : chacune qui le suit (son indice au-delà du sien) est rejouée, chaque
//! empreinte notée vérifiée jusqu'à elle, et s'écrit dans un fichier
//! `marque-<k>.soiree` : l'ouverture, le point et le journal jusqu'à la marque, de quoi
//! refaire la partie jusque-là sur la machine du développeur (HIS-06, HIS-07). Une marque
//! d'avant le dernier point est trouvée (le registre les garde toutes), et dite non
//! rejouable depuis cette garde.
//!
//! La forme d'un fichier de marque, petit-boutiste :
//!
//! ```text
//! "SOIRMRQ1", u64 indice, u64 pas, octets ouverture, octets point, octets journal
//! octets : u32 longueur, puis les octets (`partie::persistance`)
//! ```

use std::path::{Path, PathBuf};

use liaison::{Case, CodeSalle};
use moteur_wasmtime::{InstantaneWasm, Moteur, ReglagesMoteur};
use partie::Partie;
use partie::persistance::{encoder_journal, encoder_ouverture, encoder_point};
use salle::chiffre::CleDeGarde;
use salle::garde::{Marque, journal_jusqu_a, relire};
use salle::simulation::forme_et_empreinte;

/// L'en-tête d'un fichier de marque.
pub const ENTETE_MARQUE: &[u8; 8] = b"SOIRMRQ1";

/// Ce que `rejouer-garde` a trouvé d'une marque.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarqueRejouee {
    /// La marque.
    pub marque: Marque,
    /// Rejouée jusqu'à elle : la forme canonique hachée et l'empreinte de la partie là,
    /// et le fichier écrit ; ou pourquoi non.
    pub issue: Result<([u8; 32], [u8; 32], PathBuf), String>,
}

/// Ce que `rejouer-garde` a fait.
#[derive(Debug, Clone)]
pub struct Rejeu {
    /// La salle.
    pub salle: CodeSalle,
    /// Les cases de la garde lues.
    pub cases: usize,
    /// L'indice et le pas du dernier point, d'où les marques se rejouent.
    pub point: (usize, u64),
    /// Chaque marque du registre, de la plus ancienne à la plus récente.
    pub marques: Vec<MarqueRejouee>,
}

/// Lit G dans un fichier : 64 chiffres hexadécimaux (ce que la page exporte).
pub fn lire_la_cle(fichier: &Path) -> Result<CleDeGarde, String> {
    let texte =
        std::fs::read_to_string(fichier).map_err(|e| format!("{} : {e}", fichier.display()))?;
    let t = texte.trim();
    let mut o = [0u8; 32];
    if t.len() != 64 {
        return Err(format!(
            "{} : G fait 64 chiffres hexadécimaux",
            fichier.display()
        ));
    }
    for (k, case) in o.iter_mut().enumerate() {
        let d = t
            .get(k.saturating_mul(2)..k.saturating_mul(2).saturating_add(2))
            .ok_or("G illisible")?;
        *case = u8::from_str_radix(d, 16).map_err(|e| format!("G illisible : {e}"))?;
    }
    Ok(CleDeGarde::depuis_octets(o))
}

fn octets(o: &mut Vec<u8>, b: &[u8]) {
    o.extend_from_slice(&u32::try_from(b.len()).unwrap_or(u32::MAX).to_le_bytes());
    o.extend_from_slice(b);
}

/// Rejoue la garde du dossier `garde` (celui de `soiree-relais --garde`), de la salle
/// `salle` (celle que le relais garde, si `None`), jusqu'à chaque marque, sur `module` ;
/// écrit les fichiers de marque dans `sortie`.
pub fn rejouer(
    garde: &Path,
    salle: Option<CodeSalle>,
    g: &CleDeGarde,
    module: &[u8],
    sortie: &Path,
) -> Result<Rejeu, String> {
    let dossier =
        relais::garde::Dossier::ouvrir(garde).map_err(|e| format!("{} : {e}", garde.display()))?;
    let salle = salle
        .or_else(|| dossier.salle())
        .ok_or_else(|| format!("{} : aucune salle", garde.display()))?;
    let mut cases: Vec<(Case, Vec<u8>)> = Vec::new();
    for c in dossier.lister(salle).map_err(|e| format!("lister : {e}"))? {
        if let Some(o) = dossier.lire(salle, c).map_err(|e| format!("lire : {e}"))? {
            cases.push((c, o));
        }
    }
    let lue = relire::<InstantaneWasm>(g, salle, &cases, None)
        .map_err(|e| format!("la garde ne se relit pas : {e:?}"))?;
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).map_err(|e| format!("le moteur : {e}"))?;
    let charge = moteur
        .charger(module)
        .map_err(|e| format!("le module est refusé : {e}"))?;
    std::fs::create_dir_all(sortie).map_err(|e| format!("{} : {e}", sortie.display()))?;
    let indice_du_point = lue.point.indice();
    let mut marques = Vec::new();
    for (k, m) in lue.registre.marques.iter().enumerate() {
        let issue = (|| {
            let indice = usize::try_from(m.indice).map_err(|e| format!("{e}"))?;
            if indice < indice_du_point {
                return Err(format!(
                    "avant le dernier point de la garde (indice {indice_du_point}) : non rejouable depuis elle"
                ));
            }
            let journal = journal_jusqu_a(&lue.journal, indice, m.pas).ok_or_else(|| {
                format!(
                    "l'indice {indice} n'est pas dans le journal ({}..{})",
                    lue.journal.base(),
                    lue.journal.fin()
                )
            })?;
            let mut p = Partie::reprendre(
                charge.fabrique(),
                lue.ouverture.clone(),
                &lue.point,
                &journal,
            )
            .map_err(|e| format!("la reprise jusqu'à la marque échoue : {e:?}"))?;
            let (forme, empreinte) = forme_et_empreinte(&mut p)?;
            let mut o = ENTETE_MARQUE.to_vec();
            o.extend_from_slice(&m.indice.to_le_bytes());
            o.extend_from_slice(&m.pas.numero().to_le_bytes());
            octets(&mut o, &encoder_ouverture(&lue.ouverture));
            octets(&mut o, &encoder_point(&lue.point));
            octets(&mut o, &encoder_journal(&journal));
            let fichier = sortie.join(format!("marque-{k}.soiree"));
            std::fs::write(&fichier, o).map_err(|e| format!("{} : {e}", fichier.display()))?;
            Ok((forme, empreinte, fichier))
        })();
        marques.push(MarqueRejouee { marque: *m, issue });
    }
    Ok(Rejeu {
        salle,
        cases: cases.len(),
        point: (indice_du_point, lue.point.pas().numero()),
        marques,
    })
}

impl core::fmt::Display for Rejeu {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(
            f,
            "salle {} : {} cases ; dernier point à l'indice {}, pas {} ; {} marque(s)",
            self.salle,
            self.cases,
            self.point.0,
            self.point.1,
            self.marques.len()
        )?;
        for (k, m) in self.marques.iter().enumerate() {
            match &m.issue {
                Ok((forme, _, fichier)) => writeln!(
                    f,
                    "  marque {k} (indice {}, pas {}) : rejouée, forme {}…, {}",
                    m.marque.indice,
                    m.marque.pas.numero(),
                    blake3::Hash::from(*forme).to_hex().get(..16).unwrap_or(""),
                    fichier.display()
                )?,
                Err(e) => writeln!(
                    f,
                    "  marque {k} (indice {}, pas {}) : {e}",
                    m.marque.indice,
                    m.marque.pas.numero()
                )?,
            }
        }
        Ok(())
    }
}
