//! La ligne de commande de `soiree-epreuve` (§ 8.8 ; RESTE-PHASE-1, points 12 et 13).
//!
//! ```text
//! soiree-epreuve <dossier du jeu> [--module CRATE] [--cible DOSSIER] [options du plan]
//! soiree-epreuve --wasm <module.wasm> [options du plan]
//! soiree-epreuve <module.wasm> [options du plan]
//!
//! options du plan : [--graine HEX] [--soirees N] [--duree S] [--memoire S]
//!                   [--controles E1,E2,…] [--court] [--json CHEMIN]
//! ```
//!
//! **Depuis les sources** (un dossier) : l'étape 1 de l'épreuve ([`crate::sources`]) lit
//! le workspace du jeu, vérifie ses règles d'écriture, compile son module et sa copie
//! native hors de son dépôt, puis lance celle-ci, qui joue l'épreuve avec le constat
//! de l'étape 1 (`--sources <fichier>`, une option que seul ce binaire écrit).
//! `--module` choisit la crate de module d'un workspace qui en a plusieurs ; `--cible`,
//! le dossier de compilation (par défaut, celui du workspace du jeu).
//!
//! **Un module seul** (`--wasm`, ou un fichier) : l'épreuve le joue sans en voir les
//! sources ; le rapport le dit, et l'étape 1 n'est pas faite : le code de sortie n'est
//! jamais 0.
//!
//! Le rapport lisible va sur la sortie standard, le rapport JSON dans un fichier (par
//! défaut `<module>.epreuve.json`, à côté du module). Le code de sortie est celui de
//! [`crate::Rapport::code_de_sortie`] ; 64 pour une ligne de commande mal formée, 66
//! pour un module ou un dossier illisible, 70 pour un moteur qui ne démarre pas.
//!
//! Une crate de jeu peut encore appeler [`ligne_de_commande`] avec sa copie native,
//! sur un module : c'est ce que fait le binaire généré par l'étape 1.

use std::path::PathBuf;
use std::process::Command;

use jeu_abi::Duree;
use moteur_wasmtime::{Moteur, ReglagesMoteur};

use crate::dynamique::FabriqueDynamique;
use crate::etapes::Compilation;
use crate::plan::{IdControle, Plan};
use crate::rapport::Rapport;
use crate::sources::{self, Prepare};

const USAGE: &str = "usage : soiree-epreuve <dossier du jeu> [--module CRATE] [--cible DOSSIER] | --wasm <module.wasm> ; [--graine HEX] [--soirees N] [--duree S] [--memoire S] [--controles E1,E2,…] [--court] [--json CHEMIN]";

/// D'où vient le module.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    /// Un module déjà compilé.
    Wasm(PathBuf),
    /// Les sources d'un jeu.
    Dossier(sources::Demande),
}

/// Ce que dit la ligne de commande.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Demande {
    source: Source,
    plan: Plan,
    json: Option<PathBuf>,
    /// Le constat de l'étape 1, écrit par `soiree-epreuve <dossier>` pour la copie
    /// native qu'il lance.
    constat: Option<PathBuf>,
}

fn graine(hex: &str) -> Option<[u8; 32]> {
    let hex = hex.trim();
    if hex.len() > 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut g = [0u8; 32];
    let plein = format!("{hex:0>64}");
    for (k, o) in g.iter_mut().enumerate() {
        let d = plein.get(k.saturating_mul(2)..k.saturating_mul(2).saturating_add(2))?;
        *o = u8::from_str_radix(d, 16).ok()?;
    }
    Some(g)
}

fn lire(args: impl IntoIterator<Item = String>) -> Result<Demande, String> {
    let mut args = args.into_iter();
    let mut position = None;
    let mut wasm = None;
    let mut module = None;
    let mut cible = None;
    let mut constat = None;
    let mut plan = Plan::standard([0x5e; 32]);
    let mut json = None;
    let mut court = false;
    let mut reglages: Vec<(String, String)> = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--court" => court = true,
            "--graine" | "--soirees" | "--duree" | "--memoire" | "--controles" | "--json"
            | "--wasm" | "--module" | "--cible" | "--sources" => {
                let v = args
                    .next()
                    .ok_or_else(|| format!("{a} attend une valeur"))?;
                reglages.push((a, v));
            }
            "-h" | "--aide" | "--help" => return Err(String::from(USAGE)),
            _ if a.starts_with("--") => return Err(format!("option inconnue : {a}")),
            _ if position.is_none() => position = Some(PathBuf::from(a)),
            _ => return Err(format!("argument en trop : {a}")),
        }
    }
    if court {
        plan = Plan::court(plan.graine);
    }
    for (a, v) in reglages {
        match a.as_str() {
            "--graine" => {
                plan.graine = graine(&v).ok_or_else(|| format!("graine illisible : {v}"))?;
            }
            "--soirees" => {
                plan.soirees = v.parse().map_err(|_| format!("nombre illisible : {v}"))?;
            }
            "--duree" => {
                plan.duree = Duree::s(v.parse().map_err(|_| format!("durée illisible : {v}"))?);
            }
            "--memoire" => {
                plan.memoire = Duree::s(v.parse().map_err(|_| format!("durée illisible : {v}"))?);
            }
            "--controles" => {
                plan.controles = v
                    .split(',')
                    .map(|c| {
                        IdControle::depuis_nom(c).ok_or_else(|| format!("contrôle inconnu : {c}"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
            }
            "--wasm" => wasm = Some(PathBuf::from(v)),
            "--module" => module = Some(v),
            "--cible" => cible = Some(PathBuf::from(v)),
            "--sources" => constat = Some(PathBuf::from(v)),
            _ => json = Some(PathBuf::from(v)),
        }
    }
    let source = match (wasm, position) {
        (Some(_), Some(p)) => return Err(format!("argument en trop : {}", p.display())),
        (Some(w), None) => Source::Wasm(w),
        (None, Some(p)) if p.is_dir() => Source::Dossier(sources::Demande {
            dossier: p,
            module: module.take(),
            cible: cible.take(),
        }),
        (None, Some(p)) => Source::Wasm(p),
        (None, None) => return Err(String::from(USAGE)),
    };
    if matches!(source, Source::Wasm(_)) && (module.is_some() || cible.is_some()) {
        return Err(String::from(
            "--module et --cible ne valent que pour un dossier de jeu",
        ));
    }
    if matches!(source, Source::Dossier(_)) && constat.is_some() {
        return Err(String::from("--sources ne vaut que pour un module"));
    }
    Ok(Demande {
        source,
        plan,
        json,
        constat,
    })
}

/// Les options du plan, réécrites pour la copie native que lance l'étape 1.
fn options_du_plan(plan: &Plan) -> Vec<String> {
    vec![
        String::from("--graine"),
        crate::rapport::hex(&plan.graine),
        String::from("--soirees"),
        plan.soirees.to_string(),
        String::from("--duree"),
        (plan.duree.en_ms() / 1000).to_string(),
        String::from("--memoire"),
        (plan.memoire.en_ms() / 1000).to_string(),
        String::from("--controles"),
        plan.controles
            .iter()
            .map(|c| c.nom())
            .collect::<Vec<_>>()
            .join(","),
    ]
}

/// Écrit le rapport : le texte sur la sortie standard, le JSON dans son fichier.
fn rendre(rapport: &Rapport, json: &PathBuf) -> u8 {
    print!("{}", rapport.texte());
    if let Err(e) = std::fs::write(json, rapport.json()) {
        eprintln!("{} : {e}", json.display());
    } else {
        println!("Rapport JSON : {}", json.display());
    }
    rapport.code_de_sortie()
}

/// L'épreuve d'un module, avec la copie native si elle est donnée, et le constat de
/// l'étape 1 s'il y en a un.
fn eprouver_le_module(
    module: &PathBuf,
    natif: Option<&mut dyn FabriqueDynamique>,
    plan: &Plan,
    compilation: Option<Compilation>,
    json: &PathBuf,
) -> u8 {
    let octets = match std::fs::read(module) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{} : {e}", module.display());
            return 66;
        }
    };
    let moteur = match Moteur::new(ReglagesMoteur::PHASE_1) {
        Ok(m) => m,
        Err(p) => {
            eprintln!("le moteur ne démarre pas : {p}");
            return 70;
        }
    };
    let mut rapport = crate::eprouver_octets(&moteur, &octets, natif, plan);
    rapport.compilation = compilation;
    rendre(&rapport, json)
}

/// Lance l'épreuve depuis une ligne de commande (sans le nom du programme), avec la
/// copie native du jeu si elle est donnée. Rend le code de sortie.
pub fn ligne_de_commande(
    args: impl IntoIterator<Item = String>,
    natif: Option<&mut dyn FabriqueDynamique>,
) -> u8 {
    let d = match lire(args) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            return 64;
        }
    };
    match d.source {
        Source::Wasm(module) => {
            let compilation = match &d.constat {
                None => None,
                Some(f) => {
                    let Some(c) = std::fs::read_to_string(f)
                        .ok()
                        .as_deref()
                        .and_then(Compilation::lire)
                    else {
                        eprintln!("{} : constat de l'étape 1 illisible", f.display());
                        return 66;
                    };
                    Some(c)
                }
            };
            let json = d
                .json
                .unwrap_or_else(|| module.with_extension("epreuve.json"));
            eprouver_le_module(&module, natif, &d.plan, compilation, &json)
        }
        Source::Dossier(_) if natif.is_some() => {
            eprintln!(
                "une copie native donnée s'éprouve sur un module (--wasm) : c'est soiree-epreuve qui part des sources"
            );
            64
        }
        Source::Dossier(demande) => depuis_les_sources(&demande, &d.plan, d.json),
    }
}

/// `soiree-epreuve <dossier>` : l'étape 1, puis l'épreuve par la copie native compilée.
fn depuis_les_sources(demande: &sources::Demande, plan: &Plan, json: Option<PathBuf>) -> u8 {
    println!(
        "Étape 1 : le jeu de {} se compile depuis ses sources…",
        demande.dossier.display()
    );
    let prepare = match sources::preparer(demande) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return 66;
        }
    };
    match prepare {
        Prepare::SansModule { compilation } => {
            let rapport = crate::epreuve::sans_module(plan, compilation);
            let json = json.unwrap_or_else(|| demande.dossier.join("epreuve.json"));
            rendre(&rapport, &json)
        }
        Prepare::Pret {
            wasm,
            natif: None,
            compilation,
            ..
        } => {
            let json = json.unwrap_or_else(|| wasm.with_extension("epreuve.json"));
            eprouver_le_module(&wasm, None, plan, Some(compilation), &json)
        }
        Prepare::Pret {
            wasm,
            natif: Some(binaire),
            dossier_natif,
            compilation,
        } => {
            let constat = dossier_natif.join("constat-de-compilation.tsv");
            if let Err(e) = std::fs::write(&constat, compilation.ecrire()) {
                eprintln!("{} : {e}", constat.display());
                return 2;
            }
            let json = json.unwrap_or_else(|| wasm.with_extension("epreuve.json"));
            let statut = Command::new(&binaire)
                .arg(&wasm)
                .arg("--sources")
                .arg(&constat)
                .args(options_du_plan(plan))
                .arg("--json")
                .arg(&json)
                .status();
            match statut {
                Ok(s) => s.code().and_then(|c| u8::try_from(c).ok()).unwrap_or(2),
                Err(e) => {
                    eprintln!("{} : {e}", binaire.display());
                    2
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Source, graine, lire, options_du_plan};
    use crate::plan::{IdControle, Plan};

    #[test]
    fn une_ligne_complete() {
        let d = lire(
            [
                "m.wasm",
                "--graine",
                "ff",
                "--controles",
                "E1,e9",
                "--duree",
                "30",
                "--json",
                "r.json",
            ]
            .map(String::from),
        )
        .expect("lisible");
        assert_eq!(d.plan.controles, [IdControle::E1, IdControle::E9]);
        assert_eq!(d.plan.duree.en_ms(), 30_000);
        assert_eq!(d.plan.graine[31], 0xff);
        assert_eq!(d.json.as_deref().and_then(|j| j.to_str()), Some("r.json"));
        assert_eq!(d.source, Source::Wasm(std::path::PathBuf::from("m.wasm")));
    }

    /// Les options du plan, réécrites pour la copie native, redonnent le même plan.
    #[test]
    fn le_plan_se_transmet() {
        let mut p = Plan::court([7; 32]);
        p.controles = vec![IdControle::E4, IdControle::E19];
        let mut args = vec![String::from("m.wasm")];
        args.extend(options_du_plan(&p));
        assert_eq!(lire(args).expect("lisible").plan, p);
    }

    #[test]
    fn un_dossier_est_une_source() {
        let ici = env!("CARGO_MANIFEST_DIR").to_owned();
        let d = lire([ici.clone(), String::from("--module"), String::from("x")]).expect("lisible");
        assert!(matches!(d.source, Source::Dossier(s) if s.module.as_deref() == Some("x")));
        assert!(lire(["m.wasm", "--module", "x"].map(String::from)).is_err());
        assert!(lire([ici, String::from("--sources"), String::from("c")]).is_err());
    }

    #[test]
    fn des_lignes_mal_formees() {
        assert!(lire(["--duree".to_owned()]).is_err());
        assert!(lire(["m.wasm", "--inconnue"].map(String::from)).is_err());
        assert!(lire(["m.wasm", "--controles", "E99"].map(String::from)).is_err());
        assert_eq!(graine("zz"), None);
        let d = lire(["m.wasm".to_owned()]).expect("lisible");
        assert_eq!(d.json, None);
    }
}
