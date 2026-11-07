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

/// Les crates tierces dont `krate`, une crate du commun, dépend pour de bon : ses
/// `[dependencies]`, et celles des crates du commun qu'elle y nomme, en descendant ; ni
/// ses dépendances de développement, ni celles des autres (un `Cargo.lock` ne les
/// distingue pas : il faut les manifestes).
pub fn dependances_tierces(soiree: &Path, krate: &str) -> Result<Vec<String>, String> {
    let espace = lire(&soiree.join("Cargo.toml"))?;
    let locales = sous(&espace, &["workspace", "dependencies"])
        .and_then(Value::as_table)
        .cloned()
        .unwrap_or_default();
    let chemin = |nom: &str| {
        locales
            .get(nom)
            .and_then(|v| v.get("path"))
            .and_then(Value::as_str)
            .map(|p| soiree.join(p))
    };
    let mut tierces = Vec::new();
    let mut vues = Vec::new();
    let mut a_voir = vec![String::from(krate)];
    while let Some(k) = a_voir.pop() {
        if vues.contains(&k) {
            continue;
        }
        let dossier = chemin(&k).ok_or_else(|| format!("{k} n'est pas une crate du commun"))?;
        let manifeste = lire(&dossier.join("Cargo.toml"))?;
        for nom in manifeste
            .get("dependencies")
            .and_then(Value::as_table)
            .into_iter()
            .flat_map(|t| t.keys())
        {
            if chemin(nom).is_some() {
                a_voir.push(nom.clone());
            } else if !tierces.contains(nom) {
                tierces.push(nom.clone());
            }
        }
        vues.push(k);
    }
    Ok(tierces)
}

/// La télé dessine ce que l'épreuve mesure, par le même code (PHASE-3, § 2.9, lot 9) :
/// chaque paquet que tirent les crates tierces `racines` (celles de `mise-en-page`, par
/// [`dependances_tierces`]) dans le `Cargo.lock` du commun, s'il est aussi dans celui de
/// `web/`, doit y être à la même version. Rend les écarts, un par paquet.
/// `lock_commun` et `lock_web` sont les textes des deux `Cargo.lock`. Ce que la règle ne
/// voit pas : les fonctionnalités (`web/` donne à `cosmic-text` `swash` et `fontconfig`,
/// que `glyphon` demande ; ni l'une ni l'autre ne touche à la mise en forme : `swash`
/// dessine les glyphes, `fontconfig` lit les polices du système, que la mesure ne lit
/// jamais).
pub fn ecarts_de_versions(lock_commun: &str, lock_web: &str, racines: &[String]) -> Vec<String> {
    let lire_lock = |texte: &str| -> Result<Vec<(String, String, Vec<String>)>, String> {
        let t = texte.parse::<Table>().map_err(|e| e.to_string())?;
        let mut paquets = Vec::new();
        for p in t
            .get("package")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let champ = |c: &str| p.get(c).and_then(Value::as_str).unwrap_or_default();
            let deps = p
                .get("dependencies")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect();
            paquets.push((
                String::from(champ("name")),
                String::from(champ("version")),
                deps,
            ));
        }
        Ok(paquets)
    };
    let (commun, web) = match (lire_lock(lock_commun), lire_lock(lock_web)) {
        (Ok(c), Ok(w)) => (c, w),
        (Err(e), _) | (_, Err(e)) => return vec![format!("Cargo.lock illisible : {e}")],
    };
    // Une dépendance s'écrit « nom » ou « nom version », suivi peut-être de sa source.
    let trouver = |dep: &str| {
        let mut mots = dep.split(' ');
        let nom = mots.next().unwrap_or_default();
        let version = mots.next();
        commun
            .iter()
            .find(|(n, v, _)| n == nom && version.is_none_or(|x| x == v))
    };
    let mut vus: Vec<(String, String)> = Vec::new();
    let mut a_voir: Vec<String> = racines.to_vec();
    while let Some(dep) = a_voir.pop() {
        let Some((n, v, deps)) = trouver(&dep) else {
            continue;
        };
        if vus.iter().any(|(m, w)| m == n && w == v) {
            continue;
        }
        vus.push((n.clone(), v.clone()));
        a_voir.extend(deps.iter().cloned());
    }
    let mut ecarts = Vec::new();
    for (n, v) in &vus {
        let ailleurs: Vec<&str> = web
            .iter()
            .filter(|(m, _, _)| m == n)
            .map(|(_, w, _)| w.as_str())
            .collect();
        // Absent de `web/` : rien de lui n'y est compilé (une dépendance optionnelle
        // qu'un autre membre du commun active, comme `embedded-io` sous `postcard`).
        if !ailleurs.is_empty() && !ailleurs.contains(&v.as_str()) {
            ecarts.push(format!(
                "{n} est à {} dans web/, à {v} dans le commun",
                ailleurs.join(", ")
            ));
        }
    }
    ecarts
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
