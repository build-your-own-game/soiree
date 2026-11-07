//! Ce que la page de la télé fait d'un message du Worker (`salle::ecran::VersPage`) : la
//! scène à dessiner, les polices et les images du jeu. Sans navigateur : testé en natif.
//!
//! Une ressource dont les octets sont une police sfnt (P22, `mise_en_page::valider_police`)
//! est une police ; une police refusée (WOFF, WOFF2, collection, illisible, homonyme) est
//! refusée et dite, comme E1 la refuse ; un son (RIFF/WAVE, Ogg, FLAC, MP3) se garde pour
//! `decodeAudioData` (lot 10b) ; tout autre contenu est une image, que la page décode
//! (`createImageBitmap`, P23). Les autres messages ne servent pas encore à la page
//! (lots 10a à 13) : ils sont reconnus, et laissés.

use jeu_abi::{Cle, Pas, SceneFil, decoder};
use mise_en_page::{Polices, RefusPolice, valider_police};
use salle::ecran::VersPage;

/// Ce qu'un message apporte à la page.
#[derive(Debug, Clone, PartialEq)]
pub enum Recu {
    /// Une scène à dessiner.
    Scene {
        /// Le pas dont elle est la vue.
        pas: Pas,
        /// La scène.
        scene: SceneFil,
    },
    /// Une police, ajoutée aux polices.
    Police(Cle),
    /// Une image à décoder.
    Image {
        /// Sa clé.
        cle: Cle,
        /// Ses octets.
        octets: Vec<u8>,
    },
    /// Un son, gardé tel quel : la page le décode quand un effet le joue (lot 10b).
    Son {
        /// Sa clé.
        cle: Cle,
        /// Ses octets.
        octets: Vec<u8>,
    },
    /// Un message que la page ne traite pas encore.
    Laisse(&'static str),
}

/// Ces octets sont-ils un son que `decodeAudioData` lit : WAVE (`RIFF…WAVE`), Ogg
/// (`OggS`), FLAC (`fLaC`), MP3 (une étiquette `ID3`, ou une trame qui commence par onze
/// bits à un) ?
pub fn est_un_son(o: &[u8]) -> bool {
    let wave = o.get(..4) == Some(b"RIFF") && o.get(8..12) == Some(b"WAVE");
    let mp3_trame = matches!(o.get(..2), Some([0xff, b]) if b & 0xe0 == 0xe0);
    wave || o.starts_with(b"OggS") || o.starts_with(b"fLaC") || o.starts_with(b"ID3") || mp3_trame
}

/// Lit un message du Worker ; une police s'ajoute à `polices`.
pub fn recevoir(octets: &[u8], polices: &mut Polices) -> Result<Recu, String> {
    let m = VersPage::decoder(octets).map_err(|e| format!("message illisible : {e}"))?;
    match m {
        VersPage::Scene { pas, octets, .. } => {
            let scene =
                decoder::<SceneFil>(octets).map_err(|e| format!("scène illisible : {e}"))?;
            Ok(Recu::Scene { pas, scene })
        }
        VersPage::Ressource { cle, octets } => {
            let cle = Cle::du_fil(cle);
            match valider_police(octets) {
                Ok(()) => {
                    polices
                        .ajouter(cle, octets.to_vec())
                        .map_err(|e| format!("police {:016x} refusée : {e}", cle.fil()))?;
                    Ok(Recu::Police(cle))
                }
                Err(RefusPolice::PasSfnt { .. } | RefusPolice::Vide) if est_un_son(octets) => {
                    Ok(Recu::Son {
                        cle,
                        octets: octets.to_vec(),
                    })
                }
                Err(RefusPolice::PasSfnt { .. } | RefusPolice::Vide) => Ok(Recu::Image {
                    cle,
                    octets: octets.to_vec(),
                }),
                Err(
                    e @ (RefusPolice::Woff
                    | RefusPolice::Woff2
                    | RefusPolice::Collection
                    | RefusPolice::Illisible
                    | RefusPolice::Homonyme { .. }),
                ) => Err(format!("police {:016x} refusée : {e}", cle.fil())),
            }
        }
        VersPage::Demande { .. } => Ok(Recu::Laisse("demande")),
        VersPage::EtatSalle(_) => Ok(Recu::Laisse("état de la salle")),
        VersPage::Battement { .. } => Ok(Recu::Laisse("battement")),
        VersPage::Panne(_) => Ok(Recu::Laisse("panne")),
        VersPage::Traces(_) => Ok(Recu::Laisse("traces")),
        VersPage::EnCours(_) => Ok(Recu::Laisse("geste en cours")),
        VersPage::Chargement(_) => Ok(Recu::Laisse("chargement")),
        VersPage::Jeu { .. } => Ok(Recu::Laisse("jeu")),
        VersPage::Accueil { .. } => Ok(Recu::Laisse("accueil")),
        VersPage::Cle(_) => Ok(Recu::Laisse("clé de garde")),
        VersPage::Banc(_) => Ok(Recu::Laisse("banc")),
    }
}
