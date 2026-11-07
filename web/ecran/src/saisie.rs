//! Les champs de texte de la télé (`Champ`, `EnPlace` ; TL-27 à TL-34 ; lot 11), sans
//! navigateur : ce que la page envoie d'une saisie, et quand elle réécrit un champ.
//!
//! - **La limite se compte dans l'unité du champ**, comme le cœur la compte
//!   (`partie::regles::longueur`, TL-27) ; la page envoie le plus long début qui tient,
//!   coupé **entre deux graphèmes** (TL-28) : une saisie au-delà serait refusée par le
//!   cœur (`admise_par_la_scene`). Le texte en trop reste visible dans le champ pendant
//!   la saisie, et disparaît quand la scène le réécrit, à la perte du focus (TL-30).
//! - **Le texte sous le curseur n'est jamais réécrit pendant la saisie** (TL-33, CAHIER
//!   de Tier list, § 7) : un champ qui a le focus ne prend la valeur de la scène que si
//!   le jeu l'a changée lui-même (elle diffère de la scène précédente et de chacun des
//!   textes que la page a envoyés et dont l'écho n'est pas encore revenu : le champ de
//!   nom vidé après « Add Text »), et jamais pendant une composition (TL-32). Les échos
//!   reviennent en retard sur la frappe : une frappe par touche en envoie plusieurs avant
//!   que la scène du premier n'arrive (constaté au lot 11, dans Firefox : « Ban » tapé
//!   lettre à lettre, l'écho de « B » réécrivait le champ et renvoyait le curseur au
//!   début).

use std::collections::VecDeque;

use jeu_abi::{Oui, Unite};
use unicode_segmentation::UnicodeSegmentation;

/// Le plus long début de `texte` dont la longueur, dans `unite`, tient dans `limite`,
/// coupé entre deux graphèmes (TL-28).
pub fn couper(texte: &str, unite: Unite, limite: u32) -> &str {
    let limite = u64::from(limite);
    if partie::regles::longueur(texte, unite) <= limite {
        return texte;
    }
    let mut fin = 0;
    for (debut, g) in texte.grapheme_indices(true) {
        let apres = debut.saturating_add(g.len());
        match texte.get(..apres) {
            Some(t) if partie::regles::longueur(t, unite) <= limite => fin = apres,
            Some(_) | None => break,
        }
    }
    texte.get(..fin).unwrap_or("")
}

/// Les textes envoyés dont la page attend encore l'écho, au plus (choisi) : au-delà, le
/// plus ancien s'oublie.
pub const ECHOS_ATTENDUS_MAX: usize = 64;

/// Ce que la page sait d'un champ : la valeur de la dernière scène, et les textes qu'elle
/// a envoyés dont l'écho n'est pas encore revenu, dans l'ordre.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Champ {
    /// La valeur que portait la dernière scène.
    pub de_la_scene: String,
    /// Les textes envoyés au jeu dont l'écho n'est pas revenu, du plus ancien au plus
    /// récent.
    pub envoyes: VecDeque<String>,
}

impl Champ {
    /// Une scène neuve porte `valeur` : faut-il réécrire le champ ? `focus` : il a le
    /// focus ; `composition` : une composition est en cours ; `affiche` : ce que le champ
    /// montre.
    pub fn reecrire(&mut self, valeur: &str, focus: Oui, composition: Oui, affiche: &str) -> bool {
        // Un écho : ce texte, et ceux envoyés avant lui, sont revenus (les entrées de
        // l'écran entrent dans l'ordre).
        let echo = self.envoyes.iter().position(|e| e == valeur);
        if let Some(k) = echo {
            let _: Vec<String> = self.envoyes.drain(..=k).collect();
        }
        let changee_par_le_jeu = valeur != self.de_la_scene && echo.is_none();
        valeur.clone_into(&mut self.de_la_scene);
        match (composition, focus) {
            (Oui::Oui, _) => false,
            (Oui::Non, Oui::Oui) => changee_par_le_jeu && valeur != affiche,
            (Oui::Non, Oui::Non) => valeur != affiche,
        }
    }

    /// Le texte à envoyer d'une saisie : coupé à la limite.
    pub fn envoyer<'t>(&mut self, texte: &'t str, unite: Unite, limite: u32) -> &'t str {
        let t = couper(texte, unite, limite);
        if self.envoyes.len() >= ECHOS_ATTENDUS_MAX {
            let _: Option<String> = self.envoyes.pop_front();
        }
        self.envoyes.push_back(String::from(t));
        t
    }
}
