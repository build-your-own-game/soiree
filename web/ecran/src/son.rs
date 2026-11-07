//! Les effets sonores de la télé (PHASE-3, § 2.9 ; lot 10b), sans navigateur : ce que la
//! page fait de chaque effet d'une scène reçue. Le son lui-même (`AudioContext`, les
//! oscillateurs, `decodeAudioData`) est dans la page ; ce qui se décide est ici, testé en
//! natif.
//!
//! - **Une identité ne rejoue pas** (SAL-167, PUN-66) : un effet déjà vu (joué, perdu ou
//!   périmé) ne se rejoue jamais, ni à une scène redite, ni à une reconnexion de la page
//!   au même Worker. Un rechargement de la page repart sans rien, en silence : la
//!   première scène tombe avant tout geste (SAL-176, « une reconnexion établit une
//!   nouvelle référence silencieuse »).
//! - **Seul un contexte en marche joue** (SAL-174, SAL-175) : la page ne crée son contexte
//!   audio qu'au geste (« Jouer », « Cliquer pour reprendre ») ; un effet tombé avant est
//!   **compté perdu**, jamais mis en attente, et le diagnostic le dit (INTERFACE § 6.4).
//! - **Le temps de l'événement décide s'il vaut encore** (SAL-176) : au-delà de `quand`
//!   plus sa `peremption`, l'effet est périmé, compté, jamais joué ; sinon il se programme
//!   à `quand`, ramené à l'instant de la scène (la page y ajoute la latence de sortie).
//! - Une vibration est pour un téléphone : la télé l'ignore.

use std::collections::VecDeque;

use jeu_abi::{Effet, IdEffet, Instant, Pas, Sensation, Son};
use salle::horloge::debut_du_pas;

/// Les identités retenues au plus (choisi : une scène porte quelques effets ; 256 couvrent
/// des minutes de partie, et une identité plus vieille ne revient pas).
pub const VUS_MAX: usize = 256;

/// L'état du contexte audio de la page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audio {
    /// Aucun contexte : pas encore de geste (SAL-175).
    Aucun,
    /// Un contexte qui n'est pas `running` (suspendu, interrompu, fermé).
    Suspendu,
    /// Un contexte `running` : le seul qui joue (SAL-174).
    EnMarche,
}

/// Ce que la page fait d'un effet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Le jouer, dans ce nombre de microsecondes après l'instant de la scène.
    Jouer {
        /// Le son.
        son: Son,
        /// Le délai, depuis la scène.
        dans_us: u64,
    },
    /// Perdu : aucun contexte en marche quand il est tombé.
    Perdu,
    /// Périmé : la scène arrive après `quand` plus sa péremption.
    Perime,
}

/// Les effets vus, et les comptes du diagnostic.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Effets {
    vus: VecDeque<IdEffet>,
    /// Les effets joués.
    pub joues: u64,
    /// Les effets perdus (aucun contexte en marche).
    pub perdus: u64,
    /// Les effets périmés.
    pub perimes: u64,
}

/// Un instant de la partie, en microsecondes depuis le pas 0.
fn en_us(i: Instant) -> u64 {
    let pas = debut_du_pas(i.pas);
    let suivant = debut_du_pas(Pas::du_fil(i.pas.numero().saturating_add(1)));
    let fraction = suivant
        .saturating_sub(pas)
        .saturating_mul(u64::from(i.fraction))
        / 65_536;
    pas.saturating_add(fraction)
}

impl Effets {
    /// Rien vu encore.
    pub const fn new() -> Effets {
        Effets {
            vus: VecDeque::new(),
            joues: 0,
            perdus: 0,
            perimes: 0,
        }
    }

    /// Les effets d'une scène reçue au pas `pas`, le contexte audio dans l'état `audio` :
    /// une décision par effet neuf (un effet déjà vu, ou une vibration, n'en a pas).
    pub fn recevoir(
        &mut self,
        effets: &[Effet],
        pas: Pas,
        audio: Audio,
    ) -> Vec<(IdEffet, Decision)> {
        let maintenant = debut_du_pas(pas);
        let mut decisions = Vec::new();
        for e in effets {
            let Sensation::Son(son) = e.quoi else {
                continue;
            };
            if self.vus.contains(&e.id) {
                continue;
            }
            if self.vus.len() >= VUS_MAX {
                let _: Option<IdEffet> = self.vus.pop_front();
            }
            self.vus.push_back(e.id);
            let quand = en_us(e.quand);
            let limite = quand.saturating_add(e.peremption.en_ms().saturating_mul(1000));
            let d = if maintenant > limite {
                self.perimes = self.perimes.saturating_add(1);
                Decision::Perime
            } else if audio != Audio::EnMarche {
                self.perdus = self.perdus.saturating_add(1);
                Decision::Perdu
            } else {
                self.joues = self.joues.saturating_add(1);
                Decision::Jouer {
                    son,
                    dans_us: quand.saturating_sub(maintenant),
                }
            };
            decisions.push((e.id, d));
        }
        decisions
    }
}

#[cfg(test)]
mod tests {
    use super::{Audio, Decision, Effets, VUS_MAX};
    use jeu_abi::{Duree, Effet, IdEffet, Instant, Pas, Sensation, Signal, Son, Tenue, Vibration};

    fn effet(nom: &str, pas: u64, quoi: Sensation) -> Effet {
        Effet {
            id: IdEffet::de(nom),
            quand: Instant::de(Pas::du_fil(pas)),
            quoi,
            tenue: Tenue::Definitive,
            peremption: Duree::ms(250),
        }
    }

    const DEBUT: Sensation = Sensation::Son(Son::Signal(Signal::Debut));

    /// SAL-174, SAL-175 : un effet tombé avant le geste (aucun contexte, ou un contexte
    /// suspendu) est compté perdu, et ne se joue pas plus tard, une fois le contexte en
    /// marche.
    #[test]
    fn un_effet_tombe_avant_le_geste_est_compte_perdu_sal174() {
        let mut e = Effets::new();
        let phase = effet("phase 1", 10, DEBUT);
        assert_eq!(
            e.recevoir(&[phase], Pas::du_fil(10), Audio::Aucun),
            [(phase.id, Decision::Perdu)]
        );
        assert_eq!(e.perdus, 1);
        // La même scène, redite après le geste : rien ne se rejoue.
        assert_eq!(e.recevoir(&[phase], Pas::du_fil(11), Audio::EnMarche), []);
        let suspendu = effet("phase 2", 20, DEBUT);
        assert_eq!(
            e.recevoir(&[suspendu], Pas::du_fil(20), Audio::Suspendu),
            [(suspendu.id, Decision::Perdu)]
        );
        assert_eq!((e.perdus, e.joues), (2, 0));
    }

    /// SAL-167, PUN-66 : une identité se joue une fois, quelle que soit la scène qui la
    /// redit ; un effet futur se programme à son instant.
    #[test]
    fn une_identite_se_joue_une_fois_sal167_pun66() {
        let mut e = Effets::new();
        let a = effet("manche 1", 12, DEBUT);
        assert_eq!(
            e.recevoir(&[a], Pas::du_fil(10), Audio::EnMarche),
            [(
                a.id,
                Decision::Jouer {
                    son: Son::Signal(Signal::Debut),
                    dans_us: 33_334
                }
            )]
        );
        for p in 11..20 {
            assert_eq!(e.recevoir(&[a], Pas::du_fil(p), Audio::EnMarche), []);
        }
        assert_eq!(e.joues, 1);
    }

    /// SAL-176 : au-delà de sa péremption, un effet est périmé, compté, jamais joué ; une
    /// vibration ne regarde pas la télé.
    #[test]
    fn un_effet_perime_ne_se_joue_pas_sal176() {
        let mut e = Effets::new();
        let vieux = effet("vieux", 10, DEBUT);
        // 250 ms après le pas 10, c'est le pas 25 : au pas 26, il est périmé.
        assert!(matches!(
            e.recevoir(&[vieux], Pas::du_fil(25), Audio::EnMarche)[..],
            [(_, Decision::Jouer { dans_us: 0, .. })]
        ));
        let encore = effet("encore", 10, DEBUT);
        assert_eq!(
            e.recevoir(&[encore], Pas::du_fil(26), Audio::EnMarche),
            [(encore.id, Decision::Perime)]
        );
        let v = effet("vibre", 10, Sensation::Vibration(Vibration::Courte));
        assert_eq!(e.recevoir(&[v], Pas::du_fil(10), Audio::EnMarche), []);
        assert_eq!((e.joues, e.perimes, e.perdus), (1, 1, 0));
    }

    /// Les identités retenues sont bornées.
    #[test]
    fn les_identites_retenues_sont_bornees() {
        let mut e = Effets::new();
        for k in 0..=VUS_MAX {
            let x = effet(&format!("{k}"), 1, DEBUT);
            assert_eq!(e.recevoir(&[x], Pas::du_fil(1), Audio::Aucun).len(), 1);
        }
        // La toute première est oubliée : elle compterait de nouveau.
        let premier = effet("0", 1, DEBUT);
        assert_eq!(
            e.recevoir(&[premier], Pas::du_fil(1), Audio::Aucun).len(),
            1
        );
    }
}
