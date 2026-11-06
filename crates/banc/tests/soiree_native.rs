//! Une soirée native par le vrai relais (PHASE-3, § 8.1, lot 5) : le relais de
//! `relais::coque` dans ce processus, l'autorité native sur `devine` dans wasmtime, trois
//! téléphones natifs sous les pannes du mandataire, de vraies WebSocket et le vrai
//! chiffrement. Le scénario `soiree-banc soiree-native` joue les trois jeux de même, avec
//! le binaire du relais, qu'il tue (`kill -9`).
//!
//! `cfg(test)` marque tout ce binaire comme du code de test : les fonctions d'aide y ont
//! droit à `unwrap`, `expect`, `panic!` et aux index (CONTRAT, « Les règles d'écriture »).

#![cfg(test)]

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use banc::soiree_native::{Options, SorteRelais, jouer};
use jeu_abi::Oui;

/// Le module de `devine`, compilé une fois, dans un dossier à part (`target/verif-module`) :
/// `cargo test` tient le verrou de `target/`.
fn devine() -> PathBuf {
    static CHEMIN: OnceLock<PathBuf> = OnceLock::new();
    CHEMIN
        .get_or_init(|| {
            let racine = banc::commun();
            let cible = racine.join("target/verif-module");
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));
            let statut = Command::new(cargo)
                .current_dir(&racine)
                .env("CARGO_TARGET_DIR", &cible)
                .args([
                    "build",
                    "-p",
                    "devine-module",
                    "--target",
                    "wasm32-unknown-unknown",
                    "--profile",
                    "module",
                ])
                .status()
                .expect("lancer cargo");
            assert!(
                statut.success(),
                "la compilation du module de devine a échoué"
            );
            cible.join("wasm32-unknown-unknown/module/devine_module.wasm")
        })
        .clone()
}

fn travail(nom: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("soiree-native-{nom}-{}", std::process::id()));
    let _: std::io::Result<()> = std::fs::remove_dir_all(&d);
    d
}

/// PHASE-3, § 2.5 et § 8.1 : **le relais ne voit aucun secret**. Une soirée sous pannes
/// par le vrai relais ; on cherche, dans tout ce qu'il a vu (son enregistrement, chaque
/// trame dans chaque sens) et dans sa garde sur disque, le secret d'entrée, la psk, G, la
/// clé privée de l'autorité, le jeton de chaque téléphone, chaque message de geste en
/// clair (`VersAutorite::Geste` jusqu'aux octets du geste : le prénom tapé de `devine`
/// y est) et chaque message de vue en clair (`VersAppareil::Vue`, entier), échantillonnés
/// sur les téléphones : rien ne doit s'y trouver. Les octets seuls d'un geste (8 octets
/// au moins) ou d'une vue (16 au moins) sont cherchés aussi, mais ceux de `devine` sont
/// plus courts : ce sont les messages qui portent la preuve. Le reste de la soirée doit
/// tenir aussi : la chaîne écrite, chaque geste une fois, tout accusé écrit, la garde
/// finale qui reprend la même partie.
///
/// Son contrôle négatif : le chiffrement des trames désactivé (le clair à la place du
/// chiffré, des deux côtés), qui doit faire échouer ce test, le relais voyant les
/// jetons, des messages de geste et des messages de vue
/// (`target/travail/lot-5/correction/controles.py`).
#[test]
fn le_relais_ne_voit_aucun_secret() {
    let o = Options {
        module: devine(),
        telephones: Some(3),
        duree_s: 15,
        graine: 5,
        pannes: Oui::Oui,
        tuer_le_relais: 0,
        partie_entiere: Oui::Non,
        relais: SorteRelais::DansLeProcessus,
        travail: travail("secrets"),
        calme_max_s: 30,
    };
    let b = jouer(&o).expect("l'outil a joué");
    eprintln!("{b}");
    let secrets: Vec<&String> = b
        .fautes
        .iter()
        .filter(|f| f.starts_with("le relais voit un secret"))
        .collect();
    assert!(secrets.is_empty(), "{secrets:#?}");
    // La fouille a cherché : les quatre clés, un jeton par téléphone, des messages de geste
    // et de vue (vérification du lot 5 : sans eux, seuls les clés et les jetons l'étaient).
    let sorte = |k: &str| b.aiguilles.get(k).copied().unwrap_or(0);
    assert_eq!(sorte("cles"), 4, "{:?}", b.aiguilles);
    assert!(sorte("jetons") >= 3, "{:?}", b.aiguilles);
    assert!(sorte("messages de geste") >= 1, "{:?}", b.aiguilles);
    assert!(sorte("messages de vue") >= 1, "{:?}", b.aiguilles);
    assert!(b.secrets.1 > 0);
    assert!(b.fautes.is_empty(), "{:#?}", b.fautes);
    assert!(b.historique.cases > 0 && b.gestes_emis > 0, "{b}");
}
