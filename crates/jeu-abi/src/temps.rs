//! Le temps (§ 2.4) : des numéros de pas, jamais une heure pour décider.
//!
//! `Maintenant`, `Echeance` et `Reveil` vivent dans `jeu` : ils ne voyagent pas,
//! leur forme sur le fil est un [`Pas`] ([`crate::ReveilFil`], [`crate::CompteAReboursFil`]).

use serde::{Deserialize, Serialize};

/// Le nombre de pas par seconde, le même pour tous les jeux (D13).
pub const PAS_PAR_SECONDE: u64 = 60;

/// Un instant passé de la partie, en pas. Se garde dans l'état. Ne s'additionne pas :
/// il n'existe ni `Pas + Duree` ni `Pas - Pas` pour un jeu (PUN-34).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Pas(u64);

impl Pas {
    /// Le premier pas d'une soirée.
    pub const ZERO: Pas = Pas(0);

    /// Le numéro du pas, pour l'afficher ou l'écrire au journal.
    pub const fn numero(self) -> u64 {
        self.0
    }

    /// Réservé au commun : un pas depuis son numéro.
    #[doc(hidden)]
    pub const fn du_fil(n: u64) -> Pas {
        Pas(n)
    }
}

/// Un instant à l'intérieur d'un pas, en 1/65 536 de pas, pour départager une
/// course serrée. Ordonné par pas, puis par fraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Instant {
    /// Le pas.
    pub pas: Pas,
    /// La fraction du pas, en 1/65 536.
    pub fraction: u16,
}

impl Instant {
    /// Le début d'un pas.
    pub const fn de(p: Pas) -> Instant {
        Instant {
            pas: p,
            fraction: 0,
        }
    }
}

/// Une durée en pas entiers. Jamais une somme de flottants (CAT-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Duree(u64);

impl Duree {
    /// Aucune durée.
    pub const ZERO: Duree = Duree(0);

    /// `n` pas.
    pub const fn pas(n: u64) -> Duree {
        Duree(n)
    }

    /// `n` millisecondes, arrondies au pas supérieur : 450 ms font 27 pas, 1 ms fait 1 pas.
    pub const fn ms(n: u64) -> Duree {
        let entieres = (n / 1000).saturating_mul(PAS_PAR_SECONDE);
        let reste = (n % 1000).saturating_mul(PAS_PAR_SECONDE).div_ceil(1000);
        Duree(entieres.saturating_add(reste))
    }

    /// `n` secondes.
    pub const fn s(n: u64) -> Duree {
        Duree(n.saturating_mul(PAS_PAR_SECONDE))
    }

    /// Le nombre de pas.
    pub const fn en_pas(self) -> u64 {
        self.0
    }

    /// La durée en millisecondes, arrondie à la milliseconde inférieure ; saturée.
    pub const fn en_ms(self) -> u64 {
        let entieres = (self.0 / PAS_PAR_SECONDE).saturating_mul(1000);
        let reste = (self.0 % PAS_PAR_SECONDE).saturating_mul(1000) / PAS_PAR_SECONDE;
        entieres.saturating_add(reste)
    }

    /// Les secondes restantes à afficher, arrondies au-dessus : 1 pas restant affiche 1 (SQ-35).
    pub const fn secondes_restantes(self) -> u64 {
        self.0.div_ceil(PAS_PAR_SECONDE)
    }

    /// La somme, saturée.
    #[must_use]
    pub const fn plus(self, d: Duree) -> Duree {
        Duree(self.0.saturating_add(d.0))
    }
}

/// L'heure UTC de l'ordinateur de la soirée, en millisecondes, enregistrée au journal.
/// Pour étiqueter une ligne de résultats, jamais pour décider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HeureMurale(u64);

impl HeureMurale {
    /// Les millisecondes depuis l'époque Unix.
    pub const fn ms(self) -> u64 {
        self.0
    }

    /// Réservé au commun : une heure lue sur l'horloge murale, ou relue au journal.
    #[doc(hidden)]
    pub const fn du_fil(ms: u64) -> HeureMurale {
        HeureMurale(ms)
    }
}
