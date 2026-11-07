//! `soiree-epreuve <jeu> --catalogue <dossier>` (PHASE-3, § 2.8, P17 ; lot 10b) : sur un
//! **code 0 du plan standard seulement**, le module livré et son rapport vont au catalogue
//! que `soiree-relais jouer` lit : `<empreinte>.wasm` (les octets livrés) et
//! `<empreinte>.json` (l'empreinte livrée, l'empreinte réécrite et `reecriture::VERSION`,
//! la date, le plan, le nom, les places, le verdict).
//!
//! *Corrigé après la vérification du lot 10b* : le plan court « ne garantit pas le
//! résultat d'un jeu » (PHASE-3, § 7), et un plan réglé ne joue pas ce que le CONTRAT
//! appelle l'épreuve (« chaque jeu passe l'épreuve ») ; un code 0 sur l'un d'eux ne fait
//! donc pas un module éprouvé, et rien ne va au catalogue. E20 n'est pas exigé (P17,
//! question 20) : le plan standard avec `--navigateurs` reste le plan standard.
//!
//! L'empreinte réécrite est celle de `ReglagesMoteur::PHASE_3`, la réécriture que le
//! Worker de l'autorité refait au chargement (§ 2.8, point 2) : il la compare à celle-ci.

use std::path::Path;

use moteur_wasmtime::Moteur;

use crate::json::Json;
use crate::plan::Plan;
use crate::rapport::{Rapport, hex};

/// La date d'aujourd'hui (UTC), `AAAA-MM-JJ`, depuis l'heure du système.
#[expect(
    clippy::disallowed_methods,
    reason = "la date d'un rapport de catalogue, lue une fois ; rien de ce que l'épreuve juge n'en dépend"
)]
fn aujourd_hui() -> String {
    let secondes = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    date_du_jour(secondes / 86_400)
}

/// La date civile (calendrier grégorien) du jour `jours` depuis le 1970-01-01 (H. Hinnant,
/// « `civil_from_days` »), en entiers non signés.
pub(crate) fn date_du_jour(jours: u64) -> String {
    let z = jours.saturating_add(719_468);
    let ere = z / 146_097;
    let jour_de_l_ere = z.saturating_sub(ere.saturating_mul(146_097));
    let annee_de_l_ere = jour_de_l_ere
        .saturating_sub(jour_de_l_ere / 1460)
        .saturating_add(jour_de_l_ere / 36_524)
        .saturating_sub(jour_de_l_ere / 146_096)
        / 365;
    let jour_de_l_annee = jour_de_l_ere.saturating_sub(
        annee_de_l_ere
            .saturating_mul(365)
            .saturating_add(annee_de_l_ere / 4)
            .saturating_sub(annee_de_l_ere / 100),
    );
    let mois_decale = jour_de_l_annee.saturating_mul(5).saturating_add(2) / 153;
    let jour = jour_de_l_annee
        .saturating_sub(mois_decale.saturating_mul(153).saturating_add(2) / 5)
        .saturating_add(1);
    let mois = if mois_decale < 10 {
        mois_decale.saturating_add(3)
    } else {
        mois_decale.saturating_sub(9)
    };
    let annee = annee_de_l_ere
        .saturating_add(ere.saturating_mul(400))
        .saturating_add(u64::from(mois <= 2));
    format!("{annee:04}-{mois:02}-{jour:02}")
}

/// Le nom du plan joué : `standard`, `court`, ou `réglé` (des options l'ont changé).
fn nom_du_plan(plan: &Plan) -> &'static str {
    let mut standard = Plan::standard(plan.graine);
    let mut court = Plan::court(plan.graine);
    standard.navigateurs.clone_from(&plan.navigateurs);
    court.navigateurs.clone_from(&plan.navigateurs);
    standard.controles.clone_from(&plan.controles);
    court.controles.clone_from(&plan.controles);
    if *plan == standard
        && plan
            .controles
            .starts_with(&Plan::standard(plan.graine).controles)
    {
        "standard"
    } else if *plan == court
        && plan
            .controles
            .starts_with(&Plan::court(plan.graine).controles)
    {
        "court"
    } else {
        "réglé"
    }
}

/// Le rapport de catalogue d'un module éprouvé au code 0.
pub(crate) fn rapport_de_catalogue(
    moteur: &Moteur,
    octets: &[u8],
    rapport: &Rapport,
    date: &str,
) -> Result<String, String> {
    let charge = moteur
        .charger(octets)
        .map_err(|e| format!("le module ne se recharge pas pour le catalogue : {e}"))?;
    let d = charge.declaration();
    let places = d.places;
    Ok(Json::Objet(vec![
        ("empreinte", Json::texte(&hex(&rapport.module))),
        ("reecrite", Json::texte(&hex(&charge.reecrit().empreinte))),
        ("reecriture", Json::Nombre(u64::from(reecriture::VERSION))),
        ("date", Json::texte(date)),
        (
            "plan",
            Json::Objet(vec![
                ("nom", Json::texte(nom_du_plan(&rapport.plan))),
                ("soirees", Json::Nombre(u64::from(rapport.plan.soirees))),
                ("duree_ms", Json::Nombre(rapport.plan.duree.en_ms())),
                ("memoire_ms", Json::Nombre(rapport.plan.memoire.en_ms())),
                ("graine", Json::texte(&hex(&rapport.plan.graine))),
            ]),
        ),
        ("nom", Json::texte(d.nom.as_str())),
        (
            "places",
            Json::Objet(vec![
                ("joueurs_min", Json::Nombre(u64::from(places.joueurs_min()))),
                ("joueurs_max", Json::Nombre(u64::from(places.joueurs_max()))),
                (
                    "appareils_max",
                    Json::Nombre(u64::from(places.appareils_max())),
                ),
            ]),
        ),
        ("code", Json::Nombre(u64::from(rapport.code_de_sortie()))),
        ("verdict", Json::texte(&rapport.verdict_du_code_0())),
    ])
    .ecrire())
}

/// Pourquoi un rapport ne fait pas un module éprouvé (P17) : un code non nul, ou un code 0
/// sur un autre plan que le plan standard ; `None` : il va au catalogue.
pub(crate) fn refus_du_catalogue(code: u8, plan: &Plan) -> Option<String> {
    if code != 0 {
        return Some(format!(
            "code {code} : rien ne va au catalogue (on ne joue qu'un module éprouvé, P17)"
        ));
    }
    match nom_du_plan(plan) {
        "standard" => None,
        autre => Some(format!(
            "code 0 sur le plan {autre} : rien ne va au catalogue (seul le plan standard éprouve un module, P17 ; relance sans --court ni option du plan)"
        )),
    }
}

/// Dépose le module et son rapport au catalogue, sur un code 0 du plan standard
/// seulement ; rend ce qui a été écrit, ou pourquoi rien ne l'a été.
pub(crate) fn deposer(
    dossier: &Path,
    moteur: &Moteur,
    octets: &[u8],
    rapport: &Rapport,
) -> Result<String, String> {
    if let Some(refus) = refus_du_catalogue(rapport.code_de_sortie(), &rapport.plan) {
        return Err(refus);
    }
    let json = rapport_de_catalogue(moteur, octets, rapport, &aujourd_hui())?;
    std::fs::create_dir_all(dossier).map_err(|e| format!("{} : {e}", dossier.display()))?;
    let e = hex(&rapport.module);
    let wasm = dossier.join(format!("{e}.wasm"));
    let rapport_json = dossier.join(format!("{e}.json"));
    // Le module d'abord, le rapport ensuite : un rapport ne se lit jamais sans son module.
    std::fs::write(&wasm, octets).map_err(|x| format!("{} : {x}", wasm.display()))?;
    std::fs::write(&rapport_json, json).map_err(|x| format!("{} : {x}", rapport_json.display()))?;
    Ok(format!("{} et {}", wasm.display(), rapport_json.display()))
}

#[cfg(test)]
mod tests {
    use super::{date_du_jour, nom_du_plan, refus_du_catalogue};
    use crate::plan::{IdControle, Navigateur, Plan};

    #[test]
    fn la_date_civile_d_un_jour() {
        assert_eq!(date_du_jour(0), "1970-01-01");
        assert_eq!(date_du_jour(59), "1970-03-01");
        assert_eq!(date_du_jour(11_016), "2000-02-29");
        assert_eq!(date_du_jour(20_399), "2025-11-07");
        assert_eq!(date_du_jour(20_764), "2026-11-07");
    }

    #[test]
    fn le_nom_du_plan() {
        assert_eq!(nom_du_plan(&Plan::standard([1; 32])), "standard");
        assert_eq!(nom_du_plan(&Plan::court([1; 32])), "court");
        let mut p = Plan::standard([1; 32]);
        p.navigateurs = vec![Navigateur::Chrome];
        p.controles.push(IdControle::E20);
        assert_eq!(nom_du_plan(&p), "standard");
        p.soirees = 1;
        assert_eq!(nom_du_plan(&p), "réglé");
        let mut p = Plan::standard([1; 32]);
        p.controles = vec![IdControle::E1];
        assert_eq!(nom_du_plan(&p), "réglé");
    }

    /// P17, après la vérification du lot 10b : seul un code 0 du plan standard fait un
    /// module éprouvé ; un code 0 du plan court ou d'un plan réglé ne va pas au catalogue.
    #[test]
    fn seul_le_code_0_du_plan_standard_va_au_catalogue_p17() {
        assert_eq!(refus_du_catalogue(0, &Plan::standard([2; 32])), None);
        let mut avec_e20 = Plan::standard([2; 32]);
        avec_e20.navigateurs = vec![Navigateur::Chrome];
        avec_e20.controles.push(IdControle::E20);
        assert_eq!(refus_du_catalogue(0, &avec_e20), None);
        let court = refus_du_catalogue(0, &Plan::court([2; 32])).expect("le plan court refusé");
        assert!(court.contains("plan court"), "{court}");
        let mut regle = Plan::standard([2; 32]);
        regle.soirees = 1;
        let regle = refus_du_catalogue(0, &regle).expect("un plan réglé refusé");
        assert!(regle.contains("plan réglé"), "{regle}");
        for code in [1, 2, 3] {
            let refus = refus_du_catalogue(code, &Plan::standard([2; 32])).expect("refusé");
            assert!(refus.starts_with(&format!("code {code} ")), "{refus}");
        }
    }
}
