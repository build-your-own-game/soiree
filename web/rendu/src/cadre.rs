//! Le cadre : où la scène se pose sur la toile, et à quelle échelle (PHASE-3, § 2.9).
//!
//! - `FormeEcran::Scene` : le repère tient dans la zone sûre (HIS-46, PUN-51), centrée,
//!   à la plus grande échelle qui y tient ; avec `echelle_entiere`, un facteur entier
//!   dès qu'il vaut au moins 1 (SQ-52). Le fond de la scène couvre toute la toile.
//! - `FormeEcran::Page` (Tier list) : une unité du repère vaut `echelle_page` pixels (le
//!   rapport de pixels de l'appareil), le contenu au plus `largeur_max` unités de large,
//!   centré ; `defilement` unités sont passées en haut (le défilement est celui du
//!   navigateur, lot 11).

use jeu_abi::{FormeEcran, Oui};

/// Une transformation du repère de la scène vers les pixels : `p_px = p * echelle +
/// (x, y)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine {
    /// Des pixels par unité.
    pub echelle: f32,
    /// Le décalage horizontal, en pixels.
    pub x: f32,
    /// Le décalage vertical, en pixels.
    pub y: f32,
}

impl Affine {
    /// L'identité.
    pub const IDENTITE: Affine = Affine {
        echelle: 1.0,
        x: 0.0,
        y: 0.0,
    };

    /// Un point du repère, en pixels.
    pub fn point(&self, x: f32, y: f32) -> (f32, f32) {
        (x * self.echelle + self.x, y * self.echelle + self.y)
    }

    /// `self` après `autre` : `self(autre(p))`.
    #[must_use]
    pub fn apres(&self, autre: &Affine) -> Affine {
        Affine {
            echelle: self.echelle * autre.echelle,
            x: self.x + autre.x * self.echelle,
            y: self.y + autre.y * self.echelle,
        }
    }

    /// Une translation de `(x, y)` unités, puis `self`.
    #[must_use]
    pub fn decale(&self, x: f32, y: f32) -> Affine {
        self.apres(&Affine { echelle: 1.0, x, y })
    }
}

/// La toile et la forme de l'écran du jeu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cadre {
    /// La largeur de la toile, en pixels.
    pub largeur: u32,
    /// Sa hauteur, en pixels.
    pub hauteur: u32,
    /// La forme que le jeu déclare.
    pub forme: FormeEcran,
    /// Pour `FormeEcran::Page` : des pixels par unité (le rapport de pixels de
    /// l'appareil).
    pub echelle_page: f32,
    /// Pour `FormeEcran::Page` : les unités défilées en haut.
    pub defilement: f32,
}

impl Cadre {
    /// Un cadre de `largeur` × `hauteur` pixels, une unité par pixel en page, sans
    /// défilement.
    pub const fn new(largeur: u32, hauteur: u32, forme: FormeEcran) -> Cadre {
        Cadre {
            largeur,
            hauteur,
            forme,
            echelle_page: 1.0,
            defilement: 0.0,
        }
    }

    /// La transformation du repère de la scène vers les pixels de la toile.
    pub fn transformation(&self) -> Affine {
        let (lt, ht) = (self.largeur as f32, self.hauteur as f32);
        match self.forme {
            FormeEcran::Scene {
                largeur,
                hauteur,
                zone_sure_pour_cent,
                echelle_entiere,
            } => {
                let (l, h) = (f32::from(largeur.max(1)), f32::from(hauteur.max(1)));
                let part = f32::from(zone_sure_pour_cent.min(100)) / 100.0;
                let (lz, hz) = (lt * part, ht * part);
                let mut e = plus_petit(lz / l, hz / h);
                if echelle_entiere == Oui::Oui && e >= 1.0 {
                    e = e.floor();
                }
                Affine {
                    echelle: e,
                    x: ((lt - l * e) / 2.0).round(),
                    y: ((ht - h * e) / 2.0).round(),
                }
            }
            FormeEcran::Page { largeur_max } => {
                let e = if self.echelle_page > 0.0 {
                    self.echelle_page
                } else {
                    1.0
                };
                let contenu = plus_petit(lt / e, f32::from(largeur_max));
                Affine {
                    echelle: e,
                    x: ((lt - contenu * e) / 2.0).round(),
                    y: (-self.defilement * e).round(),
                }
            }
        }
    }
}

/// Le plus petit de deux flottants, écrit à la main (`clippy.toml` : `f32::min` ne
/// traite pas le zéro signé pareil en natif et dans WebAssembly).
pub(crate) fn plus_petit(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

/// Le plus grand de deux flottants, écrit à la main (même raison).
pub(crate) fn plus_grand(a: f32, b: f32) -> f32 {
    if b > a { b } else { a }
}
