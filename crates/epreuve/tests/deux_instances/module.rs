//! Le module de `devine`, compilé pour `wasm32-unknown-unknown` avec le profil
//! `module`, puis chargé (réécrit, compilé, `Bonjour` et `Decrire` vérifiés) par
//! `moteur-wasmtime`.
//!
//! Il est compilé dans un dossier à part (`target/verif-module`), comme le fait
//! `devine/tests/module_wasm.rs` : `cargo test` tient le verrou de `target/`, et un
//! `cargo build` lancé dans le même dossier l'attendrait sans fin.

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use moteur_wasmtime::{ModuleCharge, Moteur, ReglagesMoteur};

/// Les octets du module de `devine`, tels que rustc et `wasm-ld` les livrent, avant
/// toute réécriture. Compilé une fois par binaire de test.
pub fn octets() -> &'static [u8] {
    static OCTETS: OnceLock<Vec<u8>> = OnceLock::new();
    OCTETS.get_or_init(|| {
        let racine = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
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
        std::fs::read(cible.join("wasm32-unknown-unknown/module/devine_module.wasm"))
            .expect("lire le module de devine")
    })
}

/// Le moteur de la partie (`ReglagesMoteur::PHASE_3`) et le module de `devine` qu'il a
/// chargé. Le moteur vit aussi longtemps que le module : son garde-fou mural sert
/// chaque instance.
pub fn charger() -> (Moteur, ModuleCharge) {
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).expect("le moteur démarre");
    let charge = moteur
        .charger(octets())
        .unwrap_or_else(|r| panic!("le moteur refuse le module de devine : {r}"));
    (moteur, charge)
}
