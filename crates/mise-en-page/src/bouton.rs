//! Le nom d'un bouton, que le commun dessine (PHASE-3, § 2.9, lot 9) : un `Bouton` de la
//! scène ne porte que sa boîte, son nom et son action ; son allure est celle du commun,
//! la même pour tous les jeux. Le rendu de la télé dessine le nom dans ce style et
//! cette boîte ; E16 pourra les mesurer de même (aujourd'hui, il ne mesure pas les noms
//! des boutons : PHASE-3, § 2.9).

use jeu_abi::{Alignement, Cle, Coupe, Fixe, Lignes, Rect, Rgba, StyleTexte, Taille};

/// Le fond d'un bouton (choisi : un gris très sombre, opaque, lisible sur tous les
/// fonds des trois jeux avec un nom blanc).
pub const FOND_DU_BOUTON: Rgba = Rgba(24, 24, 28, 255);

/// La couleur du nom d'un bouton (choisi).
pub const COULEUR_DU_NOM: Rgba = Rgba::BLANC;

/// La marge autour du nom, en cinquièmes de la hauteur du bouton, de chaque côté
/// (choisi).
const MARGE_EN_CINQUIEMES: i32 = 1;

/// La boîte du nom dans un bouton : la boîte du bouton, moins une marge d'un cinquième
/// de sa hauteur de chaque côté.
pub fn boite_du_nom(boite: Rect) -> Rect {
    let marge = boite.h.brut().saturating_mul(MARGE_EN_CINQUIEMES) / 5;
    let deux = marge.saturating_mul(2);
    Rect {
        x: Fixe::depuis_brut(boite.x.brut().saturating_add(marge)),
        y: Fixe::depuis_brut(boite.y.brut().saturating_add(marge)),
        l: Fixe::depuis_brut(boite.l.brut().saturating_sub(deux).max(0)),
        h: Fixe::depuis_brut(boite.h.brut().saturating_sub(deux).max(0)),
    }
}

/// Le style du nom d'un bouton de cette boîte : la police du commun, au centre, sur
/// deux lignes au plus, coupé entre les mots, à la plus grande taille qui tient entre
/// une ligne pleine de la boîte du nom et le quart de celle-ci (au moins 1/256).
pub fn style_du_nom(boite: Rect) -> StyleTexte {
    let interieur = boite_du_nom(boite).h.brut();
    // Une ligne fait 1,2 fois la taille : la plus grande taille d'une ligne est 5/6 de
    // la hauteur.
    let max = (interieur.saturating_mul(5) / 6).max(1);
    let min = (max / 4).max(1);
    StyleTexte {
        police: Cle::POLICE_DU_COMMUN,
        taille: Taille::Budget {
            max: Fixe::depuis_brut(max),
            min: Fixe::depuis_brut(min),
        },
        couleur: COULEUR_DU_NOM,
        alignement: Alignement::Centre,
        lignes: Lignes::AuPlus(2),
        coupe: Coupe::Mots,
    }
}
