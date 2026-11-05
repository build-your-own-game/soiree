//! `soiree-banc` : le banc des navigateurs (PHASE-3, § 8.2).
//!
//! ```text
//! soiree-banc machine [--sans-construire] [--chrome <exécutable>]
//! soiree-banc regles-web
//! ```
//!
//! Code de sortie : 0 réussi ; 1 échec (dont la règle « mêmes lints » qui refuse
//! `web/`, une page qui ne rapporte rien ou rapporte une erreur) ; 2 pas exécuté (un
//! navigateur absent, une construction ratée, une commande illisible). Un scénario à la fois, jamais à côté
//! d'une compilation (OUT-89, OUT-115).

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use banc::machine::{self, Arret, Options};
use banc::rapport::Attendu;

fn usage() -> ExitCode {
    eprintln!(
        "usage : soiree-banc machine [--sans-construire] [--chrome <exécutable>]\n        soiree-banc regles-web"
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
            "--chrome" => match i.next() {
                Some(c) => o.chrome = PathBuf::from(c),
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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.split_first() {
        Some((c, reste)) if c == "machine" => machine(reste),
        Some((c, reste)) if c == "regles-web" && reste.is_empty() => regles_web(),
        _ => usage(),
    }
}
