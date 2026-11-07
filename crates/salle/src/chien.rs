//! Le chien de garde de la page de l'écran (PHASE-3, § 2.3, P5, P39), sans entrées-sorties :
//! la page lui donne l'heure de ses propres minuteries et ce que le Worker lui envoie ; il
//! dit quand tuer le Worker, s'il faut le relancer, et quelle entrée lui faire écarter.
//!
//! - **Deux garde-fous muraux.** Pendant un appel du chargement (`Bonjour`, `Decrire`),
//!   que le Worker annonce par `VersPage::Chargement`, [`GARDE_FOU_CHARGEMENT_MS`] : celui
//!   de wasmtime, inchangé, dix fois au-dessus de `BUDGET_DE_CHARGEMENT` au pire mesuré
//!   (§ 2.3, lot 3c). Sinon, [`GARDE_FOU_PARTIE_MS`]. Tout message du Worker est un signe
//!   de vie : il poste `EnCours` avant chaque `Entree` et un battement à chaque tour, et
//!   rien pendant un appel qui ne rend pas la main.
//! - **« Je n'ai pas tourné » n'accuse pas l'autre** (REM-02). Le garde-fou ne compte que
//!   si les minuteries de la page ont elles-mêmes tourné pendant tout l'intervalle : un
//!   écart de plus de [`TIC_MAX_MS`] entre deux tics (la page cachée, la machine en pause)
//!   rend au Worker un intervalle entier.
//! - **Une entrée qui bloque deux fois est écartée** (P39) : au second arrêt mural pendant
//!   le même geste en cours ([`ARRETS_AVANT_D_ECARTER`]), le Worker neuf reçoit
//!   `Reprendre { ecarter }`.
//! - **Au-delà de [`RELANCES_MAX`] relances en [`FENETRE_RELANCES_MS`]**, la page ne relance
//!   plus : « Le commun est en panne » (§ 2.3).
//! - **Le retard des réveils** (§ 2.3, « L'arrière-plan ») : un battement en retard de
//!   plus de [`RETARD_SIGNALE_MS`], [`RETARDS_DE_SUITE`] fois de suite, se signale (M6).

use std::collections::BTreeMap;

use jeu_abi::{IdGeste, Oui};

use crate::ecran::VersPage;

/// Le garde-fou mural pendant la partie : 2 000 ms (choisi ; § 2.3, annexe des valeurs
/// choisies).
pub const GARDE_FOU_PARTIE_MS: u64 = 2_000;

/// Le garde-fou mural pendant un appel du chargement : 1 000 ms, celui de wasmtime
/// (`ReglagesMoteur::garde_fou_murale_ms`), inchangé (annexe des valeurs choisies).
pub const GARDE_FOU_CHARGEMENT_MS: u64 = 1_000;

/// La période des minuteries de la page qui font tourner le chien (choisi).
pub const TIC_MS: u64 = 100;

/// Un écart plus grand entre deux tics : la page n'a pas tourné, et le Worker n'est pas
/// accusé de ce temps-là (REM-02 ; choisi : cinq tics).
pub const TIC_MAX_MS: u64 = 500;

/// Les arrêts muraux sur le même geste avant de l'écarter : 2 (choisi, P39 : le premier
/// peut venir d'une pause de la machine).
pub const ARRETS_AVANT_D_ECARTER: u32 = 2;

/// Les relances du Worker permises dans [`FENETRE_RELANCES_MS`] : 3 (choisi, § 2.3).
pub const RELANCES_MAX: usize = 3;

/// La fenêtre des relances : une minute (choisi, § 2.3).
pub const FENETRE_RELANCES_MS: u64 = 60_000;

/// Le retard d'un réveil qui compte (choisi, M6).
pub const RETARD_SIGNALE_MS: u32 = 100;

/// Les réveils en retard de suite avant de le signaler (choisi, M6).
pub const RETARDS_DE_SUITE: u32 = 3;

/// Ce que la page doit faire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Rien.
    Rien,
    /// Tuer le Worker (`terminate`) et en lancer un neuf, à qui l'on demande d'écarter
    /// cette entrée, s'il y en a une.
    Relancer {
        /// Le garde-fou qui a sonné, en millisecondes.
        garde_fou_ms: u64,
        /// L'entrée à écarter.
        ecarter: Option<IdGeste>,
    },
    /// Tuer le Worker et ne plus le relancer : « Le commun est en panne ».
    Panne {
        /// Le garde-fou qui a sonné, en millisecondes.
        garde_fou_ms: u64,
    },
}

/// Le chien de garde.
#[derive(Debug, Clone, Default)]
pub struct Chien {
    /// Le dernier signe de vie, ou le début de l'intervalle rendu au Worker.
    dernier_signe_ms: Option<u64>,
    dernier_tic_ms: Option<u64>,
    en_cours: Option<IdGeste>,
    chargement: bool,
    arrets: BTreeMap<u64, u32>,
    relances: Vec<u64>,
    retards: u32,
    signales: u64,
    en_panne: bool,
}

impl Chien {
    /// Un chien neuf, sans Worker.
    pub fn new() -> Chien {
        Chien::default()
    }

    /// Un Worker neuf vient d'être lancé, à `t_ms`.
    pub fn demarre(&mut self, t_ms: u64) {
        self.dernier_signe_ms = Some(t_ms);
        self.dernier_tic_ms = Some(t_ms);
        self.en_cours = None;
        self.chargement = false;
        self.retards = 0;
    }

    /// Le Worker s'est arrêté de lui-même, ou n'est plus surveillé (remplacé par un autre
    /// onglet, en panne) : plus de garde-fou jusqu'au prochain [`Chien::demarre`].
    pub const fn arrete(&mut self) {
        self.dernier_signe_ms = None;
    }

    /// Un message du Worker, reçu à `t_ms`. Rend `true` si un retard est à signaler.
    pub fn message(&mut self, t_ms: u64, m: &VersPage<'_>) -> bool {
        if self.dernier_signe_ms.is_some() {
            self.dernier_signe_ms = Some(t_ms);
        }
        match m {
            VersPage::EnCours(id) => self.en_cours = *id,
            VersPage::Chargement(o) => self.chargement = *o == Oui::Oui,
            VersPage::Battement { retard_ms, .. } => {
                // Un tour fini : plus d'entrée en cours.
                self.en_cours = None;
                if *retard_ms > RETARD_SIGNALE_MS {
                    self.retards = self.retards.saturating_add(1);
                    if self.retards == RETARDS_DE_SUITE {
                        self.signales = self.signales.saturating_add(1);
                        return true;
                    }
                } else {
                    self.retards = 0;
                }
            }
            VersPage::Scene { .. }
            | VersPage::Ressource { .. }
            | VersPage::Demande { .. }
            | VersPage::EtatSalle(_)
            | VersPage::Panne(_)
            | VersPage::Traces(_)
            | VersPage::Jeu { .. }
            | VersPage::Accueil { .. }
            | VersPage::Cle(_)
            | VersPage::Banc(_) => {}
        }
        false
    }

    /// Les retards signalés depuis le début.
    pub const fn retards_signales(&self) -> u64 {
        self.signales
    }

    /// Le geste en cours, tel que le Worker l'a dit.
    pub const fn en_cours(&self) -> Option<IdGeste> {
        self.en_cours
    }

    /// Le commun est-il en panne (trop de relances) ?
    pub const fn en_panne(&self) -> bool {
        self.en_panne
    }

    /// Un tic des minuteries de la page, à `t_ms`.
    pub fn tic(&mut self, t_ms: u64) -> Decision {
        let ecart = self.dernier_tic_ms.map_or(0, |d| t_ms.saturating_sub(d));
        self.dernier_tic_ms = Some(t_ms);
        let Some(signe) = self.dernier_signe_ms else {
            return Decision::Rien;
        };
        if ecart > TIC_MAX_MS {
            // La page n'a pas tourné : le Worker n'est pas accusé de ce temps-là (REM-02).
            self.dernier_signe_ms = Some(t_ms);
            return Decision::Rien;
        }
        let garde_fou_ms = if self.chargement {
            GARDE_FOU_CHARGEMENT_MS
        } else {
            GARDE_FOU_PARTIE_MS
        };
        if t_ms.saturating_sub(signe) <= garde_fou_ms {
            return Decision::Rien;
        }
        // Un arrêt mural : une panne de l'outil, jamais une faute du jeu (OUT-11, HIS-06).
        self.dernier_signe_ms = None;
        let ecarter = self.en_cours.and_then(|id| {
            let n = self.arrets.entry(id.fil()).or_insert(0);
            *n = n.saturating_add(1);
            (*n >= ARRETS_AVANT_D_ECARTER).then_some(id)
        });
        self.relances
            .retain(|&r| t_ms.saturating_sub(r) < FENETRE_RELANCES_MS);
        if self.relances.len() >= RELANCES_MAX {
            self.en_panne = true;
            return Decision::Panne { garde_fou_ms };
        }
        self.relances.push(t_ms);
        Decision::Relancer {
            garde_fou_ms,
            ecarter,
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::arithmetic_side_effects,
    clippy::many_single_char_names,
    reason = "code de test : des instants petits, nommés comme dans le texte ; un débordement y serait l'échec du test"
)]
mod tests {
    use jeu_abi::{IdGeste, Oui, Pas};

    use super::{
        Chien, Decision, GARDE_FOU_CHARGEMENT_MS, GARDE_FOU_PARTIE_MS, RELANCES_MAX, TIC_MS,
    };
    use crate::ecran::VersPage;

    fn tourner(c: &mut Chien, de: u64, a: u64) -> Option<(u64, Decision)> {
        let mut t = de;
        while t <= a {
            let d = c.tic(t);
            if d != Decision::Rien {
                return Some((t, d));
            }
            t += TIC_MS;
        }
        None
    }

    #[test]
    fn un_worker_qui_bat_n_est_jamais_tue() {
        let mut c = Chien::new();
        c.demarre(0);
        let mut t = 0;
        while t < 60_000 {
            assert_eq!(c.tic(t), Decision::Rien);
            if t % 1_000 == 0 {
                let _: bool = c.message(
                    t,
                    &VersPage::Battement {
                        pas: Pas::ZERO,
                        retard_ms: 0,
                    },
                );
            }
            t += TIC_MS;
        }
    }

    #[test]
    fn un_worker_muet_est_tue_apres_son_garde_fou() {
        let mut c = Chien::new();
        c.demarre(0);
        let (t, d) = tourner(&mut c, 0, 10_000).unwrap();
        assert!(t > GARDE_FOU_PARTIE_MS && t <= GARDE_FOU_PARTIE_MS + TIC_MS);
        assert_eq!(
            d,
            Decision::Relancer {
                garde_fou_ms: GARDE_FOU_PARTIE_MS,
                ecarter: None
            }
        );
        // Plus rien tant qu'un Worker neuf n'est pas lancé.
        assert_eq!(tourner(&mut c, t + TIC_MS, t + 10_000), None);
    }

    #[test]
    fn pendant_le_chargement_le_garde_fou_est_celui_de_wasmtime() {
        let mut c = Chien::new();
        c.demarre(0);
        let _: bool = c.message(0, &VersPage::Chargement(Oui::Oui));
        let (t, d) = tourner(&mut c, 0, 10_000).unwrap();
        assert!(t > GARDE_FOU_CHARGEMENT_MS && t <= GARDE_FOU_CHARGEMENT_MS + TIC_MS);
        assert!(
            matches!(d, Decision::Relancer { garde_fou_ms, .. } if garde_fou_ms == GARDE_FOU_CHARGEMENT_MS)
        );
        c.demarre(t);
        let _: bool = c.message(t, &VersPage::Chargement(Oui::Oui));
        let _: bool = c.message(t + 500, &VersPage::Chargement(Oui::Non));
        let (u, _) = tourner(&mut c, t, t + 10_000).unwrap();
        assert!(u > t + 500 + GARDE_FOU_PARTIE_MS);
    }

    /// REM-02 : la page qui n'a pas tourné n'accuse pas le Worker.
    #[test]
    fn une_page_gelee_n_accuse_pas_le_worker_rem02() {
        let mut c = Chien::new();
        c.demarre(0);
        assert_eq!(tourner(&mut c, 0, 1_000), None);
        // La page reste gelée cinq secondes : au réveil, l'intervalle recommence.
        assert_eq!(c.tic(6_000), Decision::Rien);
        let (t, _) = tourner(&mut c, 6_000 + TIC_MS, 20_000).unwrap();
        assert!(t > 6_000 + GARDE_FOU_PARTIE_MS);
    }

    /// P39 : au second arrêt sur le même geste, le Worker neuf l'écarte ; un autre geste
    /// ne compte pas pour lui.
    #[test]
    fn le_second_arret_sur_le_meme_geste_l_ecarte_p39() {
        let mut c = Chien::new();
        let g = IdGeste::du_fil(42);
        c.demarre(0);
        let _: bool = c.message(10, &VersPage::EnCours(Some(g)));
        let (t, d) = tourner(&mut c, 0, 10_000).unwrap();
        assert!(matches!(d, Decision::Relancer { ecarter: None, .. }));
        c.demarre(t);
        let _: bool = c.message(t + 10, &VersPage::EnCours(Some(IdGeste::du_fil(7))));
        let (t, d) = tourner(&mut c, t, t + 10_000).unwrap();
        assert!(matches!(d, Decision::Relancer { ecarter: None, .. }));
        c.demarre(t);
        let _: bool = c.message(t + 10, &VersPage::EnCours(Some(g)));
        let (_, d) = tourner(&mut c, t, t + 10_000).unwrap();
        assert!(matches!(d, Decision::Relancer { ecarter: Some(x), .. } if x == g));
    }

    #[test]
    fn un_battement_finit_le_geste_en_cours() {
        let mut c = Chien::new();
        c.demarre(0);
        let _: bool = c.message(10, &VersPage::EnCours(Some(IdGeste::du_fil(1))));
        let _: bool = c.message(
            20,
            &VersPage::Battement {
                pas: Pas::ZERO,
                retard_ms: 0,
            },
        );
        assert_eq!(c.en_cours(), None);
    }

    #[test]
    fn au_dela_de_trois_relances_en_une_minute_le_commun_est_en_panne() {
        let mut c = Chien::new();
        let mut t = 0;
        for _ in 0..RELANCES_MAX {
            c.demarre(t);
            let (u, d) = tourner(&mut c, t, t + 10_000).unwrap();
            assert!(matches!(d, Decision::Relancer { .. }));
            t = u;
        }
        c.demarre(t);
        let (_, d) = tourner(&mut c, t, t + 10_000).unwrap();
        assert!(matches!(d, Decision::Panne { .. }));
        assert!(c.en_panne());
        // Une minute plus tard, les relances anciennes sont oubliées.
        let mut c = Chien::new();
        let mut t = 0;
        for _ in 0..RELANCES_MAX {
            c.demarre(t);
            t = tourner(&mut c, t, t + 10_000).unwrap().0 + 30_000;
        }
        c.demarre(t);
        let (_, d) = tourner(&mut c, t, t + 10_000).unwrap();
        assert!(matches!(d, Decision::Relancer { .. }));
    }

    #[test]
    fn trois_retards_de_suite_se_signalent_une_fois() {
        let mut c = Chien::new();
        c.demarre(0);
        let battre = |c: &mut Chien, r: u32| {
            c.message(
                0,
                &VersPage::Battement {
                    pas: Pas::ZERO,
                    retard_ms: r,
                },
            )
        };
        assert!(!battre(&mut c, 150));
        assert!(!battre(&mut c, 150));
        assert!(!battre(&mut c, 20));
        assert!(!battre(&mut c, 150));
        assert!(!battre(&mut c, 150));
        assert!(battre(&mut c, 101));
        assert!(!battre(&mut c, 150));
        assert_eq!(c.retards_signales(), 1);
        assert!(!battre(&mut c, 100));
    }
}
