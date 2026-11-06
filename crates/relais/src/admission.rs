//! L'admission d'une connexion, décidée **avant de créer son fil** (SAL-143), avec des
//! places que le réseau ne peut pas prendre, réservées à la boucle locale : une rafale de
//! connexions venues du Wi-Fi n'empêche pas l'ordinateur de la soirée de joindre son
//! propre relais. Les nouvelles connexions du réseau sont aussi limitées par seconde
//! (SAL-78). Sans entrées-sorties : le temps entre en paramètre.

/// Les connexions ouvertes au plus : 256 (choisi). Sidequest déclare 71 appareils, et
/// chaque lien tient une connexion ; la page de démarrage en prend quelques-unes, brèves.
pub const CONNEXIONS_MAX: usize = 256;

/// Les places que seule la boucle locale prend : 16 (choisi) ; l'écran, son Worker et sa
/// garde n'en demandent que trois.
pub const RESERVE_BOUCLE: usize = 16;

/// Les nouvelles connexions du réseau, par seconde, en régime : 64 (choisi).
pub const NOUVELLES_PAR_SECONDE: u64 = 64;

/// La rafale de nouvelles connexions du réseau admise d'un coup : 128 (choisi), pour que
/// 71 téléphones qui reviennent ensemble après un relais relancé passent.
pub const RAFALE: u64 = 128;

/// D'où vient une connexion, pour l'admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pair {
    /// La boucle locale.
    Boucle,
    /// Le réseau.
    Reseau,
}

/// Pourquoi une connexion n'est pas admise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusAdmission {
    /// Toutes les places qu'elle pouvait prendre sont prises.
    Pleine,
    /// Trop de nouvelles connexions du réseau en trop peu de temps (SAL-78).
    TropVite,
}

/// Les places du relais.
#[derive(Debug, Clone)]
pub struct Admission {
    boucle: usize,
    reseau: usize,
    /// Les jetons de nouvelles connexions du réseau, en millièmes.
    jetons_milli: u64,
    dernier_us: u64,
}

impl Default for Admission {
    fn default() -> Self {
        Admission::new()
    }
}

impl Admission {
    /// Aucune connexion, la rafale entière disponible.
    pub const fn new() -> Admission {
        Admission {
            boucle: 0,
            reseau: 0,
            jetons_milli: RAFALE.saturating_mul(1000),
            dernier_us: 0,
        }
    }

    /// Les connexions ouvertes.
    pub const fn ouvertes(&self) -> usize {
        self.boucle.saturating_add(self.reseau)
    }

    /// Admet une connexion, ou dit pourquoi non. Admise, elle prend une place jusqu'à
    /// [`Admission::liberer`].
    pub fn admettre(&mut self, t_us: u64, pair: Pair) -> Result<(), RefusAdmission> {
        let ouvertes = self.ouvertes();
        match pair {
            Pair::Boucle => {
                if ouvertes >= CONNEXIONS_MAX {
                    return Err(RefusAdmission::Pleine);
                }
                self.boucle = self.boucle.saturating_add(1);
            }
            Pair::Reseau => {
                if ouvertes >= CONNEXIONS_MAX.saturating_sub(RESERVE_BOUCLE) {
                    return Err(RefusAdmission::Pleine);
                }
                let ecoule = t_us.saturating_sub(self.dernier_us);
                self.dernier_us = t_us.max(self.dernier_us);
                // `NOUVELLES_PAR_SECONDE` jetons par seconde, en millièmes : une
                // microseconde en rapporte `NOUVELLES_PAR_SECONDE / 1000` millièmes.
                let gagnes = ecoule.saturating_mul(NOUVELLES_PAR_SECONDE) / 1000;
                self.jetons_milli = self
                    .jetons_milli
                    .saturating_add(gagnes)
                    .min(RAFALE.saturating_mul(1000));
                if self.jetons_milli < 1000 {
                    return Err(RefusAdmission::TropVite);
                }
                self.jetons_milli = self.jetons_milli.saturating_sub(1000);
                self.reseau = self.reseau.saturating_add(1);
            }
        }
        Ok(())
    }

    /// Une connexion admise se ferme : sa place se libère.
    pub const fn liberer(&mut self, pair: Pair) {
        match pair {
            Pair::Boucle => self.boucle = self.boucle.saturating_sub(1),
            Pair::Reseau => self.reseau = self.reseau.saturating_sub(1),
        }
    }
}
