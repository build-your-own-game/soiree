//! La ligne de commande de `soiree-epreuve` (§ 8.8).
//!
//! ```text
//! soiree-epreuve <module.wasm> [--graine HEX] [--soirees N] [--duree S]
//!                [--memoire S] [--controles E1,E2,…] [--court] [--json CHEMIN]
//! ```
//!
//! Le rapport lisible va sur la sortie standard, le rapport JSON dans un fichier
//! (par défaut `<module>.epreuve.json`). Le code de sortie est celui de
//! [`crate::Rapport::code_de_sortie`] ; 64 pour une ligne de commande mal formée, 66
//! pour un module illisible, 70 pour un moteur qui ne démarre pas.
//!
//! Le binaire ne connaît aucun jeu : il ne compare pas de copie native. Une crate de
//! jeu qui veut la comparer appelle [`ligne_de_commande`] avec la sienne, comme
//! l'exemple `devine` de cette crate (`cargo run -p epreuve --example devine`).

use std::path::PathBuf;

use jeu_abi::Duree;
use moteur_wasmtime::{Moteur, ReglagesMoteur};

use crate::dynamique::FabriqueDynamique;
use crate::plan::{IdControle, Plan};

const USAGE: &str = "usage : soiree-epreuve <module.wasm> [--graine HEX] [--soirees N] [--duree S] [--memoire S] [--controles E1,E2,…] [--court] [--json CHEMIN]";

/// Ce que dit la ligne de commande.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Demande {
    module: PathBuf,
    plan: Plan,
    json: PathBuf,
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
    let mut module = None;
    let mut plan = Plan::standard([0x5e; 32]);
    let mut json = None;
    let mut court = false;
    let mut reglages: Vec<(String, String)> = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--court" => court = true,
            "--graine" | "--soirees" | "--duree" | "--memoire" | "--controles" | "--json" => {
                let v = args
                    .next()
                    .ok_or_else(|| format!("{a} attend une valeur"))?;
                reglages.push((a, v));
            }
            "-h" | "--aide" | "--help" => return Err(String::from(USAGE)),
            _ if a.starts_with("--") => return Err(format!("option inconnue : {a}")),
            _ if module.is_none() => module = Some(PathBuf::from(a)),
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
            _ => json = Some(PathBuf::from(v)),
        }
    }
    let module = module.ok_or_else(|| String::from(USAGE))?;
    let json = json.unwrap_or_else(|| module.with_extension("epreuve.json"));
    Ok(Demande { module, plan, json })
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
    let octets = match std::fs::read(&d.module) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{} : {e}", d.module.display());
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
    let rapport = crate::eprouver_octets(&moteur, &octets, natif, &d.plan);
    print!("{}", rapport.texte());
    if let Err(e) = std::fs::write(&d.json, rapport.json()) {
        eprintln!("{} : {e}", d.json.display());
    } else {
        println!("Rapport JSON : {}", d.json.display());
    }
    rapport.code_de_sortie()
}

#[cfg(test)]
mod tests {
    use super::{graine, lire};
    use crate::plan::IdControle;

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
        assert_eq!(d.json.to_str(), Some("r.json"));
    }

    #[test]
    fn des_lignes_mal_formees() {
        assert!(lire(["--duree".to_owned()]).is_err());
        assert!(lire(["m.wasm", "--inconnue"].map(String::from)).is_err());
        assert!(lire(["m.wasm", "--controles", "E99"].map(String::from)).is_err());
        assert_eq!(graine("zz"), None);
        let d = lire(["m.wasm".to_owned()]).expect("lisible");
        assert_eq!(d.json.to_str(), Some("m.epreuve.json"));
    }
}
