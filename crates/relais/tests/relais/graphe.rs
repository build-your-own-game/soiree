//! **C'est une forme, pas une promesse** (PHASE-3, § 2.5) : le relais ne dépend, dans le
//! commun, que de `liaison` (et de `jeu-abi`, qu'elle tire), et d'aucune crate de
//! chiffrement. Le test lit le graphe que Cargo dit (`cargo metadata`) ; son contrôle
//! négatif est la même fonction, sur un témoin qui tire `snow` (§ 8.3).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// Les crates de chiffrement qu'aucun chemin du relais ne tire, et la salle.
///
/// `chacha20` à part : `tungstenite` tire `rand` sans option, et `rand` 0.10 en fait son
/// générateur (le masque des trames d'un client). Il n'est permis que par ce chemin-là :
/// sans `rand`, le relais ne doit pas l'atteindre ([`PAR_LE_HASARD`]). Rien du chiffrement
/// des trames (`chacha20poly1305`, `poly1305`, `aead`, `curve25519-dalek`) n'est permis.
const INTERDITES: [&str; 18] = [
    "salle",
    "snow",
    "chacha20poly1305",
    "poly1305",
    "aead",
    "cipher",
    "curve25519-dalek",
    "x25519-dalek",
    "ed25519-dalek",
    "blake2",
    "blake3",
    "ring",
    "aes",
    "aes-gcm",
    "rustls",
    "openssl",
    "sodiumoxide",
    "orion",
];

/// Le flux de `chacha20`, permis seulement comme générateur de `rand` (voir
/// [`INTERDITES`]).
const PAR_LE_HASARD: (&str, &str) = ("chacha20", "rand");

/// Les crates du commun que le relais peut tirer.
const DU_COMMUN_PERMISES: [&str; 2] = ["liaison", "jeu-abi"];

fn metadata(manifeste: &Path) -> Value {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));
    let sortie = Command::new(cargo)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--offline",
            "--manifest-path",
        ])
        .arg(manifeste)
        .output()
        .unwrap();
    assert!(
        sortie.status.success(),
        "cargo metadata : {}",
        String::from_utf8_lossy(&sortie.stderr)
    );
    serde_json::from_slice(&sortie.stdout).unwrap()
}

/// Ce que tire `paquet`, par ses dépendances normales et de construction (celles du
/// binaire livré, pas celles de ses tests), transitivement : les noms des paquets.
fn tire(m: &Value, paquet: &str) -> BTreeSet<String> {
    tire_sans(m, paquet, &[])
}

/// [`tire`], sans passer par les paquets `sauf`.
fn tire_sans(m: &Value, paquet: &str, sauf: &[&str]) -> BTreeSet<String> {
    let noms: BTreeMap<&str, &str> = m["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["id"].as_str().unwrap(), p["name"].as_str().unwrap()))
        .collect();
    let noeuds: BTreeMap<&str, &Value> = m["resolve"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n["id"].as_str().unwrap(), n))
        .collect();
    let depart = noms.iter().find(|(_, n)| **n == paquet).map_or_else(
        || panic!("{paquet} n'est pas dans le graphe"),
        |(id, _)| *id,
    );
    let mut vus = BTreeSet::new();
    let mut a_voir = vec![depart];
    while let Some(id) = a_voir.pop() {
        for d in noeuds[id]["deps"].as_array().unwrap() {
            let livree = d["dep_kinds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|k| k["kind"].is_null() || k["kind"] == "build");
            let pkg = d["pkg"].as_str().unwrap();
            if sauf.contains(&noms[pkg]) {
                continue;
            }
            if livree && vus.insert(pkg) {
                a_voir.push(pkg);
            }
        }
    }
    vus.into_iter().map(|id| noms[id].to_owned()).collect()
}

/// Ce que le graphe de `paquet` a de défendu.
fn ecarts(m: &Value, paquet: &str) -> Vec<String> {
    let membres: BTreeSet<String> = m["workspace_members"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|id| {
            m["packages"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["id"] == *id)
                .map(|p| p["name"].as_str().unwrap().to_owned())
        })
        .collect();
    let mut e = Vec::new();
    let (flux, hasard) = PAR_LE_HASARD;
    if tire_sans(m, paquet, &[hasard]).contains(flux) {
        e.push(format!("{paquet} tire {flux} autrement que par {hasard}"));
    }
    for n in tire(m, paquet) {
        if INTERDITES.contains(&n.as_str()) {
            e.push(format!(
                "{paquet} tire {n}, une crate de chiffrement ou la salle"
            ));
        } else if membres.contains(&n) && n != paquet && !DU_COMMUN_PERMISES.contains(&n.as_str()) {
            e.push(format!(
                "{paquet} tire {n}, du commun, où seule liaison est permise"
            ));
        }
    }
    e
}

fn commun() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// P15 : le relais ne tire, dans le commun, que `liaison` (et `jeu-abi` par elle), et
/// aucune crate de chiffrement : il ne sait pas lire ce qu'il fait passer.
#[test]
fn le_relais_ne_tire_ni_la_salle_ni_le_chiffrement() {
    let m = metadata(&commun().join("Cargo.toml"));
    let e = ecarts(&m, "relais");
    assert!(e.is_empty(), "{}", e.join("\n"));
    let t = tire(&m, "relais");
    assert!(t.contains("liaison") && t.contains("tungstenite"), "{t:?}");
}

/// Le contrôle négatif : la même fonction, sur un témoin qui tire `snow`, le refuse en le
/// nommant, et ce qu'il tire de chiffrement avec lui, `chacha20` compris, qui n'y vient
/// pas par `rand`.
#[test]
fn le_temoin_qui_tire_snow_fait_echouer_le_test_du_graphe() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/temoins/tire-snow");
    let copie = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("relais-temoin-tire-snow");
    let _: std::io::Result<()> = std::fs::remove_dir_all(&copie);
    std::fs::create_dir_all(copie.join("src")).unwrap();
    std::fs::copy(source.join("Cargo.toml"), copie.join("Cargo.toml")).unwrap();
    std::fs::copy(source.join("src/lib.rs"), copie.join("src/lib.rs")).unwrap();
    let m = metadata(&copie.join("Cargo.toml"));
    let e = ecarts(&m, "relais-temoin");
    assert!(
        e.iter()
            .any(|l| l == "relais-temoin tire snow, une crate de chiffrement ou la salle"),
        "{e:?}"
    );
    assert!(
        e.iter().any(|l| l.contains("tire chacha20poly1305")),
        "{e:?}"
    );
    // `chacha20`, que `snow` tire par `chacha20poly1305` et non par `rand` : la seule
    // exception que le test admet ne s'étend pas à lui (vérification du lot 5).
    assert!(
        e.iter()
            .any(|l| l == "relais-temoin tire chacha20 autrement que par rand"),
        "{e:?}"
    );
}
