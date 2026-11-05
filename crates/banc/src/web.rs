//! La règle « mêmes lints » du second workspace, `web/` (PHASE-3, P31 ; § 8.3).
//!
//! `web/` est à part pour que `cargo test --workspace` du commun ne compile pas ce qui
//! ne sert que dans le navigateur. Ses règles d'écriture n'en sont pas moins celles du
//! commun : ses `[workspace.lints]`, son `clippy.toml` et son `rust-toolchain.toml`
//! doivent être **égaux** à ceux de `soiree/` (comparés comme TOML, commentaires mis à
//! part), ses profils aussi, sauf [`PROFILS_DES_JEUX`], et chacun de ses membres écrire
//! `[lints] workspace = true`, rien d'autre. Un jeu
//! peut durcir les règles du commun (la règle de l'épreuve, `epreuve/src/ecriture.rs`) ;
//! `web/` est le commun, et ne le peut pas : une règle durcie là le serait aussi ici.
//!
//! La règle est tenue par `tests/web.rs`, sur `web/` et sur son témoin
//! (`web/temoins/lints-differents/`), qui doit la faire échouer.

use std::path::Path;

use toml::{Table, Value};

/// Les profils du commun qui ne servent qu'aux modules des jeux, et que `web/` ne
/// recopie pas : `module` (`panic = "abort"`, LTO), pour `cargo build --profile module`.
pub const PROFILS_DES_JEUX: [&str; 1] = ["module"];

fn lire(fichier: &Path) -> Result<Table, String> {
    let texte =
        std::fs::read_to_string(fichier).map_err(|e| format!("{} : {e}", fichier.display()))?;
    texte
        .parse::<Table>()
        .map_err(|e| format!("{} : {e}", fichier.display()))
}

fn sous<'t>(t: &'t Table, chemin: &[&str]) -> Option<&'t Value> {
    let (dernier, avant) = chemin.split_last()?;
    let mut ici = t;
    for c in avant {
        ici = ici.get(*c)?.as_table()?;
    }
    ici.get(*dernier)
}

/// Les différences de deux tables, clé par clé, en descendant dans les tables : ce qui
/// manque, ce qui diffère, ce qui est en plus.
pub fn differences(chemin: &str, commun: &Table, autre: &Table) -> Vec<String> {
    let mut d = Vec::new();
    for (cle, v) in commun {
        let ici = if chemin.is_empty() {
            cle.clone()
        } else {
            format!("{chemin}.{cle}")
        };
        match (v, autre.get(cle)) {
            (_, None) => d.push(format!("{ici} manque (le commun : {v})")),
            (Value::Table(a), Some(Value::Table(b))) => d.extend(differences(&ici, a, b)),
            (a, Some(b)) if a != b => d.push(format!("{ici} vaut {b} au lieu de {a}")),
            (_, Some(_)) => {}
        }
    }
    for (cle, v) in autre {
        if !commun.contains_key(cle) {
            let ici = if chemin.is_empty() {
                cle.clone()
            } else {
                format!("{chemin}.{cle}")
            };
            d.push(format!("{ici} = {v} : le commun ne le pose pas"));
        }
    }
    d
}

fn comparer(nom: &str, commun: Option<&Value>, autre: Option<&Value>) -> Vec<String> {
    match (commun, autre) {
        (Some(Value::Table(a)), Some(Value::Table(b))) => differences(nom, a, b),
        (Some(_), None) => vec![format!("{nom} manque")],
        (a, b) if a == b => Vec::new(),
        (a, b) => vec![format!("{nom} : {b:?} au lieu de {a:?}")],
    }
}

/// Ce qui, dans le workspace `web`, s'écarte des règles d'écriture du commun (`commun`,
/// le dossier `soiree/`). Vide si tout est égal ; un fichier qui manque, ou qui ne
/// se lit pas, est un écart parmi les autres.
pub fn ecarts(commun: &Path, web: &Path) -> Vec<String> {
    let mut e = Vec::new();
    let (cargo_commun, cargo_web) = match (
        lire(&commun.join("Cargo.toml")),
        lire(&web.join("Cargo.toml")),
    ) {
        (Ok(a), Ok(b)) => (a, b),
        (a, b) => {
            e.extend(a.err());
            e.extend(b.err());
            return e;
        }
    };
    e.extend(comparer(
        "workspace.lints",
        sous(&cargo_commun, &["workspace", "lints"]),
        sous(&cargo_web, &["workspace", "lints"]),
    ));
    let profils_communs = sous(&cargo_commun, &["profile"])
        .and_then(Value::as_table)
        .map(|t| {
            let mut t = t.clone();
            for p in PROFILS_DES_JEUX {
                t.remove(p);
            }
            Value::Table(t)
        });
    e.extend(comparer(
        "profile",
        profils_communs.as_ref(),
        sous(&cargo_web, &["profile"]),
    ));
    for f in ["clippy.toml", "rust-toolchain.toml"] {
        match (lire(&commun.join(f)), lire(&web.join(f))) {
            (Ok(a), Ok(b)) => e.extend(differences(f, &a, &b)),
            (a, b) => {
                e.extend(a.err());
                e.extend(b.err());
            }
        }
    }
    let membres = sous(&cargo_web, &["workspace", "members"]).and_then(Value::as_array);
    let Some(membres) = membres else {
        e.push(String::from("workspace.members manque"));
        return e;
    };
    for m in membres {
        let Some(m) = m.as_str() else {
            e.push(format!("membre illisible : {m}"));
            continue;
        };
        if m.contains('*') {
            e.push(format!(
                "{m} : un motif ; les membres de web/ se nomment un à un"
            ));
            continue;
        }
        match lire(&web.join(m).join("Cargo.toml")) {
            Err(x) => e.push(x),
            Ok(t) => {
                let lints = t.get("lints").and_then(Value::as_table);
                let attendu = Table::from_iter([(String::from("workspace"), Value::from(true))]);
                if lints != Some(&attendu) {
                    e.push(format!(
                        "{m} : [lints] doit être `workspace = true`, et rien d'autre (lu : {})",
                        lints.map_or_else(|| String::from("aucun"), ToString::to_string)
                    ));
                }
            }
        }
    }
    e
}

#[cfg(test)]
mod tests {
    use toml::Table;

    use super::differences;

    #[test]
    fn les_differences_descendent_dans_les_tables() {
        let a: Table = "[x]\na = 1\nb = \"deny\"\n[y]\nc = true".parse().unwrap();
        let b: Table = "[x]\na = 1\nb = \"allow\"\nd = 2".parse().unwrap();
        let d = differences("", &a, &b);
        assert_eq!(d.len(), 3, "{d:?}");
        assert!(d.iter().any(|l| l.starts_with("x.b vaut \"allow\"")));
        assert!(d.iter().any(|l| l.starts_with("y manque")));
        assert!(d.iter().any(|l| l.starts_with("x.d = 2")));
        assert_eq!(differences("", &a, &a), Vec::<String>::new());
    }
}
