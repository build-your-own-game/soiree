//! L'étape 1 de l'épreuve : partir des sources d'un jeu (CONTRAT, « L'épreuve » ;
//! RESTE-PHASE-1, point 12).
//!
//! `soiree-epreuve <dossier>` lit le workspace du jeu (`cargo metadata`), y trouve sa
//! crate de module, vérifie les règles d'écriture ([`crate::ecriture`]), lance clippy,
//! compile le module (profil `module`, `wasm32-unknown-unknown`) et la copie native,
//! puis joue. Rien n'est demandé au jeu au-delà de ce qu'INTERFACE.md, § 2.1, lui
//! demande déjà.
//!
//! # La copie native, sans exemple écrit à la main
//!
//! La crate de module d'un jeu se réduit à `jeu::module!(<chemin du type du jeu>);` :
//! c'est elle qui nomme le type du jeu. L'épreuve lit ce chemin, puis écrit, **hors du
//! dépôt du jeu**, dans son dossier de compilation (`<cible>/soiree-epreuve/<module>/`),
//! un petit binaire de trois lignes qui donne `jeu_natif::FabriqueNative::<chemin>` à
//! [`crate::ligne_de_commande`]. Il dépend de la crate de règles du jeu par chemin, et
//! de `jeu-natif` et `epreuve` de ce commun-ci ; il part du `Cargo.lock` du jeu, pour
//! en garder les versions. Le jeu doit donc dépendre de ce même commun (la règle
//! « même commun ») : deux `jeu` différents ne s'accorderaient pas.
//!
//! Le binaire généré joue l'épreuve, et reçoit le constat de l'étape 1 par
//! `--sources <fichier>` : son rapport est celui de l'épreuve entière.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value as Json;

use crate::ecriture::{self, Attendu};
use crate::etapes::{Compilation, IssueEtape, Regle};

/// Les crates du commun : elles ne sont jamais jugées comme des crates du jeu.
const CRATES_DU_COMMUN: [&str; 9] = [
    "jeu",
    "jeu-abi",
    "jeu-derive",
    "jeu-natif",
    "partie",
    "reecriture",
    "moteur-commun",
    "moteur-wasmtime",
    "epreuve",
];

/// Le plus de lignes de la sortie d'une commande gardées dans le rapport.
const LIGNES_GARDEES: usize = 30;

/// Ce que l'étape 1 a préparé.
#[derive(Debug)]
pub(crate) enum Prepare {
    /// Le module s'est compilé ; la copie native aussi, si `natif` est là.
    Pret {
        wasm: PathBuf,
        natif: Option<PathBuf>,
        dossier_natif: PathBuf,
        compilation: Compilation,
    },
    /// Le module ne s'est pas compilé, ou rien n'a pu commencer : rien ne se joue.
    SansModule { compilation: Compilation },
}

/// Ce que la ligne de commande demande à l'étape 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Demande {
    /// Le dossier du jeu : la racine de son workspace, sa crate de règles ou sa crate de
    /// module.
    pub(crate) dossier: PathBuf,
    /// La crate de module, quand le workspace en a plusieurs.
    pub(crate) module: Option<String>,
    /// Le dossier de compilation ; par défaut, celui du workspace du jeu.
    pub(crate) cible: Option<PathBuf>,
}

/// Un paquet, tel que `cargo metadata` le décrit.
#[derive(Debug, Clone)]
struct Paquet {
    id: String,
    nom: String,
    manifeste: PathBuf,
    /// Le nom de sa bibliothèque, son fichier racine et ses sortes de crate.
    lib: Option<(String, PathBuf, Vec<String>)>,
    /// Ses dépendances : nom, sorte (`None` : normale).
    deps: Vec<(String, Option<String>)>,
    /// D'où Cargo le prend (`registry+…`, `git+…`) ; `None` : un chemin sur le disque.
    source: Option<String>,
    /// Sa version.
    version: String,
}

impl Paquet {
    fn dossier(&self) -> &Path {
        self.manifeste.parent().unwrap_or(Path::new("."))
    }

    fn cdylib(&self) -> bool {
        self.lib
            .as_ref()
            .is_some_and(|(_, _, sortes)| sortes.iter().any(|s| s == "cdylib"))
    }
}

/// Le workspace du jeu, lu par `cargo metadata`.
#[derive(Debug)]
struct Workspace {
    paquets: Vec<Paquet>,
    membres: Vec<String>,
    racine: PathBuf,
    cible: PathBuf,
    /// Les dépendances normales résolues de chaque paquet, par identifiant.
    graphe: Vec<(String, Vec<String>)>,
}

impl Workspace {
    fn paquet(&self, id: &str) -> Option<&Paquet> {
        self.paquets.iter().find(|p| p.id == id)
    }

    fn par_nom(&self, nom: &str) -> Option<&Paquet> {
        self.paquets.iter().find(|p| p.nom == nom)
    }

    fn membre(&self, p: &Paquet) -> bool {
        self.membres.contains(&p.id)
    }

    /// Les dépendances normales de ce paquet, transitives, lui compris.
    fn fermeture(&self, depart: &str) -> Vec<String> {
        let mut vus = vec![String::from(depart)];
        let mut k = 0usize;
        while let Some(id) = vus.get(k).cloned() {
            if let Some((_, deps)) = self.graphe.iter().find(|(i, _)| *i == id) {
                for d in deps {
                    if !vus.contains(d) {
                        vus.push(d.clone());
                    }
                }
            }
            k = k.saturating_add(1);
        }
        vus
    }
}

fn texte(v: &Json, cle: &str) -> Option<String> {
    v.get(cle).and_then(Json::as_str).map(String::from)
}

fn lire_workspace(octets: &[u8]) -> Result<Workspace, String> {
    let v: Json =
        serde_json::from_slice(octets).map_err(|e| format!("cargo metadata illisible : {e}"))?;
    let mut paquets = Vec::new();
    for p in v
        .get("packages")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        let lib = p
            .get("targets")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .find(|t| {
                t.get("kind").and_then(Json::as_array).is_some_and(|k| {
                    k.iter()
                        .any(|x| matches!(x.as_str(), Some("lib" | "cdylib" | "rlib")))
                })
            })
            .and_then(|t| {
                let sortes = t
                    .get("crate_types")
                    .and_then(Json::as_array)
                    .map(|l| {
                        l.iter()
                            .filter_map(Json::as_str)
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default();
                Some((
                    texte(t, "name")?,
                    PathBuf::from(texte(t, "src_path")?),
                    sortes,
                ))
            });
        let deps = p
            .get("dependencies")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .filter_map(|d| Some((texte(d, "name")?, texte(d, "kind"))))
            .collect();
        paquets.push(Paquet {
            id: texte(p, "id").ok_or("un paquet sans identifiant")?,
            nom: texte(p, "name").ok_or("un paquet sans nom")?,
            manifeste: PathBuf::from(texte(p, "manifest_path").ok_or("un paquet sans manifeste")?),
            lib,
            deps,
            source: texte(p, "source"),
            version: texte(p, "version").unwrap_or_default(),
        });
    }
    let membres = v
        .get("workspace_members")
        .and_then(Json::as_array)
        .map(|l| {
            l.iter()
                .filter_map(Json::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let graphe = v
        .get("resolve")
        .and_then(|r| r.get("nodes"))
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter_map(|n| {
            let id = texte(n, "id")?;
            let deps = n
                .get("deps")
                .and_then(Json::as_array)
                .into_iter()
                .flatten()
                .filter(|d| {
                    d.get("dep_kinds")
                        .and_then(Json::as_array)
                        .is_some_and(|k| k.iter().any(|x| x.get("kind").is_none_or(Json::is_null)))
                })
                .filter_map(|d| texte(d, "pkg"))
                .collect();
            Some((id, deps))
        })
        .collect();
    Ok(Workspace {
        paquets,
        membres,
        racine: PathBuf::from(texte(&v, "workspace_root").ok_or("pas de workspace_root")?),
        cible: PathBuf::from(texte(&v, "target_directory").ok_or("pas de target_directory")?),
        graphe,
    })
}

/// La commande `cargo` : celle qui a lancé l'épreuve s'il y en a une, sinon celle du
/// chemin. Les variables qui changeraient la compilation sans que le jeu le dise
/// (options du compilateur, profils, dossier de compilation) sont retirées.
fn cargo(dans: &Path) -> Command {
    let mut c = Command::new(std::env::var("CARGO").unwrap_or_else(|_| String::from("cargo")));
    c.current_dir(dans);
    for (cle, _) in std::env::vars_os() {
        let Some(cle) = cle.to_str() else {
            continue;
        };
        if matches!(
            cle,
            "RUSTFLAGS"
                | "CARGO_ENCODED_RUSTFLAGS"
                | "RUSTDOCFLAGS"
                | "CARGO_TARGET_DIR"
                | "CLIPPY_CONF_DIR"
        ) || cle.starts_with("CARGO_PROFILE_")
            || cle.starts_with("CARGO_BUILD_RUSTFLAGS")
        {
            c.env_remove(cle);
        }
    }
    c
}

/// Les dernières lignes d'une sortie.
fn fin_de(sortie: &[u8]) -> String {
    let t = String::from_utf8_lossy(sortie);
    let lignes: Vec<&str> = t.lines().filter(|l| !l.trim().is_empty()).collect();
    let debut = lignes.len().saturating_sub(LIGNES_GARDEES);
    lignes.get(debut..).unwrap_or(&[]).join("\n")
}

/// Lance une commande ; rend sa sortie standard, ou la fin de sa sortie d'erreur.
fn lancer(c: &mut Command) -> Result<Vec<u8>, String> {
    let sortie = c
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("lancer {} : {e}", c.get_program().display()))?;
    if sortie.status.success() {
        Ok(sortie.stdout)
    } else {
        let mut d = fin_de(&sortie.stderr);
        if d.is_empty() {
            d = fin_de(&sortie.stdout);
        }
        Err(d)
    }
}

/// Les fichiers `.rs` d'un paquet, hors de `target/`, des dossiers cachés et des
/// paquets imbriqués.
fn fichiers_rust(dossier: &Path) -> Vec<PathBuf> {
    let mut trouves = Vec::new();
    let mut a_voir = vec![dossier.to_path_buf()];
    while let Some(d) = a_voir.pop() {
        let Ok(entrees) = std::fs::read_dir(&d) else {
            continue;
        };
        let mut entrees: Vec<PathBuf> = entrees.filter_map(Result::ok).map(|e| e.path()).collect();
        entrees.sort();
        for p in entrees {
            let nom = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if p.is_dir() {
                if nom == "target" || nom.starts_with('.') || p.join("Cargo.toml").is_file() {
                    continue;
                }
                a_voir.push(p);
            } else if p.extension().is_some_and(|e| e == "rs") {
                trouves.push(p);
            }
        }
    }
    trouves.sort();
    trouves
}

/// Le `clippy.toml` que clippy lit pour ce paquet : le premier en remontant depuis son
/// dossier.
fn clippy_toml_de(dossier: &Path) -> Option<PathBuf> {
    let mut d = Some(dossier);
    while let Some(ici) = d {
        for nom in ["clippy.toml", ".clippy.toml"] {
            let f = ici.join(nom);
            if f.is_file() {
                return Some(f);
            }
        }
        d = ici.parent();
    }
    None
}

fn canon(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Le dossier des crates de ce commun-ci, celui de l'épreuve qui tourne.
fn crates_du_commun() -> PathBuf {
    canon(&Path::new(env!("CARGO_MANIFEST_DIR")).join(".."))
}

/// La crate de module du dossier demandé : le dossier lui-même s'il est une crate de
/// module ; la crate de module qui dépend de lui s'il est une crate de règles ; sinon
/// la seule crate de module du workspace, ou celle que `--module` nomme.
fn trouver_le_module<'w>(w: &'w Workspace, d: &Demande) -> Result<&'w Paquet, String> {
    let modules: Vec<&Paquet> = w
        .paquets
        .iter()
        .filter(|p| w.membre(p) && p.cdylib() && !CRATES_DU_COMMUN.contains(&p.nom.as_str()))
        .collect();
    if let Some(nom) = &d.module {
        return modules
            .iter()
            .find(|p| p.nom == *nom)
            .copied()
            .ok_or_else(|| {
                format!("--module {nom} : aucune crate de module de ce nom dans le workspace")
            });
    }
    let ici = canon(&d.dossier);
    let du_dossier = w.paquets.iter().find(|p| canon(p.dossier()) == ici);
    let candidats: Vec<&Paquet> = match du_dossier {
        Some(p) if p.cdylib() => vec![p],
        Some(p) => modules
            .iter()
            .filter(|m| m.deps.iter().any(|(n, k)| *n == p.nom && k.is_none()))
            .copied()
            .collect(),
        None => modules,
    };
    match candidats.as_slice() {
        [m] => Ok(m),
        [] => Err(String::from(
            "aucune crate de module (une bibliothèque `cdylib` qui écrit `jeu::module!`) dans ce dossier",
        )),
        plusieurs => Err(format!(
            "plusieurs crates de module : {} ; nommer la bonne avec --module",
            plusieurs
                .iter()
                .map(|p| p.nom.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn lire_fichier(p: &Path) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{} : {e}", p.display()))
}

/// Les règles qui se lisent sur les sources, sans rien compiler.
fn regles_des_sources(
    w: &Workspace,
    module: &Paquet,
    regles: &mut Vec<Regle>,
) -> Option<(Paquet, String)> {
    // La forme de la crate de module.
    let racine_module = module.lib.as_ref().map(|(_, src, _)| src.clone());
    let chemin = match racine_module.as_deref().map(lire_fichier) {
        Some(Ok(source)) => match ecriture::forme_du_module(&source) {
            Ok(c) => c,
            Err(e) => {
                regles.push(Regle::echouee(
                    "forme du module",
                    format!("{} : {e}", module.nom),
                ));
                return None;
            }
        },
        Some(Err(e)) => {
            regles.push(Regle::echouee("forme du module", e));
            return None;
        }
        None => {
            regles.push(Regle::echouee(
                "forme du module",
                format!("{} n'a pas de bibliothèque", module.nom),
            ));
            return None;
        }
    };
    let nom_lib = ecriture::crate_du_chemin(&chemin);
    let Some(regles_du_jeu) = w
        .paquets
        .iter()
        .find(|p| {
            p.lib.as_ref().is_some_and(|(l, _, _)| l == nom_lib)
                && module.deps.iter().any(|(n, _)| *n == p.nom)
        })
        .cloned()
    else {
        regles.push(Regle::echouee(
            "forme du module",
            format!(
                "`jeu::module!({chemin})` nomme la crate {nom_lib}, dont {} ne dépend pas",
                module.nom
            ),
        ));
        return None;
    };
    let autres: Vec<&str> = module
        .deps
        .iter()
        .filter(|(n, k)| k.as_deref() != Some("dev") && *n != "jeu" && *n != regles_du_jeu.nom)
        .map(|(n, _)| n.as_str())
        .collect();
    if autres.is_empty() {
        regles.push(Regle::faite(
            "forme du module",
            format!(
                "{} : `jeu::module!({chemin})` seul ; il ne dépend que de jeu et de {}",
                module.nom, regles_du_jeu.nom
            ),
        ));
    } else {
        regles.push(Regle::echouee(
            "forme du module",
            format!(
                "{} dépend aussi de {} : une crate de module ne dépend que de jeu et de la crate de règles",
                module.nom,
                autres.join(", ")
            ),
        ));
    }
    Some((regles_du_jeu, chemin))
}

/// Les crates par chemin qui entrent dans le module sans être ni du workspace du jeu ni
/// du commun : leurs règles d'écriture ne seraient lues par rien, et une seule d'entre
/// elles, avec `unsafe`, forge `Autorite` ou `colle::Hote` (revue de la phase 2,
/// constat 3). Une crate du jeu est un membre de son workspace.
fn hors_du_workspace<'w>(w: &'w Workspace, module: &Paquet) -> Vec<&'w Paquet> {
    // Le commun : les crates de ce nom, rangées à côté du `jeu` dont dépend le jeu (la
    // règle « même commun » vérifie que c'est celui de cette épreuve).
    let dossier_du_commun = w
        .par_nom("jeu")
        .and_then(|j| j.dossier().parent())
        .map(canon);
    let du_commun = |p: &Paquet| {
        CRATES_DU_COMMUN.contains(&p.nom.as_str())
            && p.dossier().parent().map(canon) == dossier_du_commun
    };
    w.fermeture(&module.id)
        .iter()
        .filter_map(|id| w.paquet(id))
        .filter(|p| p.source.is_none() && !w.membre(p) && !du_commun(p))
        .collect()
}

/// Le code tiers qui entre dans le module : les crates prises d'un registre ou d'un
/// dépôt git. Le CONTRAT le permet (« Le code tiers est permis ») ; l'épreuve ne le
/// juge pas, et le rapport le nomme.
fn code_tiers<'w>(w: &'w Workspace, module: &Paquet) -> Vec<&'w Paquet> {
    w.fermeture(&module.id)
        .iter()
        .filter_map(|id| w.paquet(id))
        .filter(|p| p.source.is_some())
        .collect()
}

/// Les crates du jeu qui entrent dans le module : les dépendances normales de la crate
/// de module, transitives, qui sont des paquets du workspace du jeu et pas du commun.
fn crates_du_jeu<'w>(w: &'w Workspace, module: &Paquet) -> Vec<&'w Paquet> {
    w.fermeture(&module.id)
        .iter()
        .filter_map(|id| w.paquet(id))
        .filter(|p| w.membre(p) && !CRATES_DU_COMMUN.contains(&p.nom.as_str()))
        .collect()
}

#[expect(
    clippy::too_many_lines,
    reason = "les règles se lisent d'un trait, dans l'ordre du rapport"
)]
fn regles_d_ecriture(
    w: &Workspace,
    module: &Paquet,
    regles_du_jeu: &Paquet,
    regles: &mut Vec<Regle>,
) {
    let crates = crates_du_jeu(w, module);
    let noms: Vec<&str> = crates.iter().map(|p| p.nom.as_str()).collect();
    // Le sceau de la colle (`colle::Hote`) ne tient que si aucune crate du jeu n'a de
    // code `unsafe` (RESTE-PHASE-1, point 10).
    let mut sans_unsafe = Vec::new();
    for p in &crates {
        if p.id == module.id {
            continue;
        }
        let Some((_, racine, _)) = &p.lib else {
            continue;
        };
        match lire_fichier(racine) {
            Ok(s) => {
                if !ecriture::a_l_attribut(&s, Attendu::InterditUnsafe) {
                    sans_unsafe.push(format!(
                        "{} : pas de #![forbid(unsafe_code)] dans {}",
                        p.nom,
                        racine.display()
                    ));
                }
                if p.id == regles_du_jeu.id && !ecriture::a_l_attribut(&s, Attendu::SansStd) {
                    sans_unsafe.push(format!("{} : pas de #![no_std] (INTERFACE, § 2.1)", p.nom));
                }
            }
            Err(e) => sans_unsafe.push(e),
        }
    }
    for p in hors_du_workspace(w, module) {
        sans_unsafe.push(format!(
            "{} ({}) : une crate par chemin hors du workspace du jeu, qu'aucune règle d'écriture ne lit ; en faire un membre du workspace",
            p.nom,
            p.dossier().display()
        ));
    }
    regles.push(if sans_unsafe.is_empty() {
        let tiers: Vec<String> = code_tiers(w, module)
            .iter()
            .map(|p| {
                let avec = fichiers_rust(p.dossier())
                    .iter()
                    .filter(|f| {
                        lire_fichier(f).is_ok_and(|t| {
                            ecriture::code_seul(&t)
                                .split(|x: char| !(x.is_alphanumeric() || x == '_'))
                                .any(|m| m == "unsafe")
                        })
                    })
                    .count();
                if avec == 0 {
                    format!("{} {} (sans unsafe)", p.nom, p.version)
                } else {
                    format!("{} {} (unsafe dans {avec} fichier(s))", p.nom, p.version)
                }
            })
            .collect();
        Regle::faite(
            "sans unsafe",
            format!(
                "#![forbid(unsafe_code)] en tête de {}{}",
                noms.iter()
                    .filter(|n| **n != module.nom)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", "),
                if tiers.is_empty() {
                    String::new()
                } else {
                    format!(
                        " ; code tiers, permis par le CONTRAT et que cette règle ne juge pas (le sceau de `Autorite` et de `colle::Hote` repose aussi sur lui) : {}",
                        tiers.join(", ")
                    )
                }
            ),
        )
    } else {
        Regle::echouee("sans unsafe", sans_unsafe.join("\n"))
    });

    // Les lints.
    let racine_manifeste = w.racine.join("Cargo.toml");
    let racine = lire_fichier(&racine_manifeste).and_then(|t| ecriture::table(&t));
    match (&ecriture::lints_du_commun(), &racine) {
        (Ok(reference), Ok(racine)) => {
            let mut ecarts = Vec::new();
            for p in &crates {
                match lire_fichier(&p.manifeste).and_then(|t| ecriture::table(&t)) {
                    Ok(m) => {
                        let (lints, d_ou) = ecriture::lints_du_paquet(&m, racine);
                        for e in ecriture::ecarts_de_lints(reference, &lints) {
                            ecarts.push(format!("{} ({d_ou}) : {e}", p.nom));
                        }
                    }
                    Err(e) => ecarts.push(e),
                }
            }
            regles.push(if ecarts.is_empty() {
                Regle::faite(
                    "lints",
                    format!("ceux du commun, ou plus stricts, pour {}", noms.join(", ")),
                )
            } else {
                Regle::echouee("lints", ecarts.join("\n"))
            });
        }
        (Err(e), _) | (_, Err(e)) => regles.push(Regle::pas_faite("lints", e.clone())),
    }

    // Les profils, et la configuration de Cargo propre au dépôt du jeu.
    match &racine {
        Ok(racine) => {
            let mut ecarts = ecriture::ecarts_de_profils(racine);
            for nom in ["config.toml", "config"] {
                let f = w.racine.join(".cargo").join(nom);
                if let Ok(t) = lire_fichier(&f) {
                    match ecriture::table(&t) {
                        Ok(c) => {
                            for e in ecriture::ecarts_de_configuration(&c) {
                                ecarts.push(format!(
                                    "{} écrit {e} : la compilation changerait sans que le manifeste le dise",
                                    f.display()
                                ));
                            }
                        }
                        Err(e) => ecarts.push(format!("{} : {e}", f.display())),
                    }
                }
            }
            regles.push(if ecarts.is_empty() {
                Regle::faite(
                    "profils",
                    format!(
                        "{} : overflow-checks en release et dans le profil module, panic = \"abort\", dépendances optimisées en debug",
                        racine_manifeste.display()
                    ),
                )
            } else {
                Regle::echouee("profils", ecarts.join("\n"))
            });
        }
        Err(e) => regles.push(Regle::pas_faite("profils", e.clone())),
    }

    // Le clippy.toml que clippy lira pour chaque crate du jeu.
    match ecriture::table(ecriture::CLIPPY_DU_COMMUN) {
        Ok(reference) => {
            let mut ecarts = Vec::new();
            let mut lus = Vec::new();
            for p in &crates {
                match clippy_toml_de(p.dossier()) {
                    None => ecarts.push(format!(
                        "{} : aucun clippy.toml au-dessus de lui ; clippy n'appliquerait aucune règle du commun (HashMap, horloge, libm)",
                        p.nom
                    )),
                    Some(f) => {
                        if !lus.contains(&f) {
                            match lire_fichier(&f).and_then(|t| ecriture::table(&t)) {
                                Ok(t) => {
                                    for e in ecriture::ecarts_de_clippy(&reference, &t) {
                                        ecarts.push(format!("{} : {e}", f.display()));
                                    }
                                }
                                Err(e) => ecarts.push(e),
                            }
                            lus.push(f);
                        }
                    }
                }
            }
            regles.push(if ecarts.is_empty() {
                Regle::faite(
                    "clippy.toml",
                    format!(
                        "identique à celui du commun, ou plus strict : {}",
                        lus.iter()
                            .map(|f| f.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                )
            } else {
                Regle::echouee("clippy.toml", ecarts.join("\n"))
            });
        }
        Err(e) => regles.push(Regle::pas_faite("clippy.toml", e)),
    }

    // Aucun allow, aucun expect sans raison, dans les fichiers des crates du jeu.
    let mut trouves = Vec::new();
    let mut lus = 0usize;
    for p in &crates {
        for f in fichiers_rust(p.dossier()) {
            lus = lus.saturating_add(1);
            match lire_fichier(&f) {
                Ok(s) => {
                    let lieu = if f
                        .strip_prefix(p.dossier())
                        .is_ok_and(|r| r.starts_with("tests"))
                    {
                        ecriture::Lieu::Tests
                    } else {
                        ecriture::Lieu::Code
                    };
                    for (ligne, quoi) in ecriture::assouplissements(&s, lieu) {
                        trouves.push(format!("{}:{ligne} : {quoi}", f.display()));
                    }
                }
                Err(e) => trouves.push(e),
            }
        }
    }
    regles.push(if trouves.is_empty() {
        Regle::faite(
            "allow",
            format!("aucun allow, sauf dans les tests avec sa raison, ni expect sans raison, dans {lus} fichier(s)"),
        )
    } else {
        Regle::echouee("allow", trouves.join("\n"))
    });

    // Rien hors de l'état, et aucune forme écrite à la main, dans le code des crates du
    // jeu (les tests n'entrent pas dans le module).
    let mut hors = Vec::new();
    let mut formes = Vec::new();
    for p in &crates {
        for f in fichiers_rust(p.dossier()) {
            if f.strip_prefix(p.dossier())
                .is_ok_and(|r| r.starts_with("tests"))
            {
                continue;
            }
            match lire_fichier(&f) {
                Ok(s) => {
                    for (ligne, quoi) in ecriture::hors_de_l_etat(&s) {
                        hors.push(format!("{}:{ligne} : {quoi}", f.display()));
                    }
                    for (ligne, quoi) in ecriture::formes_a_la_main(&s) {
                        formes.push(format!("{}:{ligne} : {quoi}", f.display()));
                    }
                }
                Err(e) => hors.push(e),
            }
        }
    }
    regles.push(if hors.is_empty() {
        Regle::faite(
            "rien hors de l'état",
            "aucun `static` ni `thread_local!` dans le code du jeu (INTERFACE, § 2.2)",
        )
    } else {
        Regle::echouee("rien hors de l'état", hors.join("\n"))
    });
    regles.push(if formes.is_empty() {
        Regle::faite(
            "forme calculée",
            "aucun `impl` de `Forme`, `Etat` ou `Message` écrit à la main, ni `jeu::cache` (CONTRAT : la projection se calcule)",
        )
    } else {
        Regle::echouee("forme calculée", formes.join("\n"))
    });
}

/// Ce que les `build.rs` des crates du jeu ont dit à Cargo, lu dans le fichier
/// `output` que Cargo garde de chaque exécution (revue de la phase 2, constat 6) :
/// rend les paquets dont un `build.rs` a tourné, et ce qui changerait la compilation.
fn directives_des_build_rs(
    w: &Workspace,
    module: &Paquet,
    sortie: &[u8],
) -> (Vec<String>, Vec<String>) {
    let crates = crates_du_jeu(w, module);
    let mut lances = Vec::new();
    let mut ecarts = Vec::new();
    for ligne in String::from_utf8_lossy(sortie).lines() {
        let Ok(m) = serde_json::from_str::<Json>(ligne) else {
            continue;
        };
        if m.get("reason").and_then(Json::as_str) != Some("build-script-executed") {
            continue;
        }
        let Some(p) = m
            .get("package_id")
            .and_then(Json::as_str)
            .and_then(|id| crates.iter().find(|p| p.id == id))
        else {
            continue;
        };
        if !lances.contains(&p.nom) {
            lances.push(p.nom.clone());
        }
        let Some(out) = texte(&m, "out_dir") else {
            ecarts.push(format!(
                "{} : Cargo ne dit pas où son build.rs écrit",
                p.nom
            ));
            continue;
        };
        let fichier = Path::new(&out).with_file_name("output");
        match lire_fichier(&fichier) {
            Ok(t) => {
                let du_module = if p.id == module.id {
                    jeu_abi::Oui::Oui
                } else {
                    jeu_abi::Oui::Non
                };
                for d in ecriture::directives_de_build(&t, du_module) {
                    ecarts.push(format!(
                        "{} : son build.rs dit {d} ; la compilation changerait sans que le manifeste le dise",
                        p.nom
                    ));
                }
            }
            Err(e) => ecarts.push(e),
        }
    }
    (lances, ecarts)
}

/// Pour quelle cible clippy lit le jeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pour {
    /// La copie native, et les tests (`--all-targets`).
    Natif,
    /// Le module (`--target wasm32-unknown-unknown`) : le code sous
    /// `cfg(target_arch = "wasm32")` aussi.
    Wasm,
}

/// Clippy, avec les règles du jeu, en erreur.
fn clippy(w: &Workspace, paquets: &[&str], cible: &Path, pour: Pour) -> Regle {
    let wasm = pour == Pour::Wasm;
    let nom = if wasm { "clippy wasm32" } else { "clippy" };
    let mut c = cargo(&w.racine);
    c.arg("clippy").env("CARGO_TARGET_DIR", cible);
    for p in paquets {
        c.args(["-p", p]);
    }
    if wasm {
        c.args(["--target", "wasm32-unknown-unknown"]);
    } else {
        c.arg("--all-targets");
    }
    c.args(["--", "-D", "warnings"]);
    match lancer(&mut c) {
        Ok(_) => Regle::faite(
            nom,
            format!(
                "cargo clippy -p {}{} -- -D warnings",
                paquets.join(" -p "),
                if wasm {
                    " --target wasm32-unknown-unknown"
                } else {
                    " --all-targets"
                }
            ),
        ),
        Err(e) => Regle::echouee(nom, e),
    }
}

/// Le chemin du fichier qu'un message `compiler-artifact` de Cargo nomme, pour ce
/// paquet et cette extension (ou l'exécutable).
fn artefact(sortie: &[u8], paquet_id: &str, extension: Option<&str>) -> Option<PathBuf> {
    let mut trouve = None;
    for ligne in String::from_utf8_lossy(sortie).lines() {
        let Ok(m) = serde_json::from_str::<Json>(ligne) else {
            continue;
        };
        if m.get("reason").and_then(Json::as_str) != Some("compiler-artifact")
            || m.get("package_id").and_then(Json::as_str) != Some(paquet_id)
        {
            continue;
        }
        match extension {
            None => {
                if let Some(e) = texte(&m, "executable") {
                    trouve = Some(PathBuf::from(e));
                }
            }
            Some(ext) => {
                for f in m
                    .get("filenames")
                    .and_then(Json::as_array)
                    .into_iter()
                    .flatten()
                {
                    if let Some(f) = f.as_str()
                        && Path::new(f).extension().is_some_and(|e| e == ext)
                    {
                        trouve = Some(PathBuf::from(f));
                    }
                }
            }
        }
    }
    trouve
}

/// Le module, compilé avec le profil `module` du jeu.
fn compiler_le_module(
    w: &Workspace,
    module: &Paquet,
    cible: &Path,
) -> Result<(PathBuf, Vec<u8>), String> {
    let mut c = cargo(&w.racine);
    c.args([
        "build",
        "-p",
        &module.nom,
        "--target",
        "wasm32-unknown-unknown",
        "--profile",
        "module",
    ])
    .args(["--message-format", "json-render-diagnostics"])
    .env("CARGO_TARGET_DIR", cible);
    let sortie = lancer(&mut c)?;
    let wasm = artefact(&sortie, &module.id, Some("wasm"))
        .ok_or_else(|| String::from("Cargo n'a nommé aucun fichier .wasm"))?;
    Ok((wasm, sortie))
}

/// Un texte TOML entre guillemets.
fn toml_texte(s: &str) -> String {
    toml::Value::String(String::from(s)).to_string()
}

/// Écrit et compile la copie native : un binaire hors du dépôt du jeu, qui donne
/// `FabriqueNative::<chemin>` à [`crate::ligne_de_commande`].
fn compiler_la_copie_native(
    w: &Workspace,
    module: &Paquet,
    regles_du_jeu: &Paquet,
    chemin: &str,
    dossier: &Path,
    cible: &Path,
) -> Result<PathBuf, String> {
    let commun = crates_du_commun();
    let src = dossier.join("src");
    std::fs::create_dir_all(&src).map_err(|e| format!("{} : {e}", src.display()))?;
    let nom = format!("natif-{}", module.nom);
    let manifeste = format!(
        "# Écrit par soiree-epreuve : la copie native de `{chemin}`, hors du dépôt du jeu.\n\
         # Il se réécrit à chaque épreuve ; ne pas le modifier.\n\
         [package]\nname = {}\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
         # Un workspace à lui seul : il est rangé dans le dossier de compilation du jeu.\n\
         [workspace]\n\n\
         [dependencies]\n\
         {} = {{ path = {} }}\n\
         jeu-natif = {{ path = {} }}\n\
         epreuve = {{ path = {} }}\n\n\
         # Comme un jeu : les dépendances optimisées en debug, le dépassement vérifié.\n\
         [profile.dev.package.\"*\"]\nopt-level = 3\n\n\
         [profile.release]\noverflow-checks = true\n",
        toml_texte(&nom),
        regles_du_jeu.nom,
        toml_texte(&regles_du_jeu.dossier().display().to_string()),
        toml_texte(&commun.join("jeu-natif").display().to_string()),
        toml_texte(&commun.join("epreuve").display().to_string()),
    );
    let principal = format!(
        "//! Écrit par soiree-epreuve : l'épreuve de `{chemin}` avec sa copie native.\n\n\
         #![forbid(unsafe_code)]\n\n\
         fn main() -> std::process::ExitCode {{\n    \
         let mut natif = jeu_natif::FabriqueNative::<{chemin}>::new();\n    \
         std::process::ExitCode::from(epreuve::ligne_de_commande(\n        \
         std::env::args().skip(1),\n        \
         Some(&mut natif),\n    \
         ))\n}}\n"
    );
    let ecrire =
        |f: PathBuf, t: &str| std::fs::write(&f, t).map_err(|e| format!("{} : {e}", f.display()));
    ecrire(dossier.join("Cargo.toml"), &manifeste)?;
    ecrire(src.join("main.rs"), &principal)?;
    // Les versions du jeu : son Cargo.lock, que Cargo complète sans réseau.
    let verrou = w.racine.join("Cargo.lock");
    if verrou.is_file() && !dossier.join("Cargo.lock").is_file() {
        let _: u64 = std::fs::copy(&verrou, dossier.join("Cargo.lock"))
            .map_err(|e| format!("{} : {e}", verrou.display()))?;
    }
    let mut c = cargo(dossier);
    c.args([
        "build",
        "--offline",
        "--message-format",
        "json-render-diagnostics",
    ])
    .env("CARGO_TARGET_DIR", cible);
    let sortie = lancer(&mut c)?;
    let id_prefixe = format!("path+file://{}#", canon(dossier).display());
    // L'identifiant du paquet généré, tel que Cargo l'écrit.
    String::from_utf8_lossy(&sortie)
        .lines()
        .filter_map(|l| serde_json::from_str::<Json>(l).ok())
        .filter(|m| m.get("reason").and_then(Json::as_str) == Some("compiler-artifact"))
        .filter(|m| {
            m.get("package_id")
                .and_then(Json::as_str)
                .is_some_and(|id| id.starts_with(&id_prefixe) || id.contains(&nom))
        })
        .find_map(|m| texte(&m, "executable").map(PathBuf::from))
        .ok_or_else(|| String::from("Cargo n'a nommé aucun exécutable pour la copie native"))
}

/// L'étape 1 : lit le workspace, vérifie les règles, compile le module et la copie
/// native. `Err` : le dossier n'est pas un projet Cargo lisible.
#[expect(
    clippy::too_many_lines,
    reason = "l'étape 1 se lit d'un trait, dans l'ordre du rapport : lire, juger, compiler"
)]
pub(crate) fn preparer(d: &Demande) -> Result<Prepare, String> {
    let dossier = canon(&d.dossier);
    let manifeste = dossier.join("Cargo.toml");
    if !manifeste.is_file() {
        return Err(format!("{} : pas de Cargo.toml", d.dossier.display()));
    }
    let mut c = cargo(&dossier);
    c.args(["metadata", "--format-version", "1", "--manifest-path"])
        .arg(&manifeste);
    let w = lire_workspace(&lancer(&mut c).map_err(|e| format!("cargo metadata : {e}"))?)?;
    let mut compilation = Compilation {
        dossier: d.dossier.display().to_string(),
        ..Compilation::default()
    };
    let module = match trouver_le_module(&w, d) {
        Ok(m) => m.clone(),
        Err(e) => {
            compilation
                .regles
                .push(Regle::echouee("crate de module", e));
            return Ok(Prepare::SansModule { compilation });
        }
    };
    compilation.module.clone_from(&module.nom);
    let Some((regles_du_jeu, chemin)) = regles_des_sources(&w, &module, &mut compilation.regles)
    else {
        return Ok(Prepare::SansModule { compilation });
    };
    compilation.jeu.clone_from(&chemin);
    regles_d_ecriture(&w, &module, &regles_du_jeu, &mut compilation.regles);

    // Le même commun : le `jeu` du jeu est celui de cette épreuve.
    let attendu = crates_du_commun().join("jeu");
    let meme = match w.par_nom("jeu") {
        Some(j) if canon(j.dossier()) == attendu => {
            Regle::faite("même commun", format!("jeu vient de {}", attendu.display()))
        }
        Some(j) => Regle::echouee(
            "même commun",
            format!(
                "le jeu dépend de jeu à {}, cette épreuve est celle de {} : la copie native ne se compilerait pas contre deux `jeu` ; lancer l'épreuve du commun dont le jeu dépend",
                j.dossier().display(),
                attendu.display()
            ),
        ),
        None => Regle::echouee("même commun", "le jeu ne dépend pas de la crate jeu"),
    };
    let meme_commun = meme.issue == IssueEtape::Faite;
    compilation.regles.push(meme);

    let cible = d.cible.clone().unwrap_or_else(|| w.cible.clone());
    let paquets = [regles_du_jeu.nom.as_str(), module.nom.as_str()];
    compilation
        .regles
        .push(clippy(&w, &paquets, &cible, Pour::Natif));
    compilation
        .regles
        .push(clippy(&w, &paquets, &cible, Pour::Wasm));

    let wasm = match compiler_le_module(&w, &module, &cible) {
        Ok((f, sortie)) => {
            compilation.regles.push(Regle::faite(
                "module",
                format!(
                    "cargo build -p {} --target wasm32-unknown-unknown --profile module : {}",
                    module.nom,
                    f.display()
                ),
            ));
            // Les `build.rs` du jeu ont déjà tourné sur cette machine (clippy, puis le
            // module) : l'épreuve ne les borne pas, mais juge ce qu'ils ont dit à Cargo.
            let (lances, ecarts) = directives_des_build_rs(&w, &module, &sortie);
            compilation.regles.push(if ecarts.is_empty() {
                Regle::faite(
                    "build.rs",
                    if lances.is_empty() {
                        String::from("aucun build.rs dans les crates du jeu")
                    } else {
                        format!(
                            "lancés sur cette machine, sans borne, à la compilation : {} ; ils n'ont rien dit à Cargo qui change la compilation, sauf le maximum de mémoire du module",
                            lances.join(", ")
                        )
                    },
                )
            } else {
                Regle::echouee("build.rs", ecarts.join("\n"))
            });
            f
        }
        Err(e) => {
            compilation.regles.push(Regle::echouee("module", e));
            return Ok(Prepare::SansModule { compilation });
        }
    };
    let dossier_natif = cible.join("soiree-epreuve").join(&module.nom);
    let natif = if meme_commun {
        match compiler_la_copie_native(
            &w,
            &module,
            &regles_du_jeu,
            &chemin,
            &dossier_natif,
            &cible.join("soiree-epreuve").join("cible"),
        ) {
            Ok(f) => {
                compilation.regles.push(Regle::faite(
                    "copie native",
                    format!(
                        "jeu_natif::FabriqueNative::<{chemin}>, écrite dans {} : {}",
                        dossier_natif.display(),
                        f.display()
                    ),
                ));
                Some(f)
            }
            Err(e) => {
                compilation.regles.push(Regle::echouee("copie native", e));
                None
            }
        }
    } else {
        compilation.regles.push(Regle::pas_faite(
            "copie native",
            "pas compilée : le jeu ne dépend pas de ce commun-ci",
        ));
        None
    };
    Ok(Prepare::Pret {
        wasm,
        natif,
        dossier_natif,
        compilation,
    })
}

#[cfg(test)]
mod tests {
    use super::lire_workspace;

    /// Un `cargo metadata` réduit : la crate de module, sa crate de règles, et `jeu`.
    const META: &str = r#"{
      "packages": [
        {"id": "m", "name": "x-module", "manifest_path": "/j/crates/x-module/Cargo.toml",
         "targets": [{"kind": ["cdylib"], "crate_types": ["cdylib"], "name": "x_module", "src_path": "/j/crates/x-module/src/lib.rs"}],
         "dependencies": [{"name": "jeu", "kind": null}, {"name": "x", "kind": null}]},
        {"id": "x", "name": "x", "manifest_path": "/j/crates/x/Cargo.toml",
         "targets": [{"kind": ["lib"], "crate_types": ["lib"], "name": "x", "src_path": "/j/crates/x/src/lib.rs"}],
         "dependencies": [{"name": "jeu", "kind": null}, {"name": "proptest", "kind": "dev"}]},
        {"id": "jeu", "name": "jeu", "manifest_path": "/s/crates/jeu/Cargo.toml", "targets": [], "dependencies": []}
      ],
      "workspace_members": ["m", "x"],
      "workspace_root": "/j",
      "target_directory": "/j/target",
      "resolve": {"nodes": [
        {"id": "m", "deps": [{"pkg": "x", "dep_kinds": [{"kind": null}]}, {"pkg": "jeu", "dep_kinds": [{"kind": null}]}]},
        {"id": "x", "deps": [{"pkg": "jeu", "dep_kinds": [{"kind": null}]}]}
      ]}
    }"#;

    #[test]
    fn le_workspace_se_lit() {
        let w = lire_workspace(META.as_bytes()).expect("lisible");
        let m = w.paquet("m").expect("le module");
        assert!(m.cdylib());
        let d = super::Demande {
            dossier: std::path::PathBuf::from("/ailleurs"),
            module: None,
            cible: None,
        };
        assert_eq!(
            super::trouver_le_module(&w, &d).map(|p| p.nom.as_str()),
            Ok("x-module")
        );
        let noms: Vec<&str> = super::crates_du_jeu(&w, m)
            .iter()
            .map(|p| p.nom.as_str())
            .collect();
        assert_eq!(noms, ["x-module", "x"]);
        let d = super::Demande {
            module: Some(String::from("y")),
            ..d
        };
        assert!(super::trouver_le_module(&w, &d).is_err());
        assert!(super::hors_du_workspace(&w, m).is_empty());
    }

    /// Revue de la phase 2, constat 3 : une crate par chemin hors du workspace du jeu,
    /// dont dépend la crate de règles, n'est lue par aucune règle d'écriture ; une
    /// seule fonction `unsafe` y forge `Autorite`. Elle se voit. Une crate d'un
    /// registre est du code tiers, nommé à part ; une crate du commun ne compte pas.
    #[test]
    fn une_crate_par_chemin_hors_du_workspace_se_voit() {
        let meta = META
            .replace(
                r#"{"id": "jeu", "name": "jeu","#,
                r#"{"id": "outil", "name": "outil", "manifest_path": "/ailleurs/outil/Cargo.toml", "source": null, "targets": [], "dependencies": []},
        {"id": "tiers", "name": "tiers", "version": "1.2.3", "manifest_path": "/registre/tiers/Cargo.toml", "source": "registry+https://github.com/rust-lang/crates.io-index", "targets": [], "dependencies": []},
        {"id": "jeu", "name": "jeu","#,
            )
            .replace(
                r#"{"id": "x", "deps": [{"pkg": "jeu", "dep_kinds": [{"kind": null}]}]}"#,
                r#"{"id": "x", "deps": [{"pkg": "jeu", "dep_kinds": [{"kind": null}]}, {"pkg": "outil", "dep_kinds": [{"kind": null}]}, {"pkg": "tiers", "dep_kinds": [{"kind": null}]}]}"#,
            );
        let w = lire_workspace(meta.as_bytes()).expect("lisible");
        let m = w.paquet("m").expect("le module");
        let hors: Vec<&str> = super::hors_du_workspace(&w, m)
            .iter()
            .map(|p| p.nom.as_str())
            .collect();
        assert_eq!(hors, ["outil"]);
        let tiers: Vec<&str> = super::code_tiers(&w, m)
            .iter()
            .map(|p| p.nom.as_str())
            .collect();
        assert_eq!(tiers, ["tiers"]);
    }
}
