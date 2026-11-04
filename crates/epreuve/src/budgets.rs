//! E19 : le carburant par sorte d'appel, en distribution ; le 99ᵉ centile sous le
//! budget, aucun appel au-delà d'une image, et `(fenêtre + 2)` pas sous une image
//! (§ 7.2 ; CONTRAT, « Les budgets »).
//!
//! La sonde note le carburant de chaque appel que le cœur fait sur la définitive des
//! soirées principales, et celui de chaque pas (la somme de ses entrées). Le carburant
//! compte les instructions du module réécrit (§ 3.6) : il est le même à chaque
//! exécution, sur toute machine. Une distribution d'appels se juge sur son 99ᵉ
//! centile, jamais sur sa moyenne ; mais un seul appel au-delà du budget d'une image
//! fait échouer, si rare soit-il.
//!
//! Le coût d'une image où un geste arrive en retard est le rejeu de la fenêtre sur la
//! tête, plus le pas de la définitive et la lecture de la scène : `(fenêtre + 2)` pas
//! (§ 10, « Ce qu'un FPS exige »). Il se mesure sur la soirée elle-même : la somme du
//! carburant de chaque suite de `(fenêtre + 2)` pas consécutifs, pas vides compris.
//! Le CONTRAT dit « aucune image au-delà de 33 ms en jeu » : un maximum, et non un
//! centile. Chaque image se juge donc, la plus lourde comprise : un décompte final
//! dix fois trop lourd, une fois par partie, échoue (INTERFACE § 7.2 disait « le
//! 99ᵉ centile » ; le contrat gagne).
//!
//! # Ce qui n'est pas mesuré
//!
//! Les trois budgets ci-dessous sont des points de départ, choisis et non mesurés : ni
//! le nombre d'instructions qu'une machine de la soirée exécute en une image, ni la
//! part de l'image laissée au jeu ne le sont. Le mainteneur les fixe dans `CONTRAT.md`
//! (REPONSES-PROVISOIRES.md, question 12).

use std::collections::BTreeMap;

use crate::sonde::Compte;

/// Le 99ᵉ centile du carburant d'une entrée, au plus : 2²¹ instructions.
pub const BUDGET_ENTREE: u64 = 1 << 21;
/// Le 99ᵉ centile du carburant d'une lecture du cœur (vues, état, validation,
/// ressources), au plus : 2²¹ instructions.
pub const BUDGET_LECTURE: u64 = 1 << 21;
/// Le carburant d'une image, `(fenêtre + 2)` pas consécutifs, au plus : 2²³
/// instructions, pour chaque image. Aucun appel seul ne le dépasse non plus.
pub const BUDGET_IMAGE: u64 = 1 << 23;

/// Les lectures que le cœur fait en jouant, jugées sur `BUDGET_LECTURE`. Les autres
/// (la forme canonique, la garde, `Bonjour`, `Decrire`) ne tombent pas dans une image.
const LECTURES_JUGEES: [&str; 4] = ["Regarder", "Etat", "Valider", "Ressources"];

/// Un centile d'une suite triée : la plus petite valeur dont au moins `pour_cent` %
/// des valeurs sont plus petites ou égales.
pub(crate) fn centile(triees: &[u64], pour_cent: u64) -> u64 {
    let n = u64::try_from(triees.len()).unwrap_or(0);
    if n == 0 {
        return 0;
    }
    let rang = n.saturating_mul(pour_cent).div_ceil(100).max(1);
    usize::try_from(rang.saturating_sub(1))
        .ok()
        .and_then(|r| triees.get(r))
        .copied()
        .unwrap_or(0)
}

fn decrire(nom: &str, v: &[u64]) -> String {
    format!(
        "{nom} : {} appels, médiane {}, 99ᵉ centile {}, maximum {}",
        v.len(),
        centile(v, 50),
        centile(v, 99),
        v.last().copied().unwrap_or(0)
    )
}

/// Le carburant de chaque suite de `largeur` pas consécutifs, depuis celui de chaque
/// pas.
pub(crate) fn images(pas: &[u64], largeur: usize) -> Vec<u64> {
    pas.windows(largeur.max(1))
        .map(|w| w.iter().fold(0u64, |a, x| a.saturating_add(*x)))
        .collect()
}

/// Juge le carburant de soirées : par appel, et par image (`(fenêtre + 2)` pas
/// consécutifs, déjà sommés par [`images`]).
pub(crate) fn juger(
    appels: &BTreeMap<&'static str, Vec<u64>>,
    images: &[u64],
    fenetre: u64,
) -> (Compte, Vec<String>) {
    let mut c = Compte::default();
    let mut notes = Vec::new();
    for (nom, v) in appels {
        let mut v = v.clone();
        v.sort_unstable();
        notes.push(decrire(nom, &v));
        let budget = if *nom == "Entree" {
            Some(BUDGET_ENTREE)
        } else if LECTURES_JUGEES.contains(nom) {
            Some(BUDGET_LECTURE)
        } else {
            None
        };
        if let Some(b) = budget {
            c.evaluer();
            let p99 = centile(&v, 99);
            if p99 > b {
                c.echouer(format!(
                    "{nom} : le 99ᵉ centile du carburant, {p99}, dépasse le budget de {b}"
                ));
            }
            let max = v.last().copied().unwrap_or(0);
            if max > BUDGET_IMAGE {
                c.echouer(format!(
                    "{nom} : un appel brûle {max}, au-delà du budget d'une image entière, {BUDGET_IMAGE}"
                ));
            }
        }
    }
    let mut images = images.to_vec();
    images.sort_unstable();
    if !images.is_empty() {
        c.evaluer();
        notes.push(decrire(
            &format!("une image, (fenêtre {fenetre} + 2) pas"),
            &images,
        ));
        let trop = images.iter().filter(|i| **i > BUDGET_IMAGE).count();
        if trop > 0 {
            let max = images.last().copied().unwrap_or(0);
            c.echouer(format!(
                "{trop} image(s) de (fenêtre {fenetre} + 2) pas consécutifs dépassent le budget d'une image de {BUDGET_IMAGE}, jusqu'à {max}"
            ));
        }
    }
    notes.push(format!(
        "budgets non mesurés, points de départ : entrée {BUDGET_ENTREE}, lecture {BUDGET_LECTURE}, image {BUDGET_IMAGE}"
    ));
    (c, notes)
}

/// E19 sur les pires cas (E16) : la vue de l'écran de chaque pire cas, construit
/// depuis les limites déclarées, sous le budget d'une lecture. Le carburant des
/// soirées ne mesure que ce que les bots du jeu atteignent ; un jeu qui ménage ses
/// bots ne ménage pas ses pires cas.
pub(crate) fn juger_les_pires_cas(vues: &[(u32, u64)]) -> (Compte, Vec<String>) {
    let mut c = Compte::default();
    let mut notes = Vec::new();
    for (indice, carburant) in vues {
        c.evaluer();
        if *carburant > BUDGET_LECTURE {
            c.echouer(format!(
                "la vue de l'écran du pire cas d'indice {indice} brûle {carburant}, au-delà du budget d'une lecture, {BUDGET_LECTURE}"
            ));
        }
    }
    if let Some(max) = vues.iter().map(|(_, c)| *c).max() {
        notes.push(format!(
            "pires cas : {} vues de l'écran, la plus lourde à {max}",
            vues.len()
        ));
    }
    (c, notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centiles() {
        let v: Vec<u64> = (1..=100).collect();
        assert_eq!(centile(&v, 99), 99);
        assert_eq!(centile(&v, 50), 50);
        assert_eq!(centile(&[7], 99), 7);
        assert_eq!(centile(&[], 99), 0);
    }

    /// Une entrée sur cent qui dépasse ne fait pas échouer ; deux, si.
    #[test]
    fn le_centile_juge_pas_la_moyenne() {
        let mut appels = BTreeMap::new();
        let mut v = vec![10u64; 99];
        v.push(BUDGET_ENTREE + 1);
        let _: Option<Vec<u64>> = appels.insert("Entree", v.clone());
        assert_eq!(juger(&appels, &[1], 0).0.echecs, 0);
        v.push(BUDGET_ENTREE + 1);
        let _: Option<Vec<u64>> = appels.insert("Entree", v);
        assert_eq!(juger(&appels, &[1], 0).0.echecs, 1);
        // Une image est la somme de pas consécutifs : la fenêtre l'élargit.
        let pas = vec![BUDGET_IMAGE / 4; 10];
        assert_eq!(juger(&BTreeMap::new(), &images(&pas, 4), 2).0.echecs, 0);
        assert_eq!(juger(&BTreeMap::new(), &images(&pas, 5), 3).0.echecs, 1);
        assert_eq!(images(&[1, 2, 3], 2), vec![3, 5]);
    }

    /// Un pire cas dont la vue dépasse le budget d'une lecture échoue, quoi que les
    /// bots du jeu aient choisi.
    #[test]
    fn un_pire_cas_trop_lourd_echoue() {
        let (c, _) = juger_les_pires_cas(&[(0, 10), (1, BUDGET_LECTURE + 1)]);
        assert_eq!((c.evaluations, c.echecs), (2, 1), "{:?}", c.details);
    }

    /// CONTRAT, « aucune image au-delà de 33 ms en jeu » : une seule entrée dix fois
    /// au-dessus du budget, une fois par partie, échoue, par l'appel et par l'image,
    /// même sous le 99ᵉ centile.
    #[test]
    fn une_entree_rare_et_lourde_echoue() {
        let mut appels = BTreeMap::new();
        let mut v = vec![10u64; 999];
        v.push(10 * BUDGET_ENTREE);
        let _: Option<Vec<u64>> = appels.insert("Entree", v);
        let mut pas = vec![10u64; 999];
        pas.push(10 * BUDGET_ENTREE);
        let (c, _) = juger(&appels, &images(&pas, 2), 0);
        assert_eq!(c.echecs, 2, "{:?}", c.details);
    }
}
