//! L'épreuve qui part des sources (CONTRAT, « L'épreuve », étape 1 ; RESTE-PHASE-1,
//! points 12 et 13) : `soiree-epreuve <dossier>` compile lui-même le module et la copie
//! native, vérifie les règles d'écriture, puis joue ; le rapport nomme chaque étape.
//!
//! Les compilations de ces tests vont dans `target/verif-sources` : `cargo test` tient
//! le verrou de `target/`, et un `cargo` lancé dans le même dossier l'attendrait sans
//! fin (comme `tests/epreuve/modules.rs`). La première fois, la copie native y compile
//! l'épreuve entière (wasmtime compris) : quelques minutes.

#![expect(
    clippy::expect_used,
    reason = "code de test : un échec s'y lit comme l'échec du test qui l'appelle"
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// La racine du workspace du commun.
fn racine() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `soiree-epreuve` sur ce dossier, sans réseau, compilé dans `target/verif-sources`.
fn eprouver(dossier: &Path, options: &[&str], json: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_soiree-epreuve"))
        .arg(dossier)
        .arg("--cible")
        .arg(racine().join("target/verif-sources"))
        .args(options)
        .arg("--json")
        .arg(json)
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("lancer soiree-epreuve")
}

/// Une crate de module qui fait plus que `jeu::module!` : l'étape 1 échoue avant de
/// compiler quoi que ce soit, rien n'est joué, et le code de sortie vaut 1 (le jeu
/// échoue). Le sceau de la colle (`colle::Hote`) ne tient que pour une crate de module
/// réduite à la macro (RESTE-PHASE-1, point 10).
#[test]
fn une_crate_de_module_qui_fait_plus_echoue_l_etape_1() {
    let d = std::env::temp_dir().join(format!("soiree-sources-{}", std::process::id()));
    let jeu = std::fs::canonicalize(racine().join("crates/jeu")).expect("la crate jeu");
    let ecrire = |chemin: &str, texte: &str| {
        let f = d.join(chemin);
        std::fs::create_dir_all(f.parent().expect("un dossier")).expect("dossier");
        std::fs::write(f, texte).expect("écrire");
    };
    let dep_jeu = format!("jeu = {{ path = {:?} }}\n", jeu.display().to_string());
    ecrire(
        "Cargo.toml",
        "[workspace]\nresolver = \"3\"\nmembers = [\"regles\", \"module\"]\n",
    );
    ecrire(
        "regles/Cargo.toml",
        &format!(
            "[package]\nname = \"regles\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\n{dep_jeu}"
        ),
    );
    ecrire("regles/src/lib.rs", "#![no_std]\n#![forbid(unsafe_code)]\n");
    ecrire(
        "module/Cargo.toml",
        &format!(
            "[package]\nname = \"regles-module\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n\n[dependencies]\n{dep_jeu}regles = {{ path = \"../regles\" }}\n"
        ),
    );
    ecrire(
        "module/src/lib.rs",
        "#![cfg_attr(target_arch = \"wasm32\", no_std)]\njeu::module!(regles::Jeu);\npub fn de_plus() {}\n",
    );
    let json = d.join("rapport.json");
    let sortie = eprouver(&d, &["--court"], &json);
    let texte = String::from_utf8_lossy(&sortie.stdout);
    assert_eq!(
        sortie.status.code(),
        Some(1),
        "{texte}\n{}",
        String::from_utf8_lossy(&sortie.stderr)
    );
    assert!(texte.contains("1. Compiler — échouée"), "{texte}");
    assert!(texte.contains("forme du module : échouée"), "{texte}");
    assert!(texte.contains("rien n'est joué"), "{texte}");
    let j = std::fs::read_to_string(&json).expect("le rapport JSON");
    assert!(j.contains("\"etapes\""), "{j}");
    std::fs::remove_dir_all(&d).expect("nettoyer");
}

/// `devine`, depuis ses sources, sur un plan réduit à E1 et E9 : l'étape 1 est faite
/// (règles, clippy, module, copie native écrite hors du dépôt), l'étape 2 aussi, avec la
/// copie native comparée ; les étapes 4 et 7 ne le sont pas, hors du plan : code 3.
#[test]
fn devine_depuis_ses_sources() {
    let d = std::env::temp_dir().join(format!("soiree-devine-{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("dossier");
    let json = d.join("rapport.json");
    let sortie = eprouver(
        &racine().join("crates/exemples/devine"),
        &["--court", "--controles", "E1,E9"],
        &json,
    );
    let texte = String::from_utf8_lossy(&sortie.stdout);
    assert_eq!(
        sortie.status.code(),
        Some(3),
        "{texte}\n{}",
        String::from_utf8_lossy(&sortie.stderr)
    );
    for attendu in [
        "1. Compiler — faite",
        "forme du module : faite",
        "sans unsafe : faite",
        "lints : faite",
        "profils : faite",
        "clippy.toml : faite",
        "allow : faite",
        // Revue de la phase 2, constats 1, 4 et 6.
        "rien hors de l'état : faite",
        "forme calculée : faite",
        "build.rs : faite",
        "lancés sur cette machine, sans borne, à la compilation : devine-module",
        "même commun : faite",
        "clippy : faite",
        "clippy wasm32 : faite",
        "module : faite",
        "copie native : faite",
        "2. Constater — faite",
        "4. Chasser — pas faite",
        "copie native comparée",
    ] {
        assert!(texte.contains(attendu), "{attendu} :\n{texte}");
    }
    let j = std::fs::read_to_string(&json).expect("le rapport JSON");
    assert!(j.contains("\"jeu\": \"devine::Devine\""), "{j}");
    std::fs::remove_dir_all(&d).expect("nettoyer");
}
