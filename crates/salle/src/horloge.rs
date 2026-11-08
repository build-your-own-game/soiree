//! L'horloge de la salle (PHASE-3, § 2.3 et § 2.7) : le pas que l'autorité joue, et
//! l'heure de l'autorité telle qu'un appareil l'estime.
//!
//! **Le temps entre en paramètre**, en microsecondes, comme le hasard : la coque le lit
//! (`performance.timeOrigin + performance.now()` dans le navigateur, l'horloge simulée
//! dans les tests, SAL-09). Rien ici ne lit l'heure.
//!
//! - [`Horloge`] : la correspondance, fixée par l'[`partie::Ouverture`], entre l'heure
//!   de l'ordinateur de la soirée et le temps de la partie. Elle ne dépend que de
//!   l'ouverture : une autorité qui reprend depuis la garde retrouve la même, et le
//!   temps passé à recharger devient un saut (D14).
//! - **Le temps de la partie**, en microsecondes depuis le pas 0 : c'est l'heure que
//!   l'autorité donne aux appareils (`Echo`, `Accueil`), pour qu'ils en tirent l'instant
//!   d'un geste sans connaître l'heure de l'ordinateur ([`instant_de_la_partie`]).
//! - [`Estimation`] : côté appareil, l'heure de l'autorité, gardée au plus petit
//!   aller-retour des derniers échos (SAL-127, SAL-128).
//! - [`AllerRetour`] : côté autorité, l'aller-retour qu'elle mesure elle-même vers un
//!   appareil. Un appareil peut le grossir en tardant à répondre, jamais le réduire
//!   (SAL-129) : la demi-latence qu'on donne à la partie (`regles::placer`) est celle-ci,
//!   et non une valeur qu'il annoncerait.

use jeu_abi::{Duree, Instant, PAS_PAR_SECONDE, Pas};
use partie::Ouverture;

/// Une seconde, en microsecondes.
const SECONDE_US: u128 = 1_000_000;

/// Les échos dont on garde le meilleur : le plus petit aller-retour des derniers
/// échos, et non de toute la soirée, pour qu'une horloge qui dérive soit suivie
/// (SAL-128, « se mesure en continu »). Choisi : 16 échos, 4 s à un écho par battement.
pub const ECHOS_RETENUS: usize = 16;

/// L'instant de la partie à `us` microsecondes depuis le pas 0 : le pas, et sa fraction
/// en 1/65 536.
pub fn instant_de_la_partie(us: u64) -> Instant {
    let ticks = u128::from(us).saturating_mul(u128::from(PAS_PAR_SECONDE));
    let pas = u64::try_from(ticks / SECONDE_US).unwrap_or(u64::MAX);
    let reste = ticks % SECONDE_US;
    // `reste < SECONDE_US`, donc la fraction tient sur 16 bits.
    let fraction = u16::try_from(reste.saturating_mul(65_536) / SECONDE_US).unwrap_or(u16::MAX);
    Instant {
        pas: Pas::du_fil(pas),
        fraction,
    }
}

/// Le début du pas `p`, en microsecondes de la partie, arrondi au-dessous.
pub fn debut_du_pas(p: Pas) -> u64 {
    let us = u128::from(p.numero())
        .saturating_mul(SECONDE_US)
        .checked_div(u128::from(PAS_PAR_SECONDE))
        .unwrap_or(0);
    u64::try_from(us).unwrap_or(u64::MAX)
}

/// Une durée en microsecondes, arrondie au pas supérieur : une demi-latence ne se
/// réduit jamais en se convertissant.
pub fn duree_de_us(us: u64) -> Duree {
    let ticks = u128::from(us).saturating_mul(u128::from(PAS_PAR_SECONDE));
    Duree::pas(u64::try_from(ticks.div_ceil(SECONDE_US)).unwrap_or(u64::MAX))
}

/// Une durée en microsecondes, depuis des pas.
pub fn us_de_duree(d: Duree) -> u64 {
    debut_du_pas(Pas::du_fil(d.en_pas()))
}

/// Le temps de la partie sur l'horloge de l'ordinateur de la soirée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Horloge {
    /// L'heure du pas 0, en microsecondes de l'horloge de l'ordinateur.
    origine_us: u64,
}

impl Horloge {
    /// L'horloge d'une partie ouverte : `ouverture.pas` commence à `ouverture.heure`
    /// (millisecondes). La même ouverture donne la même horloge, à chaque reprise.
    pub fn de(ouverture: &Ouverture) -> Horloge {
        let heure_us = ouverture.heure.saturating_mul(1000);
        Horloge {
            origine_us: heure_us.saturating_sub(debut_du_pas(Pas::du_fil(ouverture.pas))),
        }
    }

    /// L'heure de l'ordinateur, en microsecondes, d'un instant de la partie (`partie_us`
    /// depuis le pas 0) : l'inverse de [`Horloge::partie_us`]. La trace d'un geste (lot 13)
    /// vient du téléphone dans le temps de la partie ; l'autorité la ramène ainsi à son
    /// heure.
    pub const fn heure_us(&self, partie_us: u64) -> u64 {
        self.origine_us.saturating_add(partie_us)
    }

    /// Le temps de la partie, en microsecondes depuis le pas 0, à l'heure `t_us` de
    /// l'ordinateur. Avant le pas 0, zéro.
    pub const fn partie_us(&self, t_us: u64) -> u64 {
        t_us.saturating_sub(self.origine_us)
    }

    /// L'instant de la partie à l'heure `t_us`.
    pub fn instant(&self, t_us: u64) -> Instant {
        instant_de_la_partie(self.partie_us(t_us))
    }

    /// Le pas que l'horloge a atteint à l'heure `t_us` : la cible de `avancer`.
    pub fn pas(&self, t_us: u64) -> Pas {
        self.instant(t_us).pas
    }

    /// L'heure de l'ordinateur au début du pas `p`.
    pub fn heure_du_pas(&self, p: Pas) -> u64 {
        self.origine_us.saturating_add(debut_du_pas(p))
    }
}

/// Un aller-retour : l'heure de l'autre côté, prise au milieu, et sa durée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Echantillon {
    /// L'aller-retour, en microsecondes.
    aller_retour_us: u64,
    /// L'écart de l'horloge de l'autre côté sur la sienne, en microsecondes, au milieu
    /// de l'aller-retour.
    ecart_us: i128,
}

/// Les derniers échantillons, et le meilleur parmi eux.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Echantillons {
    /// Du plus ancien au plus récent, au plus [`ECHOS_RETENUS`].
    derniers: std::collections::VecDeque<Echantillon>,
}

impl Echantillons {
    fn noter(&mut self, e: Echantillon) {
        if self.derniers.len() >= ECHOS_RETENUS {
            let _: Option<Echantillon> = self.derniers.pop_front();
        }
        self.derniers.push_back(e);
    }

    /// Le plus petit aller-retour retenu ; le plus récent à égalité.
    fn meilleur(&self) -> Option<Echantillon> {
        self.derniers
            .iter()
            .rev()
            .min_by_key(|e| e.aller_retour_us)
            .copied()
    }
}

/// Côté appareil : l'heure de l'autorité (le temps de la partie), estimée par écho et
/// gardée au plus petit aller-retour des [`ECHOS_RETENUS`] derniers (SAL-127, SAL-128).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Estimation {
    echos: Echantillons,
}

impl Estimation {
    /// Un écho est revenu : envoyé à `envoi_us` et reçu à `recu_us` (horloge de
    /// l'appareil), il porte `autorite_us`, le temps de la partie quand l'autorité y a
    /// répondu. Un écho revenu avant d'être parti ne compte pas.
    pub fn noter(&mut self, envoi_us: u64, autorite_us: u64, recu_us: u64) {
        let Some(aller_retour_us) = recu_us.checked_sub(envoi_us) else {
            return;
        };
        let milieu = i128::from(envoi_us).saturating_add(i128::from(aller_retour_us / 2));
        self.echos.noter(Echantillon {
            aller_retour_us,
            ecart_us: i128::from(autorite_us).saturating_sub(milieu),
        });
    }

    /// Le temps de la partie à l'heure `t_us` de l'appareil, si un écho est revenu.
    pub fn autorite_us(&self, t_us: u64) -> Option<u64> {
        let e = self.echos.meilleur()?;
        let v = i128::from(t_us).saturating_add(e.ecart_us).max(0);
        Some(u64::try_from(v).unwrap_or(u64::MAX))
    }

    /// L'incertitude de l'estimation : un demi-aller-retour, en microsecondes.
    pub fn incertitude_us(&self) -> Option<u64> {
        self.echos.meilleur().map(|e| e.aller_retour_us / 2)
    }
}

/// Côté autorité : l'aller-retour vers un appareil, mesuré par l'autorité elle-même
/// (SAL-129), au plus petit des [`ECHOS_RETENUS`] derniers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AllerRetour {
    echos: Echantillons,
}

impl AllerRetour {
    /// L'appareil a répondu, à `recu_us`, à l'écho que l'autorité avait envoyé à
    /// `envoi_us` (même horloge, celle de l'autorité).
    pub fn noter(&mut self, envoi_us: u64, recu_us: u64) {
        if let Some(aller_retour_us) = recu_us.checked_sub(envoi_us) {
            self.echos.noter(Echantillon {
                aller_retour_us,
                ecart_us: 0,
            });
        }
    }

    /// La demi-latence à donner à la partie, arrondie au pas supérieur ; zéro tant
    /// qu'aucun écho n'est revenu.
    pub fn demi(&self) -> Duree {
        self.echos
            .meilleur()
            .map_or(Duree::ZERO, |e| duree_de_us(e.aller_retour_us / 2))
    }
}
