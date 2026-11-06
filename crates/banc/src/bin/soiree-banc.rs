//! `soiree-banc` : le banc des navigateurs (PHASE-3, § 8.2).
//!
//! ```text
//! soiree-banc machine [--sans-construire] [--navigateur chrome|firefox|safari] [--executable <chemin>]
//! soiree-banc rejouer <dossier> [--navigateurs chrome,firefox,safari] [--fautif fausser-apres:<n>]
//!                     [--sortie <fichier.json>] [--delai <s>] [--sans-construire]
//! soiree-banc etalonner [--navigateurs chrome,firefox,safari] [--mesures m1,m10,m9]
//!                       [--donnees <dossier>] [--modules devine,…] [--passes <n>] [--tours <n>]
//!                       [--fois <n>] [--budget <unités>] [--etiquette <nom>] [--sans-construire]
//!                       [--fautif fausser-apres:<n>]
//! soiree-banc regles-web
//! soiree-banc soiree-native <module.wasm> [--telephones N] [--duree S] [--graine G] [--sans-pannes]
//!                           [--tuer-le-relais N] [--partie-entiere] [--relais <binaire> | --relais-interne]
//!                           [--travail <dossier>] [--calme S]
//! ```
//!
//! `machine` : les essais de `Machine` (lot 3a). `rejouer` : E20, que `soiree-epreuve
//! --navigateurs` lance sur le dossier qu'il écrit (lot 3b, `banc::rejouer`).
//! `etalonner` : M1, M10 et M9 dans chaque navigateur (lot 3c, `banc::etalonner`), sur
//! la capture du harnais (`mesures/navigateur/README.md`). `soiree-native` : une soirée
//! par le vrai relais (le binaire `soiree-relais` à côté de celui-ci, `SOIREE_RELAIS`, ou
//! `--relais`), l'autorité et des téléphones natifs, sous pannes (lot 5,
//! `banc::soiree_native`) ; code 0 si tout tient, 1 sinon, 2 si l'outil n'a pas pu jouer.
//!
//! Code de sortie : 0 réussi (pour `etalonner` : tout a été mesuré, sans défaut) ; 1
//! échec (dont la règle « mêmes lints » qui refuse `web/`, une page qui ne rapporte
//! rien ou rapporte une erreur pour `machine`, un écart pour `rejouer` ; pour
//! `etalonner`, un défaut : un écart de carburant, d'issue ou de retour avec wasmtime,
//! un module réécrit autrement que dans la capture, une récursion de M9 sous `PHASE_3`
//! qui n'est pas la faute `Profondeur` ou sans plafond que la pile native n'arrête pas,
//! et, sous `--fautif`, l'écart que le contrôle négatif de M1 doit compter ; il
//! l'emporte sur « pas exécuté ») ; 2 pas exécuté ou panne de l'outil (un navigateur
//! absent, une construction ratée, une commande illisible, un rejeu qui n'a pas
//! tourné, et, sous `--fautif`, un contrôle négatif qui n'a pas vu son écart au
//! `n + 1`-ième appel, ce qui l'emporte sur tout, pour `rejouer` comme pour
//! `etalonner`). Un scénario à la fois, jamais à côté d'une compilation (OUT-89,
//! OUT-115).

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use std::time::Duration;

use banc::etalonner;
use banc::machine::{self, Arret, Options};
use banc::navigateur::Navigateur;
use banc::rapport::Attendu;
use banc::rejouer;
use banc::soiree_native;

fn usage() -> ExitCode {
    eprintln!(
        "usage : soiree-banc machine [--sans-construire] [--navigateur chrome|firefox|safari] [--executable <chemin>]\n        soiree-banc rejouer <dossier> [--navigateurs chrome,firefox,safari] [--fautif fausser-apres:<n>] [--sortie <fichier.json>] [--delai <s>] [--sans-construire]\n        soiree-banc etalonner [--navigateurs chrome,firefox,safari] [--mesures m1,m10,m9] [--donnees <dossier>] [--modules devine,…] [--passes <n>] [--tours <n>] [--fois <n>] [--budget <unités>] [--etiquette <nom>] [--sans-construire] [--fautif fausser-apres:<n>]\n        soiree-banc regles-web\n        soiree-banc soiree-native <module.wasm> [--telephones N] [--duree S] [--graine G] [--sans-pannes] [--tuer-le-relais N] [--partie-entiere] [--relais <binaire> | --relais-interne] [--travail <dossier>] [--calme S]"
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

fn etalonner_(args: &[String]) -> ExitCode {
    let mut o = etalonner::Options::default();
    let mut i = args.iter();
    while let Some(a) = i.next() {
        let valeur = match a.as_str() {
            "--sans-construire" => {
                o.construire = false;
                continue;
            }
            _ => i.next(),
        };
        let Some(v) = valeur else { return usage() };
        let lu = match a.as_str() {
            "--navigateurs" => Navigateur::liste(v).map(|n| o.navigateurs = n),
            "--mesures" => etalonner::Mesure::liste(v).map(|m| o.mesures = m),
            "--donnees" => {
                o.donnees = PathBuf::from(v);
                Ok(())
            }
            "--modules" => {
                o.modules = v.split(',').map(String::from).collect();
                Ok(())
            }
            "--passes" => v.parse().map(|n| o.passes = n).map_err(|e| e.to_string()),
            "--tours" => v.parse().map(|n| o.tours = n).map_err(|e| e.to_string()),
            "--fois" => v.parse().map(|n| o.fois = n).map_err(|e| e.to_string()),
            "--budget" => v.parse().map(|n| o.budget = n).map_err(|e| e.to_string()),
            "--etiquette" => {
                o.etiquette = Some(v.clone());
                Ok(())
            }
            "--fautif" => v
                .strip_prefix(rejouer::FAUSSER_APRES)
                .ok_or_else(|| format!("fautif inconnu : {v}"))
                .and_then(|n| {
                    n.parse()
                        .map_err(|e| format!("fautif illisible : {v} ({e})"))
                })
                .map(|n| o.fautif = Some(n)),
            _ => Err(format!("option inconnue : {a}")),
        };
        if let Err(e) = lu {
            eprintln!("{e}");
            return usage();
        }
    }
    let passages = match etalonner::jouer(&o) {
        Ok(p) => p,
        Err(Arret::Echec(e)) => {
            println!("etalonner : échec\n{e}");
            return ExitCode::from(1);
        }
        Err(Arret::PasExecute(e)) => {
            println!("etalonner : pas exécuté\n{e}");
            return ExitCode::from(2);
        }
    };
    let (texte, resume) = etalonner::analyser(&o, &passages);
    println!("{texte}");
    // Une mesure n'est pas un jugement : le code dit si tout a été mesuré. Mais un
    // défaut (un écart avec wasmtime, D32 ; `PHASE_3` qui laisse la pile native céder
    // d'abord, § 8.7) en est un, et l'emporte, comme un échec de `rejouer`.
    // Sous `--fautif`, un contrôle négatif qui n'a pas vu son écart est une panne de
    // l'outil : le code 2, jamais le 1 d'un contrôle réussi.
    let code = etalonner::code_de_sortie(&resume, &passages);
    let compte = |cle: &str| {
        resume
            .get(cle)
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len)
    };
    if compte("controle_rate") > 0 {
        println!(
            "etalonner : panne de l'outil, le contrôle négatif a raté {} fois (voir « CONTRÔLE NÉGATIF RATÉ » plus haut)",
            compte("controle_rate")
        );
    } else if code == 1 {
        println!(
            "etalonner : {} défauts (voir « DÉFAUTS » plus haut)",
            compte("defauts")
        );
    }
    ExitCode::from(code)
}

/// `soiree-native` (lot 5) : une soirée par le vrai relais, l'autorité et des téléphones
/// natifs, sous pannes.
#[expect(
    clippy::too_many_lines,
    reason = "la lecture des options d'une commande, une par une"
)]
fn soiree_native(args: &[String]) -> ExitCode {
    let mut module = None;
    let mut o = soiree_native::Options {
        module: PathBuf::new(),
        telephones: None,
        duree_s: 120,
        graine: 1,
        pannes: jeu_abi::Oui::Oui,
        tuer_le_relais: 0,
        partie_entiere: jeu_abi::Oui::Non,
        relais: soiree_native::SorteRelais::DansLeProcessus,
        travail: PathBuf::new(),
        calme_max_s: 60,
    };
    let mut relais = std::env::var_os("SOIREE_RELAIS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|e| e.parent().map(|d| d.join("soiree-relais")))
        });
    let mut travail = None;
    let mut i = args.iter();
    while let Some(a) = i.next() {
        let nombre = |v: Option<&String>| v.and_then(|x| x.parse::<u64>().ok());
        match a.as_str() {
            "--telephones" => match nombre(i.next()) {
                Some(n) => o.telephones = usize::try_from(n).ok(),
                None => return usage(),
            },
            "--duree" => match nombre(i.next()) {
                Some(n) => o.duree_s = n,
                None => return usage(),
            },
            "--graine" => match nombre(i.next()) {
                Some(n) => o.graine = n,
                None => return usage(),
            },
            "--calme" => match nombre(i.next()) {
                Some(n) => o.calme_max_s = n,
                None => return usage(),
            },
            "--tuer-le-relais" => match nombre(i.next()).and_then(|n| u32::try_from(n).ok()) {
                Some(n) => o.tuer_le_relais = n,
                None => return usage(),
            },
            "--sans-pannes" => o.pannes = jeu_abi::Oui::Non,
            "--partie-entiere" => o.partie_entiere = jeu_abi::Oui::Oui,
            "--relais" => match i.next() {
                Some(r) => relais = Some(PathBuf::from(r)),
                None => return usage(),
            },
            "--relais-interne" => relais = None,
            "--travail" => match i.next() {
                Some(t) => travail = Some(PathBuf::from(t)),
                None => return usage(),
            },
            _ if a.starts_with("--") || module.is_some() => return usage(),
            _ => module = Some(PathBuf::from(a)),
        }
    }
    let Some(module) = module else {
        return usage();
    };
    o.module = module;
    if let Some(r) = relais {
        if !r.exists() {
            eprintln!(
                "soiree-native : le relais {} n'existe pas (cargo build --release -p relais, ou --relais <binaire>)",
                r.display()
            );
            return ExitCode::from(2);
        }
        o.relais = soiree_native::SorteRelais::Processus(r);
    }
    o.travail = travail.unwrap_or_else(|| {
        let nom = o
            .module
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        banc::commun().join(format!(
            "target/travail/soiree-native/{nom}-graine{}-{}",
            o.graine,
            std::process::id()
        ))
    });
    println!(
        "soiree-native : {} ; travail dans {}",
        o.module.display(),
        o.travail.display()
    );
    match soiree_native::jouer(&o) {
        Ok(b) => {
            print!("{b}");
            if b.fautes.is_empty() {
                println!("réussie");
                ExitCode::SUCCESS
            } else {
                for f in &b.fautes {
                    println!("ÉCHEC : {f}");
                }
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("soiree-native : panne de l'outil : {e}");
            ExitCode::from(2)
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.split_first() {
        Some((c, reste)) if c == "machine" => machine(reste),
        Some((c, reste)) if c == "rejouer" => rejouer_e20(reste),
        Some((c, reste)) if c == "etalonner" => etalonner_(reste),
        Some((c, reste)) if c == "regles-web" && reste.is_empty() => regles_web(),
        Some((c, reste)) if c == "soiree-native" => soiree_native(reste),
        _ => usage(),
    }
}
