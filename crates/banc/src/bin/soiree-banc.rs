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
//! soiree-banc dessin <module.wasm>… [--navigateurs chrome,firefox,safari] [--fois <n>]
//!                    [--sans-charge] [--sortie <fichier.json>] [--delai <s>] [--sans-construire]
//! soiree-banc soiree-native <module.wasm> [--telephones N] [--duree S] [--graine G] [--sans-pannes]
//!                           [--tuer-le-relais N] [--partie-entiere] [--relais <binaire> | --relais-interne]
//!                           [--travail <dossier>] [--calme S]
//! soiree-banc soiree <module.wasm> [--navigateurs chrome,firefox] [--telephones N]
//!                    [--duree-max S] [--m12 TOURS] [--travail <dossier>] [--sans-construire]
//! soiree-banc arriere-plan <module.wasm> [--navigateurs chrome,firefox] [--cache S]
//!                          [--fois N] [--telephones N] [--travail <dossier>] [--sans-construire]
//! soiree-banc rejouer-garde <dossier de la garde> --cle <fichier de G> --module <module.wasm>
//!                           [--salle CODE] [--sortie <dossier>]
//! soiree-banc ecran tier-list <module.wasm> [--navigateurs chrome,firefox] [--sans-webgl2]
//!                             [--plein N] [--fautif activation-perdue] [--travail <dossier>]
//!                             [--sans-construire]
//! ```
//!
//! `ecran tier-list` : Tier list jouée sur la télé par les vraies entrées du navigateur
//! (lot 11, `banc::ecran`) : import, glisser, édition en place, rechargement, export et
//! sauvegarde captés, restauration, plateau plein de `--plein` images de bruit (M3, M8 ;
//! 0 pour s'en passer), dans chaque navigateur et dans Chrome avec WebGL2 forcé ;
//! `--fautif activation-perdue` est son contrôle négatif (la sauvegarde n'est pas captée).
//!
//! `soiree` : la vraie autorité dans le Worker d'un navigateur, l'écran rechargé, le
//! Worker tué, un second onglet, F8, la partie jusqu'au podium, chaque reprise et la
//! soirée entière jugées contre wasmtime (lot 10a, `banc::soiree`) ; `--m12` mesure la
//! reprise au pire moment. `arriere-plan` : l'onglet caché (§ 8.3). `rejouer-garde` : la
//! garde rejouée jusqu'à chaque marque F8 (§ 2.4, `banc::rejouer_garde`). Code 0 si tout
//! tient, 1 au moindre écart, 2 si rien n'a pu jouer.
//!
//! `machine` : les essais de `Machine` (lot 3a). `rejouer` : E20, que `soiree-epreuve
//! --navigateurs` lance sur le dossier qu'il écrit (lot 3b, `banc::rejouer`).
//! `etalonner` : M1, M10 et M9 dans chaque navigateur (lot 3c, `banc::etalonner`), sur
//! la capture du harnais (`mesures/navigateur/README.md`). `dessin` : les pires cas de chaque module dessinés par la télé, dans chaque
//! navigateur, avec WebGL2 forcé, la perte du GPU et le contrôle négatif (lot 9,
//! `banc::dessin`) ; code 0 si tout tient, 1 au moindre écart, 2 si rien n'a pu jouer
//! (Safari sans l'automatisation à distance est « pas exécuté » sans changer le code).
//! `soiree-native` : une soirée
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

use banc::dessin;
use banc::ecran;
use banc::etalonner;
use banc::machine::{self, Arret, Options};
use banc::navigateur::Navigateur;
use banc::rapport::Attendu;
use banc::rejouer;
use banc::soiree;
use banc::soiree_native;

fn usage() -> ExitCode {
    eprintln!(
        "usage : soiree-banc machine [--sans-construire] [--navigateur chrome|firefox|safari] [--executable <chemin>]\n        soiree-banc rejouer <dossier> [--navigateurs chrome,firefox,safari] [--fautif fausser-apres:<n>] [--sortie <fichier.json>] [--delai <s>] [--sans-construire]\n        soiree-banc etalonner [--navigateurs chrome,firefox,safari] [--mesures m1,m10,m9] [--donnees <dossier>] [--modules devine,…] [--passes <n>] [--tours <n>] [--fois <n>] [--budget <unités>] [--etiquette <nom>] [--sans-construire] [--fautif fausser-apres:<n>]\n        soiree-banc regles-web\n        soiree-banc dessin <module.wasm>… [--navigateurs chrome,firefox,safari] [--fois <n>] [--sans-charge] [--sortie <fichier.json>] [--delai <s>] [--sans-construire]\n        soiree-banc soiree-native <module.wasm> [--telephones N] [--duree S] [--graine G] [--sans-pannes] [--tuer-le-relais N] [--partie-entiere] [--relais <binaire> | --relais-interne] [--travail <dossier>] [--calme S]\n        soiree-banc soiree <module.wasm> [--navigateurs chrome,firefox] [--telephones N] [--duree-max S] [--m12 TOURS] [--travail <dossier>] [--sans-construire]\n        soiree-banc arriere-plan <module.wasm> [--navigateurs chrome,firefox] [--cache S] [--fois N] [--telephones N] [--travail <dossier>] [--sans-construire]\n        soiree-banc rejouer-garde <dossier de la garde> --cle <fichier de G> --module <module.wasm> [--salle CODE] [--sortie <dossier>]\n        soiree-banc ecran tier-list <module.wasm> [--navigateurs chrome,firefox] [--sans-webgl2] [--plein N] [--fautif activation-perdue] [--travail <dossier>] [--sans-construire]"
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

fn afficher_un_passage(p: &dessin::Passage) {
    let quoi = if p.sorte.fautif.is_empty() {
        format!("moteur {}", p.sorte.moteur)
    } else {
        format!(
            "contrôle négatif « {} » (moteur {})",
            p.sorte.fautif, p.sorte.moteur
        )
    };
    println!("\n== {} {} — {quoi}", p.navigateur.nom(), p.version);
    for c in &p.constats {
        println!("  {c}");
    }
    if let Some(t) = &p.trace {
        println!("  trace de Chrome (images de la composition, à constater) : {t}");
    }
    for c in &p.console {
        println!("  page : {c}");
    }
    match &p.verdict {
        Ok(v) => println!("verdict : réussi — {v}"),
        Err(e) if e.starts_with("pas exécuté") => println!("verdict : {e}"),
        Err(e) => println!("verdict : échec — {e}"),
    }
}

fn dessin_(args: &[String]) -> ExitCode {
    let mut o = dessin::Options::default();
    let mut sortie = None;
    let mut i = args.iter();
    while let Some(a) = i.next() {
        match a.as_str() {
            "--sans-construire" => o.construire = false,
            "--sans-charge" => o.charge = jeu_abi::Oui::Non,
            "--navigateurs" => match i.next().map(|n| Navigateur::liste(n)) {
                Some(Ok(n)) => o.navigateurs = n,
                Some(Err(e)) => {
                    eprintln!("{e}");
                    return usage();
                }
                None => return usage(),
            },
            "--fois" => match i.next().and_then(|d| d.parse::<u32>().ok()) {
                Some(n) if n > 0 => o.fois = n,
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
            _ if a.starts_with("--") => return usage(),
            _ => o.modules.push(PathBuf::from(a)),
        }
    }
    let (jeux, passages) = match dessin::jouer(&o) {
        Ok(x) => x,
        Err(Arret::Echec(e)) => {
            println!("dessin : échec\n{e}");
            return ExitCode::from(1);
        }
        Err(Arret::PasExecute(e)) => {
            println!("dessin : pas exécuté\n{e}");
            return ExitCode::from(2);
        }
    };
    for j in &jeux {
        println!(
            "jeu {} ({}) : {} scènes, {} points à relire, {} textes",
            j.nom,
            if j.empreinte.is_empty() {
                "synthétique"
            } else {
                j.empreinte.as_str()
            },
            j.scenes.len(),
            j.scenes.iter().map(|s| s.sondes.len()).sum::<usize>(),
            j.scenes.iter().map(|s| s.agencements.len()).sum::<usize>(),
        );
    }
    let (mut reussis, mut echecs, mut non): (u32, u32, u32) = (0, 0, 0);
    let mut sorties = Vec::new();
    for p in &passages {
        afficher_un_passage(p);
        match &p.verdict {
            Ok(_) => reussis = reussis.saturating_add(1),
            Err(e) if e.starts_with("pas exécuté") => non = non.saturating_add(1),
            Err(_) => echecs = echecs.saturating_add(1),
        }
        sorties.push(serde_json::json!({
            "navigateur": p.navigateur.nom(),
            "version": p.version,
            "moteur_demande": p.sorte.moteur,
            "fautif": p.sorte.fautif,
            "verdict": match &p.verdict { Ok(v) => serde_json::json!({ "reussi": v }), Err(e) => serde_json::json!({ "echec": e }) },
            "constats": p.constats,
            "trace": p.trace,
            "console": p.console,
            "rapport": p.rapport,
        }));
    }
    if let Some(s) = &sortie
        && let Err(e) = std::fs::write(s, serde_json::Value::from(sorties).to_string())
    {
        eprintln!("{} : {e}", s.display());
    }
    println!("\ndessin : {reussis} réussi(s), {echecs} échec(s), {non} pas exécuté(s)");
    if echecs > 0 {
        ExitCode::from(1)
    } else if reussis == 0 {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    }
}

/// `soiree` ou `arriere-plan`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scenario {
    Soiree,
    ArrierePlan,
}

fn soiree_(args: &[String], scenario: Scenario) -> ExitCode {
    let commun = banc::commun();
    let mut o = soiree::Options {
        module: PathBuf::new(),
        navigateurs: vec![Navigateur::Chrome, Navigateur::Firefox],
        telephones: None,
        construire: true,
        duree_max_s: 400,
        travail: commun.join("target/banc/soiree"),
        m12: 0,
        fois: 1,
        cache_s: if scenario == Scenario::ArrierePlan {
            360
        } else {
            0
        },
    };
    let mut i = args.iter();
    while let Some(a) = i.next() {
        match a.as_str() {
            "--sans-construire" => o.construire = false,
            "--navigateurs" => match i.next().map(|n| Navigateur::liste(n)) {
                Some(Ok(n)) => o.navigateurs = n,
                _ => return usage(),
            },
            "--telephones" => match i.next().and_then(|d| d.parse().ok()) {
                Some(n) => o.telephones = Some(n),
                None => return usage(),
            },
            "--duree-max" => match i.next().and_then(|d| d.parse().ok()) {
                Some(n) => o.duree_max_s = n,
                None => return usage(),
            },
            "--m12" if scenario == Scenario::Soiree => {
                match i.next().and_then(|d| d.parse().ok()) {
                    Some(n) => o.m12 = n,
                    None => return usage(),
                }
            }
            "--cache" if scenario == Scenario::ArrierePlan => {
                match i.next().and_then(|d| d.parse().ok()) {
                    Some(n) if n > 0 => o.cache_s = n,
                    _ => return usage(),
                }
            }
            "--fois" if scenario == Scenario::ArrierePlan => {
                match i.next().and_then(|d| d.parse().ok()) {
                    Some(n) if n > 0 => o.fois = n,
                    _ => return usage(),
                }
            }
            "--travail" => match i.next() {
                Some(d) => o.travail = PathBuf::from(d),
                None => return usage(),
            },
            _ if a.starts_with("--") || !o.module.as_os_str().is_empty() => return usage(),
            _ => o.module = PathBuf::from(a),
        }
    }
    if o.module.as_os_str().is_empty() {
        return usage();
    }
    let passages = match soiree::jouer(&o, &commun) {
        Ok(p) => p,
        Err(e) => {
            println!("soiree : l'outil n'a pas pu jouer : {e}");
            return ExitCode::from(2);
        }
    };
    let (mut reussis, mut echecs, mut non) = (0u32, 0u32, 0u32);
    for p in &passages {
        print!("{p}");
        if p.pas_execute.is_some() {
            non = non.saturating_add(1);
        } else if p.fautes.is_empty() {
            reussis = reussis.saturating_add(1);
        } else {
            echecs = echecs.saturating_add(1);
        }
    }
    println!(
        "
soiree : {reussis} réussi(s), {echecs} échec(s), {non} pas exécuté(s)"
    );
    if echecs > 0 {
        ExitCode::from(1)
    } else if reussis == 0 {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    }
}

fn ecran_(args: &[String]) -> ExitCode {
    let commun = banc::commun();
    let Some((jeu, args)) = args.split_first() else {
        return usage();
    };
    if jeu != "tier-list" {
        println!("ecran : seul « tier-list » a son scénario (lot 11)");
        return usage();
    }
    let mut o = ecran::Options {
        module: PathBuf::new(),
        navigateurs: vec![Navigateur::Chrome, Navigateur::Firefox],
        webgl2: true,
        fautif: None,
        images_plein: 64,
        construire: true,
        travail: commun.join("target/banc/ecran"),
    };
    let mut i = args.iter();
    while let Some(a) = i.next() {
        match a.as_str() {
            "--sans-construire" => o.construire = false,
            "--sans-webgl2" => o.webgl2 = false,
            "--navigateurs" => match i.next().map(|n| Navigateur::liste(n)) {
                Some(Ok(n)) => o.navigateurs = n,
                _ => return usage(),
            },
            "--plein" => match i.next().and_then(|d| d.parse().ok()) {
                Some(n) => o.images_plein = n,
                None => return usage(),
            },
            "--fautif" => match i.next().map(String::as_str) {
                Some("activation-perdue") => o.fautif = Some(String::from("activation-perdue")),
                _ => return usage(),
            },
            "--travail" => match i.next() {
                Some(d) => o.travail = PathBuf::from(d),
                None => return usage(),
            },
            _ if a.starts_with("--") || !o.module.as_os_str().is_empty() => return usage(),
            _ => o.module = PathBuf::from(a),
        }
    }
    if o.module.as_os_str().is_empty() {
        return usage();
    }
    let passages = match ecran::jouer(&o, &commun) {
        Ok(p) => p,
        Err(e) => {
            println!("ecran : l'outil n'a pas pu jouer : {e}");
            return ExitCode::from(2);
        }
    };
    let (mut reussis, mut echecs, mut non) = (0u32, 0u32, 0u32);
    for p in &passages {
        print!("{p}");
        if p.pas_execute.is_some() {
            non = non.saturating_add(1);
        } else if p.fautes.is_empty() {
            reussis = reussis.saturating_add(1);
        } else {
            echecs = echecs.saturating_add(1);
        }
    }
    println!("\necran tier-list : {reussis} réussi(s), {echecs} échec(s), {non} pas exécuté(s)");
    if echecs > 0 {
        ExitCode::from(1)
    } else if reussis == 0 {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    }
}

fn rejouer_garde_(args: &[String]) -> ExitCode {
    let (mut garde, mut cle, mut module, mut salle, mut sortie) = (None, None, None, None, None);
    let mut i = args.iter();
    while let Some(a) = i.next() {
        match a.as_str() {
            "--cle" => cle = i.next().map(PathBuf::from),
            "--module" => module = i.next().map(PathBuf::from),
            "--sortie" => sortie = i.next().map(PathBuf::from),
            "--salle" => match i.next().and_then(|c| liaison::CodeSalle::lire(c)) {
                Some(c) => salle = Some(c),
                None => return usage(),
            },
            _ if a.starts_with("--") || garde.is_some() => return usage(),
            _ => garde = Some(PathBuf::from(a)),
        }
    }
    let (Some(garde), Some(cle), Some(module)) = (garde, cle, module) else {
        return usage();
    };
    let sortie = sortie.unwrap_or_else(|| garde.join("marques"));
    let g = match banc::rejouer_garde::lire_la_cle(&cle) {
        Ok(g) => g,
        Err(e) => {
            println!("rejouer-garde : {e}");
            return ExitCode::from(2);
        }
    };
    let octets = match std::fs::read(&module) {
        Ok(o) => o,
        Err(e) => {
            println!("rejouer-garde : {} : {e}", module.display());
            return ExitCode::from(2);
        }
    };
    match banc::rejouer_garde::rejouer(&garde, salle, &g, &octets, &sortie) {
        Ok(r) => {
            print!("{r}");
            if r.marques.iter().all(|m| m.issue.is_ok()) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(e) => {
            println!("rejouer-garde : {e}");
            ExitCode::from(1)
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
        Some((c, reste)) if c == "dessin" => dessin_(reste),
        Some((c, reste)) if c == "soiree" => soiree_(reste, Scenario::Soiree),
        Some((c, reste)) if c == "arriere-plan" => soiree_(reste, Scenario::ArrierePlan),
        Some((c, reste)) if c == "rejouer-garde" => rejouer_garde_(reste),
        Some((c, reste)) if c == "ecran" => ecran_(reste),
        _ => usage(),
    }
}
