//! `soiree-banc` : le banc des navigateurs (PHASE-3, § 8.2).
//!
//! ```text
//! soiree-banc machine [--sans-construire] [--navigateur chrome|firefox] [--executable <chemin>]
//! soiree-banc rejouer <dossier> [--navigateurs chrome,firefox] [--fautif fausser-apres:<n>]
//!                     [--sortie <fichier.json>] [--delai <s>] [--sans-construire]
//! soiree-banc regles-web
//! ```
//!
//! `machine` : les essais de `Machine` (lot 3a). `rejouer` : E20, que `soiree-epreuve
//! --navigateurs` lance sur le dossier qu'il écrit (lot 3b, `banc::rejouer`).
//!
//! Code de sortie : 0 réussi ; 1 échec (dont la règle « mêmes lints » qui refuse
//! `web/`, une page qui ne rapporte rien ou rapporte une erreur pour `machine`, un
//! écart pour `rejouer`) ; 2 pas exécuté ou panne de l'outil (un navigateur absent, une
//! construction ratée, une commande illisible, un rejeu qui n'a pas tourné). Un
//! scénario à la fois, jamais à côté d'une compilation (OUT-89, OUT-115).

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use std::time::Duration;

use banc::machine::{self, Arret, Options};
use banc::navigateur::Navigateur;
use banc::rapport::Attendu;
use banc::rejouer;

fn usage() -> ExitCode {
    eprintln!(
        "usage : soiree-banc machine [--sans-construire] [--navigateur chrome|firefox] [--executable <chemin>]\n        soiree-banc rejouer <dossier> [--navigateurs chrome,firefox] [--fautif fausser-apres:<n>] [--sortie <fichier.json>] [--delai <s>] [--sans-construire]\n        soiree-banc regles-web"
    );
    ExitCode::from(2)
}

fn regles_web() -> ExitCode {
    let soiree = banc::commun();
    let ecarts = banc::web::ecarts(&soiree, &soiree.join("web"));
    if ecarts.is_empty() {
        println!("web/ : mêmes lints, même clippy.toml, même rust-toolchain.toml que le commun");
        ExitCode::SUCCESS
    } else {
        println!("web/ s'écarte des règles d'écriture du commun :");
        for e in ecarts {
            println!("  {e}");
        }
        ExitCode::from(1)
    }
}

fn machine(args: &[String]) -> ExitCode {
    let mut o = Options::default();
    let mut i = args.iter();
    while let Some(a) = i.next() {
        match a.as_str() {
            "--sans-construire" => o.construire = false,
            "--navigateur" => match i.next().and_then(|n| Navigateur::depuis_nom(n)) {
                Some(n) => o.navigateur = n,
                None => return usage(),
            },
            "--executable" | "--chrome" => match i.next() {
                Some(c) => o.executable = Some(PathBuf::from(c)),
                None => return usage(),
            },
            _ => return usage(),
        }
    }
    let passages = match machine::jouer(&o) {
        Ok(p) => p,
        Err(Arret::Echec(e)) => {
            println!("machine : échec\n{e}");
            return ExitCode::from(1);
        }
        Err(Arret::PasExecute(e)) => {
            println!("machine : pas exécuté\n{e}");
            return ExitCode::from(2);
        }
    };
    let mut code = ExitCode::SUCCESS;
    for p in &passages {
        let quoi = match p.attendu {
            Attendu::ToutReussi => String::from("la machine du navigateur"),
            Attendu::SeuleEchoue(f) => {
                format!("contrôle négatif « {} » (seule {f} doit échouer)", p.fautif)
            }
        };
        println!("\n== {quoi} — {}", p.navigateur);
        if let Some(r) = &p.rapport {
            println!("agent : {}", r.agent);
            for e in &r.essais {
                let marque = if e.reussi { "réussi" } else { "ÉCHEC " };
                println!("  {marque} {} : {}", e.nom, e.detail);
            }
            for l in &r.leves {
                println!(
                    "  levé, scénario {} : {} : {}",
                    l.scenario, l.nom, l.message
                );
            }
        }
        for c in &p.console {
            println!("  page : {c}");
        }
        match &p.verdict {
            Ok(v) => println!("verdict : réussi — {v}"),
            Err(e) if e.starts_with("pas exécuté") => {
                println!("verdict : {e}");
                code = ExitCode::from(2);
            }
            Err(e) => {
                println!("verdict : échec — {e}");
                if code == ExitCode::SUCCESS {
                    code = ExitCode::from(1);
                }
            }
        }
    }
    code
}

fn rejouer_e20(args: &[String]) -> ExitCode {
    let mut o = rejouer::Options::default();
    let mut dossier = None;
    let mut sortie = None;
    let mut i = args.iter();
    while let Some(a) = i.next() {
        match a.as_str() {
            "--sans-construire" => o.construire = false,
            "--navigateurs" => match i.next().map(|n| Navigateur::liste(n)) {
                Some(Ok(n)) => o.navigateurs = n,
                Some(Err(e)) => {
                    eprintln!("{e}");
                    return usage();
                }
                None => return usage(),
            },
            "--fautif" => match i.next() {
                Some(f) if f.starts_with(rejouer::FAUSSER_APRES) => o.fautif = Some(f.clone()),
                _ => return usage(),
            },
            "--sortie" => match i.next() {
                Some(s) => sortie = Some(PathBuf::from(s)),
                None => return usage(),
            },
            "--delai" => match i.next().and_then(|d| d.parse::<u64>().ok()) {
                Some(s) => o.delai = Duration::from_secs(s),
                None => return usage(),
            },
            _ if a.starts_with("--") || dossier.is_some() => return usage(),
            _ => dossier = Some(PathBuf::from(a)),
        }
    }
    let Some(dossier) = dossier else {
        return usage();
    };
    let ecrire = |v: &serde_json::Value| {
        if let Some(s) = &sortie
            && let Err(e) = std::fs::write(s, v.to_string())
        {
            eprintln!("{} : {e}", s.display());
        }
    };
    let (resultats, construction_ms) = match rejouer::jouer(&dossier, &o) {
        Ok(r) => r,
        Err(Arret::Echec(e)) => {
            println!("rejouer : échec\n{e}");
            ecrire(&serde_json::json!({ "arret": "echec", "detail": e }));
            return ExitCode::from(1);
        }
        Err(Arret::PasExecute(e)) => {
            println!("rejouer : pas exécuté\n{e}");
            ecrire(&serde_json::json!({ "arret": "pas_execute", "detail": e }));
            return ExitCode::from(2);
        }
    };
    let mut code = 0u8;
    println!("construction et règle « mêmes lints » : {construction_ms} ms");
    for r in &resultats {
        println!(
            "\n== E20 — {} ({}) : {} en {} ms (chargement {:.0} ms, reprise {:.0} ms ; {} ms lancement et fermeture compris)",
            r.navigateur.nom(),
            r.version,
            r.issue.code(),
            r.duree_ms,
            r.chargement_ms,
            r.rejeu_ms,
            r.total_ms
        );
        println!("{}", r.detail);
        for c in &r.console {
            println!("  page : {c}");
        }
        code = code.max(match r.issue {
            rejouer::Issue::Reussi => 0,
            rejouer::Issue::Echec => 1,
            rejouer::Issue::Panne | rejouer::Issue::PasExecute => 2,
        });
    }
    ecrire(&serde_json::json!({
        "construction_ms": construction_ms,
        "navigateurs": resultats.iter().map(rejouer::Resultat::json).collect::<Vec<_>>(),
    }));
    // Un échec l'emporte : c'est ce qu'il faut corriger d'abord.
    if resultats.iter().any(|r| r.issue == rejouer::Issue::Echec) {
        code = 1;
    }
    ExitCode::from(code)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.split_first() {
        Some((c, reste)) if c == "machine" => machine(reste),
        Some((c, reste)) if c == "rejouer" => rejouer_e20(reste),
        Some((c, reste)) if c == "regles-web" && reste.is_empty() => regles_web(),
        _ => usage(),
    }
}
