//! Le script d'une soirée rejoué sur une autre instance (E9, E14) : les mêmes
//! arrivées, aux mêmes instants, les mêmes avancées, les mêmes regards, les mêmes
//! points et les mêmes redémarrages.
//!
//! Sur la copie native, la partie doit écrire le même journal que sur le module, aux
//! empreintes d'instance et aux messages de panique près : une empreinte d'instance
//! ne se compare qu'entre instances de la même sorte (`partie::Instance`), et le
//! message d'une panique dépend de la façon dont elle est rattrapée. Ses reprises,
//! depuis ses propres points, doivent retrouver sa propre forme canonique.

use jeu_abi::Oui;
use partie::{Fabrique, Faute, IssueJournal, Journal, Ligne, LigneEntree, Partie};

use crate::soiree::{Op, ouverture, partie_au};
use crate::sonde::{Compte, Obs, Sonde, Verifs, premiere_entree_differente};

/// Ce que le rejeu d'un script a vu.
#[derive(Debug, Default)]
pub(crate) struct Rejoue {
    /// Le journal diverge de la référence avant tout redémarrage : c'est le
    /// déterminisme (E9), pas la reprise.
    pub(crate) divergence_avant: Option<String>,
    /// Les constats de la reprise (E14) : aux redémarrages, et depuis les points.
    pub(crate) e14: Compte,
    /// Les redémarrages rejoués.
    pub(crate) redemarrages: u64,
    /// Une panne de l'outil, s'il y en a eu une.
    pub(crate) panne: Option<String>,
}

/// Une ligne du journal, comparable d'une sorte d'instance à l'autre.
fn comparable(l: &Ligne) -> Ligne {
    match l {
        Ligne::Empreinte { partie, ordre, .. } => Ligne::Empreinte {
            partie: *partie,
            ordre: *ordre,
            empreinte: [0; 32],
        },
        Ligne::Entree(e) => match &e.issue {
            IssueJournal::Ecartee {
                faute: Faute::Panique { .. },
            } => Ligne::Entree(LigneEntree {
                issue: IssueJournal::Ecartee {
                    faute: Faute::Panique {
                        message: String::new(),
                    },
                },
                ..e.clone()
            }),
            IssueJournal::Acceptee { .. }
            | IssueJournal::Refusee { .. }
            | IssueJournal::Ignoree
            | IssueJournal::Ecartee { .. } => l.clone(),
        },
        Ligne::Arrivees { .. }
        | Ligne::Saut { .. }
        | Ligne::Pause { .. }
        | Ligne::Vue { .. }
        | Ligne::Partie { .. }
        | Ligne::Reprise { .. } => l.clone(),
    }
}

/// La première ligne où deux journaux diffèrent, sur les `n` premières.
pub(crate) fn premiere_difference(
    gauche: &Journal,
    droite: &Journal,
    lignes: usize,
) -> Option<String> {
    let la = gauche.lignes();
    let lb = droite.lignes();
    for rang in 0..lignes {
        match (la.get(rang), lb.get(rang)) {
            (Some(g), Some(d)) if comparable(g) == comparable(d) => {}
            (None, None) => return None,
            (g, d) => {
                return Some(format!(
                    "ligne {rang} du journal : {} contre {}",
                    resume(g),
                    resume(d)
                ));
            }
        }
    }
    None
}

fn resume(l: Option<&Ligne>) -> String {
    let s = l.map_or_else(|| String::from("la fin du journal"), |l| format!("{l:?}"));
    s.chars().take(200).collect()
}

/// Rejoue le script sur des parties de cette fabrique, et compare au journal de
/// référence.
#[expect(
    clippy::too_many_lines,
    reason = "un bras par opération du script, puis les reprises depuis les points"
)]
pub(crate) fn rejouer<F: Fabrique + Clone>(
    fabrique: &F,
    graine: [u8; 32],
    script: &[Op],
    reference: &Journal,
) -> Rejoue {
    let mut rejoue = Rejoue::default();
    let ouv = ouverture(graine);
    let releve = Verifs {
        releve: Oui::Oui,
        ..Verifs::RIEN
    };
    let obs = Obs::new(releve, ouv.budgets.lecture);
    let mut partie = match Partie::ouvrir(Sonde::new(fabrique.clone(), &obs), ouv.clone()) {
        Ok(p) => p,
        Err(erreur) => {
            rejoue.divergence_avant = Some(format!("la partie ne s'ouvre pas : {erreur:?}"));
            return rejoue;
        }
    };
    let mut points = Vec::new();
    let mut dernier = None;
    for op in script {
        let res: Result<(), String> = match op {
            Op::Arriver(arrivee, recu) => partie
                .arriver(arrivee.clone(), *recu)
                .map(|_| ())
                .map_err(|panne| panne.to_string()),
            Op::Avancer(cible) => partie
                .avancer(*cible)
                .map(|_| ())
                .map_err(|panne| panne.to_string()),
            Op::Regarder(regards) => partie
                .regarder(regards)
                .map(|_| ())
                .map_err(|panne| panne.to_string()),
            Op::Etat => {
                let _: Result<partie::EtatVisible, partie::Echec> = partie.etat();
                Ok(())
            }
            Op::Canonique => {
                let _: Result<Vec<u8>, partie::Echec> = partie.canonique();
                Ok(())
            }
            Op::Point => match partie.point() {
                Ok(point) => {
                    if points.len() < 4 {
                        points.push(point.clone());
                    }
                    dernier = Some(point);
                    Ok(())
                }
                Err(panne) => Err(panne.to_string()),
            },
            // La partie, en panne, repart de son point sain : les points pris depuis
            // ne servent plus (une reprise depuis eux ne retrouve pas la reprise).
            Op::Repartir => match partie.repartir() {
                Ok(_) => {
                    if let Some(Ligne::Reprise { depuis, .. }) = partie.journal().lignes().last() {
                        let depuis = *depuis;
                        points.retain(|p| p.indice() <= depuis);
                        if dernier.as_ref().is_some_and(|p| p.indice() > depuis) {
                            dernier = None;
                        }
                    }
                    Ok(())
                }
                Err(panne) => Err(panne.to_string()),
            },
            Op::Redemarrer { lignes } => {
                if rejoue.redemarrages == 0
                    && let Some(d) = premiere_difference(partie.journal(), reference, *lignes)
                {
                    rejoue.divergence_avant = Some(d);
                    return rejoue;
                }
                let Some(point) = dernier.clone() else {
                    rejoue.panne = Some(String::from("un redémarrage sans point de reprise"));
                    return rejoue;
                };
                rejoue.redemarrages = rejoue.redemarrages.saturating_add(1);
                rejoue.e14.evaluer();
                let avant = partie.canonique();
                let entrees = obs.borrow().entrees.clone();
                let journal = partie.journal().clone();
                {
                    let mut ob = obs.borrow_mut();
                    let numero = partie_au(&journal, point.indice());
                    ob.partie_reprise = numero;
                    ob.prochaine_partie = numero.saturating_add(1);
                }
                match Partie::reprendre(
                    Sonde::new(fabrique.clone(), &obs),
                    ouv.clone(),
                    &point,
                    &journal,
                ) {
                    Ok(mut reprise) => {
                        let pas = reprise.definitif().numero();
                        if let Some((numero, ordre)) =
                            premiere_entree_differente(&entrees, &obs.borrow().entrees)
                        {
                            rejoue.e14.echouer(format!(
                                "copie native : le redémarrage du pas {pas} diverge à l'entrée d'ordre {ordre} de la partie {numero}"
                            ));
                        }
                        let apres = reprise.canonique();
                        if !matches!((&avant, &apres), (Ok(a), Ok(b)) if a == b) {
                            rejoue.e14.echouer(format!(
                                "copie native : la forme canonique après le redémarrage du pas {pas} n'est pas celle d'avant"
                            ));
                        }
                        partie = reprise;
                        Ok(())
                    }
                    Err(erreur) => {
                        rejoue
                            .e14
                            .echouer(format!("copie native : le redémarrage échoue : {erreur:?}"));
                        return rejoue;
                    }
                }
            }
        };
        if let Err(panne) = res {
            rejoue.panne = Some(panne);
            return rejoue;
        }
    }
    let lignes = reference
        .lignes()
        .len()
        .max(partie.journal().lignes().len());
    if let Some(d) = premiere_difference(partie.journal(), reference, lignes) {
        if rejoue.redemarrages == 0 {
            rejoue.divergence_avant = Some(d);
        } else {
            rejoue.e14.evaluer();
            rejoue
                .e14
                .echouer(format!("copie native, après un redémarrage : {d}"));
        }
        return rejoue;
    }
    let finale = partie.canonique();
    let journal = partie.journal().clone();
    for point in points {
        rejoue.e14.evaluer();
        let obs_reprise = Obs::new(releve, ouv.budgets.lecture);
        {
            let mut ob = obs_reprise.borrow_mut();
            let numero = partie_au(&journal, point.indice());
            ob.partie_reprise = numero;
            ob.prochaine_partie = numero.saturating_add(1);
        }
        let pas = point.pas().numero();
        match Partie::reprendre(
            Sonde::new(fabrique.clone(), &obs_reprise),
            ouv.clone(),
            &point,
            &journal,
        ) {
            Ok(mut reprise) => {
                if let Some((numero, ordre)) =
                    premiere_entree_differente(&obs.borrow().entrees, &obs_reprise.borrow().entrees)
                {
                    rejoue.e14.echouer(format!(
                        "copie native : la reprise depuis le pas {pas} diverge à l'entrée d'ordre {ordre} de la partie {numero}"
                    ));
                }
                if !matches!((&finale, reprise.canonique()), (Ok(a), Ok(b)) if *a == b) {
                    rejoue.e14.echouer(format!(
                        "copie native : la reprise depuis le pas {pas} n'a pas la forme canonique de la partie"
                    ));
                }
            }
            Err(erreur) => rejoue.e14.echouer(format!(
                "copie native : la reprise depuis le pas {pas} échoue : {erreur:?}"
            )),
        }
    }
    rejoue
}

/// Le journal a-t-il une faute que la copie native ne saurait pas refaire sans
/// carburant ni plafond de mémoire (une boucle sans fin, une récursion, une mémoire
/// sans borne) ?
pub(crate) fn faute_sans_borne_native(j: &Journal) -> bool {
    j.entrees().any(|e| {
        matches!(
            e.issue,
            IssueJournal::Ecartee {
                faute: Faute::Budget | Faute::Profondeur | Faute::Memoire | Faute::HorsMemoire
            }
        )
    })
}
