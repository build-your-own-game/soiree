//! Ce que vise une entrée de la télé (lot 11), sans navigateur : le bouton sous un clic,
//! la fermeture d'une modale, le raccourci d'une touche. Seul ce que la scène porte
//! s'envoie (SAL-152) ; le cœur le vérifie encore (`admise_par_la_scene`).
//!
//! - **Un clic** vise le dernier bouton, dans l'ordre du peintre, dont la boîte contient
//!   le point. Une modale ouverte (le dernier composant `Modale`) ne laisse viser que ce
//!   qui vient après elle ; un clic hors de son panneau la ferme (son action `fermer`).
//! - **Une touche** vise un raccourci de la scène : même touche, mêmes modificateurs ;
//!   `CMD_OU_CTRL` (Ctrl et Cmd à `Oui`) veut dire Cmd sur Mac, Ctrl ailleurs ; une
//!   répétition automatique seulement si le raccourci la veut. Un raccourci cède au champ
//!   qui a le focus, et aucune touche n'en est un pendant une composition (TL-32,
//!   TL-34) : la page ne demande rien ici dans ces cas.

use jeu_abi::{ComposantVuFil, Mods, Oui, RaccourciFil, SceneFil, SorteNoeud};
use rendu::{Boite, Cadre, Contexte};

fn contient(b: &Boite, (x, y): (f32, f32)) -> bool {
    x >= b[0] && x < b[2] && y >= b[1] && y < b[3]
}

/// La modale ouverte : son indice dans la scène, son panneau en pixels, son action de
/// fermeture.
#[derive(Debug, Clone, PartialEq)]
pub struct Modale {
    /// L'indice de son nœud.
    pub indice: usize,
    /// Son panneau, en pixels de la toile.
    pub panneau: Boite,
    /// L'action envoyée à sa fermeture, encodée.
    pub fermer: Vec<u8>,
}

/// La modale ouverte de la scène, s'il y en a une : la dernière.
pub fn modale(scene: &SceneFil, cadre: &Cadre, contexte: &Contexte) -> Option<Modale> {
    scene
        .noeuds
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, n)| match &n.sorte {
            SorteNoeud::Composant {
                composant: ComposantVuFil::Modale { fermer, panneau },
                ..
            } => {
                let a = rendu::transformation_des_enfants(scene, i, cadre, contexte);
                Some(Modale {
                    indice: i,
                    panneau: rendu::boite_en_pixels(&a, *panneau),
                    fermer: fermer.clone(),
                })
            }
            SorteNoeud::Rect { .. }
            | SorteNoeud::Texte { .. }
            | SorteNoeud::Image { .. }
            | SorteNoeud::Groupe { .. }
            | SorteNoeud::Bouton { .. }
            | SorteNoeud::CompteARebours { .. }
            | SorteNoeud::Qr { .. }
            | SorteNoeud::Composant { .. }
            | SorteNoeud::Avertissements { .. }
            | SorteNoeud::Surface { .. }
            | SorteNoeud::Definitif => None,
        })
}

/// L'action d'un clic en `p` (pixels de la toile, comme les boîtes) : celle du dernier
/// bouton qui le contient (après la modale, s'il y en a une), ou la fermeture de la modale
/// pour un clic hors de son panneau ; `None` : rien.
pub fn clic<'s>(
    scene: &'s SceneFil,
    boites: &[Option<Boite>],
    modale: Option<&'s Modale>,
    p: (f32, f32),
) -> Option<&'s [u8]> {
    let debut = modale.map_or(0, |m| m.indice.saturating_add(1));
    let bouton = scene
        .noeuds
        .iter()
        .zip(boites)
        .enumerate()
        .skip(debut)
        .rev()
        .find_map(|(_, (n, b))| match (&n.sorte, b) {
            (SorteNoeud::Bouton { action, .. }, Some(b)) if contient(b, p) => {
                Some(action.as_slice())
            }
            _ => None,
        });
    match (bouton, modale) {
        (Some(a), _) => Some(a),
        (None, Some(m)) if !contient(&m.panneau, p) => Some(m.fermer.as_slice()),
        (None, Some(_) | None) => None,
    }
}

/// Le raccourci que vise une touche enfoncée (`code` : `KeyboardEvent.code` ; `mods` :
/// les modificateurs tenus, tels que le navigateur les donne) ; `mac` : la machine de la
/// soirée est un Mac (Cmd y tient lieu de Ctrl).
pub fn raccourci<'s>(
    raccourcis: &'s [RaccourciFil],
    code: &str,
    mods: Mods,
    repetition: Oui,
    mac: Oui,
) -> Option<&'s [u8]> {
    raccourcis
        .iter()
        .find(|r| {
            let m = r.mods;
            let cmd_ou_ctrl = m.ctrl == Oui::Oui && m.meta == Oui::Oui;
            let (ctrl, meta) = match (cmd_ou_ctrl, mac) {
                (true, Oui::Oui) => (Oui::Non, Oui::Oui),
                (true, Oui::Non) => (Oui::Oui, Oui::Non),
                (false, _) => (m.ctrl, m.meta),
            };
            r.touche.as_str() == code
                && m.maj == mods.maj
                && m.alt == mods.alt
                && ctrl == mods.ctrl
                && meta == mods.meta
                && (repetition == Oui::Non || r.repetition == Oui::Oui)
        })
        .map(|r| r.action.as_slice())
}
