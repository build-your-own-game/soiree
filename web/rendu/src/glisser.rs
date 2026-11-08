//! Glisser entre deux pas (PHASE-3, § 2.9 ; lot 13) : la télé dessine, entre la scène
//! précédente et la dernière, les nœuds qui glissent à mi-chemin (SQ-56, SQ-58).
//!
//! Un nœud glisse s'il porte `glisse` sans `rupture` (TL-23), s'il a la même clé dans les
//! deux scènes, et s'il y est de la même sorte : sa position (le coin du rectangle d'un
//! `Rect`, d'une `Image`, d'une `Surface`, de la boîte d'un texte, d'un bouton, d'un
//! compte à rebours, d'un QR, des avertissements ; le décalage d'un groupe) va de la
//! précédente à la dernière, à la fraction voulue. Tout le reste est celui de la dernière
//! scène : ses tailles (un texte ne se remet pas en page à chaque image), ses couleurs,
//! ses textes, sa structure, et les composants (de vrais éléments, que la page pose à
//! leur boîte sans les faire glisser).
//!
//! Glisser coûte un pas de retard aux nœuds qui glissent : la dernière scène n'est
//! dessinée en entier qu'une fois le chemin fini. La page ne glisse qu'à 100 Hz ou plus,
//! ou quand le banc le force (`?glisser=oui`), et seulement quand un nœud se déplace
//! ([`deplace_des_noeuds`]) ; à 60 Hz, elle dessine la dernière scène (§ 2.9). La fraction se calcule en millièmes : rien de ce qui est dessiné ne revient à
//! la partie.

use std::collections::BTreeMap;

use jeu_abi::{Fixe, NoeudFil, Oui, Rect, SceneFil, SorteNoeud};

/// Un millième de chemin : la fraction pleine.
pub const PLEIN: u32 = 1000;

/// D'une valeur à l'autre, à `f` millièmes du chemin (`f` borné à [`PLEIN`]).
fn entre_fixes(a: Fixe, b: Fixe, f: u32) -> Fixe {
    let f = i64::from(f.min(PLEIN));
    let (a, b) = (i64::from(a.brut()), i64::from(b.brut()));
    let v = a + (b - a) * f / i64::from(PLEIN);
    Fixe::depuis_brut(i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX }))
}

fn entre_rects(a: Rect, b: Rect, f: u32) -> Rect {
    Rect {
        x: entre_fixes(a.x, b.x, f),
        y: entre_fixes(a.y, b.y, f),
        ..b
    }
}

/// La sorte d'un nœud de la dernière scène, sa géométrie prise à `f` millièmes depuis
/// celle d'avant, si les deux sont de la même sorte ; `None` sinon.
fn sorte_entre(avant: &SorteNoeud, apres: &SorteNoeud, f: u32) -> Option<SorteNoeud> {
    let mut s = apres.clone();
    match (avant, &mut s) {
        (SorteNoeud::Rect { r: a, .. }, SorteNoeud::Rect { r: b, .. })
        | (SorteNoeud::Image { r: a, .. }, SorteNoeud::Image { r: b, .. })
        | (SorteNoeud::Surface { r: a }, SorteNoeud::Surface { r: b })
        | (SorteNoeud::Texte { boite: a, .. }, SorteNoeud::Texte { boite: b, .. })
        | (SorteNoeud::Bouton { boite: a, .. }, SorteNoeud::Bouton { boite: b, .. })
        | (
            SorteNoeud::CompteARebours { boite: a, .. },
            SorteNoeud::CompteARebours { boite: b, .. },
        )
        | (SorteNoeud::Qr { boite: a }, SorteNoeud::Qr { boite: b })
        | (SorteNoeud::Avertissements { boite: a }, SorteNoeud::Avertissements { boite: b }) => {
            *b = entre_rects(*a, *b, f);
        }
        (SorteNoeud::Groupe { decalage: a }, SorteNoeud::Groupe { decalage: b }) => {
            *b = (entre_fixes(a.0, b.0, f), entre_fixes(a.1, b.1, f));
        }
        (
            SorteNoeud::Rect { .. }
            | SorteNoeud::Image { .. }
            | SorteNoeud::Surface { .. }
            | SorteNoeud::Texte { .. }
            | SorteNoeud::Bouton { .. }
            | SorteNoeud::CompteARebours { .. }
            | SorteNoeud::Qr { .. }
            | SorteNoeud::Avertissements { .. }
            | SorteNoeud::Groupe { .. }
            | SorteNoeud::Composant { .. }
            | SorteNoeud::Definitif,
            _,
        ) => return None,
    }
    Some(s)
}

/// Le nœud glisse-t-il à ce pas ?
const fn glisse(n: &NoeudFil) -> bool {
    matches!((n.glisse, n.rupture), (Oui::Oui, Oui::Non))
}

/// La scène à dessiner à `f` millièmes du chemin de `precedente` à `derniere` ; à
/// [`PLEIN`] et au-delà, la dernière telle quelle.
pub fn entre(precedente: &SceneFil, derniere: &SceneFil, f: u32) -> SceneFil {
    let mut s = derniere.clone();
    if f >= PLEIN {
        return s;
    }
    let avant: BTreeMap<u64, &NoeudFil> = precedente
        .noeuds
        .iter()
        .filter(|n| glisse(n))
        .map(|n| (n.cle.fil(), n))
        .collect();
    for n in s.noeuds.iter_mut().filter(|n| glisse(n)) {
        if let Some(a) = avant.get(&n.cle.fil())
            && let Some(sorte) = sorte_entre(&a.sorte, &n.sorte, f)
        {
            n.sorte = sorte;
        }
    }
    s
}

/// Une scène a-t-elle un nœud qui glisse ? Sans lui, glisser ne change rien.
pub fn a_des_noeuds_qui_glissent(scene: &SceneFil) -> bool {
    scene.noeuds.iter().any(glisse)
}

/// Glisser de `precedente` à `derniere` déplace-t-il un nœud ? Il faut un nœud qui glisse
/// dans la dernière, sous la même clé et de la même sorte dans la précédente, à une autre
/// position. Sinon, chaque image du chemin serait la dernière scène telle quelle, et la
/// page la dessine en entier tout de suite : glisser ne retarde que les scènes où quelque
/// chose glisse vraiment (corrigé après la vérification du lot 13 : la page retardait le
/// pixel de toute scène, même sans nœud qui glisse, chez ardoise et Punchline).
pub fn deplace_des_noeuds(precedente: &SceneFil, derniere: &SceneFil) -> bool {
    if !a_des_noeuds_qui_glissent(derniere) {
        return false;
    }
    let avant: BTreeMap<u64, &NoeudFil> = precedente
        .noeuds
        .iter()
        .filter(|n| glisse(n))
        .map(|n| (n.cle.fil(), n))
        .collect();
    derniere.noeuds.iter().filter(|n| glisse(n)).any(|n| {
        avant
            .get(&n.cle.fil())
            .and_then(|a| sorte_entre(&a.sorte, &n.sorte, 0))
            .is_some_and(|depart| depart != n.sorte)
    })
}
