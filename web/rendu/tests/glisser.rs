//! Glisser entre deux pas (PHASE-3, § 2.9 ; lot 13) : seuls les nœuds qui glissent, sous
//! la même clé et de la même sorte, vont de la scène précédente à la dernière ; tout le
//! reste, et la dernière scène à la fin du chemin, sont ceux de la dernière (SQ-56, SQ-58 ;
//! TL-23 pour la rupture).

use jeu_abi::{Cle, Fixe, NoeudFil, Oui, Rect, Rgba, SceneFil, SorteNoeud};
use rendu::glisser::{PLEIN, a_des_noeuds_qui_glissent, deplace_des_noeuds, entre};

fn rect(x: i32, y: i32) -> Rect {
    Rect {
        x: Fixe::unites(x),
        y: Fixe::unites(y),
        l: Fixe::unites(10),
        h: Fixe::unites(10),
    }
}

fn noeud(cle: u64, sorte: SorteNoeud, glisse: Oui, rupture: Oui) -> NoeudFil {
    NoeudFil {
        glisse,
        rupture,
        ..NoeudFil::simple(Cle::du_fil(cle), None, sorte)
    }
}

fn plein(r: Rect, couleur: Rgba) -> SorteNoeud {
    SorteNoeud::Rect { r, couleur }
}

fn scene(noeuds: Vec<NoeudFil>) -> SceneFil {
    let mut s = SceneFil::vide();
    s.noeuds = noeuds;
    s
}

/// La position d'un nœud (le coin de son rectangle, ou le décalage d'un groupe) ;
/// `None` pour une autre sorte.
fn position(s: &SceneFil, k: usize) -> Option<(i32, i32)> {
    match &s.noeuds.get(k)?.sorte {
        SorteNoeud::Rect { r, .. } | SorteNoeud::Image { r, .. } | SorteNoeud::Surface { r } => {
            Some((r.x.brut(), r.y.brut()))
        }
        SorteNoeud::Groupe { decalage } => Some((decalage.0.brut(), decalage.1.brut())),
        SorteNoeud::Texte { .. }
        | SorteNoeud::Bouton { .. }
        | SorteNoeud::CompteARebours { .. }
        | SorteNoeud::Qr { .. }
        | SorteNoeud::Composant { .. }
        | SorteNoeud::Avertissements { .. }
        | SorteNoeud::Definitif => None,
    }
}

#[test]
fn un_noeud_qui_glisse_va_a_mi_chemin_et_garde_le_reste_de_la_derniere() {
    let blanc = Rgba(255, 255, 255, 255);
    let rouge = Rgba(255, 0, 0, 255);
    let avant = scene(vec![noeud(1, plein(rect(0, 0), blanc), Oui::Oui, Oui::Non)]);
    let apres = scene(vec![noeud(
        1,
        plein(rect(100, 40), rouge),
        Oui::Oui,
        Oui::Non,
    )]);
    let u = Fixe::PAR_UNITE;
    assert_eq!(position(&entre(&avant, &apres, 0), 0), Some((0, 0)));
    assert_eq!(
        position(&entre(&avant, &apres, 500), 0),
        Some((50 * u, 20 * u))
    );
    assert_eq!(
        position(&entre(&avant, &apres, 250), 0),
        Some((25 * u, 10 * u))
    );
    assert_eq!(entre(&avant, &apres, PLEIN), apres);
    assert_eq!(entre(&avant, &apres, 5_000), apres);
    let SorteNoeud::Rect { couleur, r } = &entre(&avant, &apres, 500).noeuds[0].sorte else {
        panic!()
    };
    assert_eq!(
        (*couleur, r.l, r.h),
        (rouge, Fixe::unites(10), Fixe::unites(10))
    );
}

#[test]
fn rupture_cle_absente_ou_sorte_changee_ne_glissent_pas() {
    let b = Rgba(255, 255, 255, 255);
    let avant = scene(vec![
        noeud(1, plein(rect(0, 0), b), Oui::Oui, Oui::Non),
        noeud(2, plein(rect(0, 0), b), Oui::Oui, Oui::Non),
        noeud(4, plein(rect(0, 0), b), Oui::Non, Oui::Non),
        noeud(5, plein(rect(0, 0), b), Oui::Oui, Oui::Non),
    ]);
    let apres = scene(vec![
        // Rupture à ce pas (TL-23).
        noeud(1, plein(rect(80, 80), b), Oui::Oui, Oui::Oui),
        // Devenu une surface : une autre sorte.
        noeud(
            2,
            SorteNoeud::Surface { r: rect(80, 80) },
            Oui::Oui,
            Oui::Non,
        ),
        // Absent avant.
        noeud(3, plein(rect(80, 80), b), Oui::Oui, Oui::Non),
        // Ne glisse pas.
        noeud(4, plein(rect(80, 80), b), Oui::Non, Oui::Non),
        // Un rectangle devenu un groupe : une autre sorte, il ne glisse pas (corrigé au lot
        // 13b, le commentaire disait que son décalage glisse ; le décalage d'un groupe qui
        // l'était déjà glisse : `le_decalage_d_un_groupe_glisse`).
        noeud(
            5,
            SorteNoeud::Groupe {
                decalage: (Fixe::unites(80), Fixe::unites(80)),
            },
            Oui::Oui,
            Oui::Non,
        ),
    ]);
    let m = entre(&avant, &apres, 500);
    let u = Fixe::PAR_UNITE;
    for k in 0..4 {
        assert_eq!(position(&m, k), Some((80 * u, 80 * u)), "nœud {k}");
    }
    // Le nœud 5 : rectangle avant, groupe après, des sortes différentes ; il est déjà à
    // la place de la dernière scène.
    assert_eq!(position(&m, 4), Some((80 * u, 80 * u)));
    assert!(a_des_noeuds_qui_glissent(&apres));
    assert!(!a_des_noeuds_qui_glissent(&scene(vec![noeud(
        9,
        plein(rect(0, 0), b),
        Oui::Non,
        Oui::Non
    )])));
}

#[test]
fn le_decalage_d_un_groupe_glisse() {
    let g = |x: i32| SorteNoeud::Groupe {
        decalage: (Fixe::unites(x), Fixe::unites(-x)),
    };
    let avant = scene(vec![noeud(7, g(0), Oui::Oui, Oui::Non)]);
    let apres = scene(vec![noeud(7, g(60), Oui::Oui, Oui::Non)]);
    let u = Fixe::PAR_UNITE;
    assert_eq!(
        position(&entre(&avant, &apres, 500), 0),
        Some((30 * u, -30 * u))
    );
}

/// Corrigé après la vérification du lot 13 : glisser ne retarde que les scènes où un nœud
/// se déplace. Sans nœud qui glisse (ardoise, Punchline), ou si celui qui glisse n'a pas
/// bougé, a changé de sorte, vient d'apparaître ou rompt (TL-23), la page dessine la
/// dernière scène en entier tout de suite.
#[test]
fn glisser_ne_retarde_que_les_scenes_ou_un_noeud_se_deplace() {
    let b = Rgba(255, 255, 255, 255);
    let r = Rgba(255, 0, 0, 255);
    let un =
        |cle, x, glisse, rupture| scene(vec![noeud(cle, plein(rect(x, 0), b), glisse, rupture)]);
    // Il se déplace : glisser change quelque chose.
    assert!(deplace_des_noeuds(
        &un(1, 0, Oui::Oui, Oui::Non),
        &un(1, 30, Oui::Oui, Oui::Non)
    ));
    // Aucun nœud qui glisse, même s'il bouge.
    assert!(!deplace_des_noeuds(
        &un(1, 0, Oui::Non, Oui::Non),
        &un(1, 30, Oui::Non, Oui::Non)
    ));
    // Il glisse, mais n'a pas bougé (sa couleur change seulement).
    assert!(!deplace_des_noeuds(
        &un(1, 30, Oui::Oui, Oui::Non),
        &scene(vec![noeud(1, plein(rect(30, 0), r), Oui::Oui, Oui::Non)])
    ));
    // Il rompt à ce pas (TL-23).
    assert!(!deplace_des_noeuds(
        &un(1, 0, Oui::Oui, Oui::Non),
        &un(1, 30, Oui::Oui, Oui::Oui)
    ));
    // Il vient d'apparaître (une autre clé avant).
    assert!(!deplace_des_noeuds(
        &un(2, 0, Oui::Oui, Oui::Non),
        &un(1, 30, Oui::Oui, Oui::Non)
    ));
    // Il a changé de sorte.
    assert!(!deplace_des_noeuds(
        &scene(vec![noeud(
            1,
            SorteNoeud::Surface { r: rect(0, 0) },
            Oui::Oui,
            Oui::Non
        )]),
        &un(1, 30, Oui::Oui, Oui::Non)
    ));
    // Et chaque fois que rien ne se déplace, le chemin est la dernière scène à chaque
    // image : la dessiner en entier tout de suite ne montre rien d'autre.
    let (a, d) = (un(1, 30, Oui::Oui, Oui::Non), un(1, 30, Oui::Oui, Oui::Non));
    assert!(!deplace_des_noeuds(&a, &d));
    assert_eq!(entre(&a, &d, 0), d);
}
