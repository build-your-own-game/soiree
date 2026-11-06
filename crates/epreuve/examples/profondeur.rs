//! M9, la première moitié (PHASE-3, § 2.3 et § 8.6) : la profondeur d'appels imbriqués
//! qu'un jeu atteint pendant toute son épreuve, en étages de `soiree_profondeur`.
//!
//! ```text
//! cargo run --release -p epreuve --example profondeur -- [--court] <module.wasm>…
//! ```
//!
//! Chaque module livré se charge par la variante de mesure de la réécriture
//! (`Moteur::charger_en_mesurant_la_profondeur`) sous `ReglagesMoteur::PHASE_1`, dont la
//! profondeur permise (4 096 étages) est la plus grande qu'on ait jamais accordée : une
//! profondeur atteinte au-dessus du réglage retenu se voit ainsi, au lieu de tomber en
//! faute. L'épreuve joue ensuite son plan (standard, ou court avec `--court`) sur ce
//! module, sans copie native : chaque appel que l'épreuve fait au module, pires cas,
//! lectures, soirées, sondes et copies, passe par le protocole, qui lit après lui la plus
//! grande profondeur vue (`moteur_commun::profondeur_atteinte`).
//!
//! La variante brûle le même carburant et rend les mêmes octets que le module de la
//! partie (`reecriture::passes::profondeur_mesuree`) : l'épreuve y joue les mêmes
//! soirées. Le code de ses contrôles est donné pour mémoire, pas comme un rapport : le
//! module mesuré n'est pas celui qu'on publie, et la copie native manque (les contrôles
//! qui la demandent ne jugent rien ici).
//!
//! La profondeur ne dépend ni de la machine ni du moteur : la réécriture la compte sur
//! le module seul.

#![forbid(unsafe_code)]

use std::process::ExitCode;

use epreuve::Plan;
use moteur_wasmtime::{Moteur, ReglagesMoteur};

fn mesurer(moteur: &Moteur, chemin: &str, plan: &Plan) -> Result<(), String> {
    let original = std::fs::read(chemin).map_err(|e| format!("{chemin} : {e}"))?;
    moteur_commun::oublier_la_profondeur_atteinte();
    let module = moteur
        .charger_en_mesurant_la_profondeur(&original)
        .map_err(|r| format!("refusé au chargement : {r}"))?;
    let au_chargement = moteur_commun::profondeur_atteinte();
    let rapport = epreuve::eprouver(&module, None, plan);
    let atteinte = moteur_commun::profondeur_atteinte();
    let en_echec: Vec<String> = rapport
        .non_reussis()
        .iter()
        .map(|c| format!("{} ({})", c.id.nom(), c.issue.nom()))
        .collect();
    println!("{chemin}");
    println!(
        "  profondeur atteinte : {atteinte} étages (au chargement : {au_chargement}) ; permise ici : {}",
        ReglagesMoteur::PHASE_1.reecriture.profondeur_max
    );
    println!(
        "  pour mémoire, sans copie native : code des contrôles {} ; hors de « réussi » et « sans objet » : {}",
        rapport.code_des_controles(),
        if en_echec.is_empty() {
            String::from("aucun")
        } else {
            en_echec.join(", ")
        }
    );
    Ok(())
}

fn main() -> ExitCode {
    let mut court = false;
    let mut chemins = Vec::new();
    for a in std::env::args().skip(1) {
        if a == "--court" {
            court = true;
        } else {
            chemins.push(a);
        }
    }
    if chemins.is_empty() {
        eprintln!("usage : profondeur [--court] <module.wasm>…");
        return ExitCode::from(64);
    }
    let plan = if court {
        Plan::court([0x5e; 32])
    } else {
        Plan::standard([0x5e; 32])
    };
    println!(
        "plan {} : {} soirée(s) de {} s, soirée longue de {} s",
        if court { "court" } else { "standard" },
        plan.soirees,
        plan.duree.en_ms() / 1000,
        plan.memoire.en_ms() / 1000
    );
    let moteur = match Moteur::new(ReglagesMoteur::PHASE_1) {
        Ok(m) => m,
        Err(p) => {
            eprintln!("moteur : {p}");
            return ExitCode::from(70);
        }
    };
    let mut code = ExitCode::SUCCESS;
    for c in &chemins {
        if let Err(e) = mesurer(&moteur, c, &plan) {
            eprintln!("{c} : {e}");
            code = ExitCode::from(1);
        }
    }
    code
}
