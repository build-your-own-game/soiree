//! E18 : après un gel, au plus une transition de phase par entrée, et une phase
//! complète après la reprise (§ 7.2 ; PUN-34, D14, D15).
//!
//! La soirée de E18 (`Sorte::Gel`) gèle l'ordinateur de la soirée pendant une échéance
//! que la partie attend, jusqu'au-delà d'elle : au retour, le rattrapage livre quelques
//! pas, puis le numéro saute (§ 4.1). Le jeu qui part de `Maintenant` termine la phase
//! échue, et donne à la suivante toute sa durée. Celui qui chaîne son échéance sur
//! celle d'avant (`Echeance::repoussee`) voit la suivante échue aussitôt, et enchaîne
//! les phases (§ 11, « Ce qui reste aux pressions »).
//!
//! La sonde note la phase après chaque entrée acceptée. Deux constats, dans la fenêtre
//! qui suit chaque gel :
//!
//! - chaque transition de phase est une transition vue hors des gels : une transition
//!   qu'on ne voit qu'après un gel saute une phase, ce qui veut dire plus d'une
//!   transition en une seule entrée ;
//! - la phase où le gel fait entrer, quand un `Temps` la finit, dure sa durée
//!   ordinaire : la médiane des durées de la même phase hors des gels, finie de
//!   même, à [`TOLERANCE_POUR_CENT`] près. La médiane, et non la plus courte : une
//!   soirée principale, avec ses pannes, voit des phases écourtées, et la plus
//!   courte tomberait à quelques pas ; un seuil à la moitié laissait passer une phase
//!   raccourcie de trois secondes, le bogue même de PUN-34.
//!
//! La tolérance, et la fenêtre de dix secondes après un gel, sont des points de
//! départ, non mesurés.

use std::collections::{BTreeMap, BTreeSet};

use jeu_abi::Oui;

use crate::sonde::{Compte, PhaseVue};

/// La fenêtre qui suit un gel, en pas : dix secondes.
const APRES_UN_GEL: u64 = 600;

/// Ce qu'une phase peut perdre après un gel, en pour cent de sa durée ordinaire,
/// avant que E18 échoue : le rattrapage livre quelques pas, rien de plus.
pub(crate) const TOLERANCE_POUR_CENT: u64 = 10;

/// La médiane d'une suite non vide.
fn mediane(v: &mut [u64]) -> Option<u64> {
    v.sort_unstable();
    v.get(v.len() / 2).copied()
}

/// Une transition de phase : d'où, vers où, à quel pas, sur un `Temps` ou non.
#[derive(Debug, Clone, Copy)]
struct Transition {
    de: u16,
    vers: u16,
    place: u64,
    temps: Oui,
}

fn transitions(phases: &[PhaseVue]) -> Vec<Transition> {
    let mut v = Vec::new();
    let mut avant: Option<u16> = None;
    for p in phases {
        if let Some(a) = avant
            && a != p.phase
        {
            v.push(Transition {
                de: a,
                vers: p.phase,
                place: p.place,
                temps: p.temps,
            });
        }
        avant = Some(p.phase);
    }
    v
}

/// Les transitions d'une soirée qui suivent l'un de ses gels : les deux premières
/// à partir du pas où le temps reprend, par indice dans `t`.
fn apres_les_gels(t: &[Transition], gels: &[(u64, u64)]) -> BTreeSet<usize> {
    let mut v = BTreeSet::new();
    for (_, fin) in gels {
        for (k, _) in t
            .iter()
            .enumerate()
            .filter(|(_, x)| x.place >= *fin && x.place < fin.saturating_add(APRES_UN_GEL))
            .take(2)
        {
            let _: bool = v.insert(k);
        }
    }
    v
}

/// Une soirée, pour E18 : la phase après chaque entrée acceptée, et ses gels (le
/// premier pas, le pas où le temps reprend).
pub(crate) type SoireeGelee<'s> = (&'s [PhaseVue], &'s [(u64, u64)]);

/// Juge les gels de ces soirées : pour chacune, la phase après chaque entrée acceptée,
/// et ses gels (le premier pas, le pas où le temps reprend). Les transitions et les
/// durées de référence viennent de toutes, hors de ce qui suit un gel.
pub(crate) fn juger(soirees: &[SoireeGelee<'_>]) -> (Compte, Vec<String>) {
    let mut c = Compte::default();
    let mut notes = Vec::new();
    let mut normales: BTreeSet<(u16, u16)> = BTreeSet::new();
    // Les durées de chaque phase hors des gels, quand un `Temps` la finit.
    let mut durees: BTreeMap<u16, Vec<u64>> = BTreeMap::new();
    let parsees: Vec<(Vec<Transition>, BTreeSet<usize>)> = soirees
        .iter()
        .map(|(phases, gels)| {
            let t = transitions(phases);
            let apres = apres_les_gels(&t, gels);
            (t, apres)
        })
        .collect();
    for (t, apres) in &parsees {
        for (k, x) in t.iter().enumerate() {
            if !apres.contains(&k) {
                let _: bool = normales.insert((x.de, x.vers));
            }
        }
        for (k, paire) in t.windows(2).enumerate() {
            let [entree, sortie] = paire else { continue };
            if apres.contains(&k) || sortie.temps == Oui::Non {
                continue;
            }
            let d = sortie.place.saturating_sub(entree.place);
            durees.entry(entree.vers).or_default().push(d);
        }
    }
    let ordinaire: BTreeMap<u16, u64> = durees
        .into_iter()
        .filter_map(|(p, mut v)| mediane(&mut v).map(|m| (p, m)))
        .collect();
    for ((_, gels), (t, _)) in soirees.iter().zip(&parsees) {
        for (debut, fin) in *gels {
            c.evaluer();
            let dans: Vec<(usize, &Transition)> = t
                .iter()
                .enumerate()
                .filter(|(_, x)| x.place >= *fin && x.place < fin.saturating_add(APRES_UN_GEL))
                .take(2)
                .collect();
            for (_, x) in &dans {
                if !normales.contains(&(x.de, x.vers)) {
                    c.echouer(format!(
                        "après le gel des pas {debut} à {fin} : la phase {} passe à {} au pas {}, ce qu'elle ne fait jamais hors d'un gel (plus d'une transition en une entrée)",
                        x.de, x.vers, x.place
                    ));
                }
            }
            let Some((k, entree)) = dans.first() else {
                notes.push(format!(
                    "gel des pas {debut} à {fin} : aucune transition de phase dans les dix secondes qui suivent"
                ));
                continue;
            };
            let Some(sortie) = t.get(k.saturating_add(1)) else {
                continue;
            };
            if sortie.temps == Oui::Non {
                continue;
            }
            let d = sortie.place.saturating_sub(entree.place);
            let raccourcie = |m: u64| {
                d.saturating_mul(100) < m.saturating_mul(100u64.saturating_sub(TOLERANCE_POUR_CENT))
            };
            match ordinaire.get(&entree.vers) {
                Some(m) if raccourcie(*m) => c.echouer(format!(
                    "après le gel des pas {debut} à {fin} : la phase {} ne dure que {d} pas, contre {m} d'ordinaire hors des gels (PUN-34)",
                    entree.vers
                )),
                Some(m) => notes.push(format!(
                    "gel des pas {debut} à {fin} : la phase {} dure {d} pas après la reprise ({m} d'ordinaire hors des gels)",
                    entree.vers
                )),
                None => notes.push(format!(
                    "gel des pas {debut} à {fin} : la phase {} n'a pas de durée de référence hors des gels",
                    entree.vers
                )),
            }
        }
    }
    (c, notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(place: u64, phase: u16, temps: Oui) -> PhaseVue {
        PhaseVue {
            partie: 0,
            place,
            temps,
            saut: 0,
            phase,
        }
    }

    const T: Oui = Oui::Oui;
    const N: Oui = Oui::Non;

    /// Une phase qui reçoit toute sa durée après un gel passe ; une phase enchaînée
    /// aussitôt échoue (PUN-34).
    #[test]
    fn une_phase_enchainee_apres_un_gel_echoue() {
        // Hors du gel : 0 → 1 → 2 → 0, la phase 2 dure 120 pas.
        let mut v = vec![p(10, 0, N), p(20, 1, N), p(320, 2, T), p(440, 0, T)];
        // Un gel jusqu'au pas 1000 : 1 → 2 au retour, puis 2 → 0 au pas suivant.
        v.extend([p(700, 1, N), p(1000, 2, T), p(1001, 0, T)]);
        let (c, _) = juger(&[(&v, &[(800, 1000)])]);
        assert_eq!(c.evaluations, 1);
        assert_eq!(c.echecs, 1, "{:?}", c.details);
        // La même, saine : la phase 2 dure ses 120 pas.
        let mut v = v[..5].to_vec();
        v.extend([p(1000, 2, T), p(1120, 0, T)]);
        let (c, _) = juger(&[(&v, &[(800, 1000)])]);
        assert_eq!(c.echecs, 0, "{:?}", c.details);
    }

    /// PUN-34 : une phase de dix secondes raccourcie de trois secondes après un gel
    /// échoue, même quand une soirée à pannes a vu la même phase écourtée à quelques
    /// pas. La même phase, entière après le gel, passe.
    #[test]
    fn une_phase_raccourcie_de_trois_secondes_echoue() {
        // Hors des gels : la phase 2 dure 600 pas, deux fois, et 11 une fois.
        let hors = vec![
            p(10, 0, N),
            p(20, 1, N),
            p(100, 2, T),
            p(700, 0, T),
            p(710, 1, N),
            p(800, 2, T),
            p(1400, 0, T),
            p(1410, 1, N),
            p(1500, 2, T),
            p(1511, 0, T),
        ];
        let gelee = |duree: u64| {
            vec![
                p(10, 0, N),
                p(20, 1, N),
                p(3000, 2, T),
                p(3000 + duree, 0, T),
            ]
        };
        let raccourcie = gelee(420);
        let (c, _) = juger(&[(&hors, &[]), (&raccourcie, &[(2800, 3000)])]);
        assert_eq!(c.echecs, 1, "{:?}", c.details);
        let entiere = gelee(598);
        let (c, _) = juger(&[(&hors, &[]), (&entiere, &[(2800, 3000)])]);
        assert_eq!(c.echecs, 0, "{:?}", c.details);
    }

    /// Une transition qui saute une phase, jamais vue hors d'un gel, échoue.
    #[test]
    fn une_transition_jamais_vue_echoue() {
        let v = vec![
            p(10, 0, N),
            p(20, 1, N),
            p(320, 2, T),
            p(440, 0, T),
            p(700, 1, N),
            p(1000, 0, T),
        ];
        let (c, _) = juger(&[(&v, &[(800, 1000)])]);
        assert_eq!(c.echecs, 1, "{:?}", c.details);
    }

    /// Les références viennent aussi des autres soirées : une transition qu'une soirée
    /// ne fait qu'après ses gels, une autre l'a faite sans gel.
    #[test]
    fn les_references_viennent_de_toutes_les_soirees() {
        let gelee = vec![p(10, 0, N), p(700, 1, N), p(1000, 2, T), p(1100, 1, N)];
        let libre = vec![p(10, 1, N), p(1210, 2, T), p(1300, 1, N)];
        let (c, _) = juger(&[(&gelee, &[(800, 1000)]), (&libre, &[])]);
        assert_eq!(c.echecs, 0, "{:?}", c.details);
        let (c, _) = juger(&[(&gelee, &[(800, 1000)])]);
        assert_eq!(c.echecs, 2, "{:?}", c.details);
    }
}
