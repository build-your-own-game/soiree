//! Les modules éprouvés : ceux de `devine`, d'`ardoise` et de `exemples/fautifs`, compilés
//! pour `wasm32-unknown-unknown` avec le profil `module`, en une seule commande, une
//! fois par binaire de test.
//!
//! Ils sont compilés dans un dossier à part (`target/verif-module`), comme le fait
//! `devine/tests/module_wasm.rs` : `cargo test` tient le verrou de `target/`, et un
//! `cargo build` lancé dans le même dossier l'attendrait sans fin.

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use moteur_wasmtime::{Moteur, ReglagesMoteur};

/// Les jeux dont le module se compile ici : `devine`, puis les crates de
/// `exemples/fautifs/modules/`.
pub const JEUX: [&str; 35] = [
    "devine",
    "ardoise",
    "sain",
    "declaration-incoherente",
    "jamais-fini",
    "phase-inaccessible",
    "invariant-viole",
    "attend-un-absent",
    "refuse-apres-avoir-ecrit",
    "vue-qui-ecrit",
    "vue-qui-fuit",
    "natif-different",
    "octet-qui-panique",
    "etat-hors-enveloppe",
    "fuit-la-memoire",
    "depend-de-l-appareil",
    "coupure-qui-compte",
    "avec-fenetre",
    "compte-le-recu",
    "pire-cas-qui-deborde",
    "nan-dans-l-etat",
    "gel-qui-enchaine",
    "boucle-au-vote",
    "phase-sans-cas",
    "invariant-jamais-evalue",
    "retient-un-refus",
    "choix-qui-fuit",
    "panique-au-temps",
    "juge-sur-le-fait",
    "fichier-qui-panique",
    "pire-cas-trop-lourd",
    "fin-trop-lourde",
    "zones-lourdes",
    "secret-dans-un-static",
    "pire-cas-trop-leger",
];

fn crate_de(nom: &str) -> String {
    if nom == "devine" || nom == "ardoise" {
        format!("{nom}-module")
    } else {
        format!("fautif-{nom}-module")
    }
}

/// Le dossier des modules compilés.
fn dossier() -> &'static PathBuf {
    static DOSSIER: OnceLock<PathBuf> = OnceLock::new();
    DOSSIER.get_or_init(|| {
        let racine = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cible = racine.join("target/verif-module");
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));
        let mut c = Command::new(cargo);
        c.current_dir(&racine)
            .env("CARGO_TARGET_DIR", &cible)
            .arg("build");
        for j in JEUX {
            c.args(["-p", &crate_de(j)]);
        }
        let statut = c
            .args(["--target", "wasm32-unknown-unknown", "--profile", "module"])
            .status()
            .expect("lancer cargo");
        assert!(statut.success(), "la compilation des modules a échoué");
        cible.join("wasm32-unknown-unknown/module")
    })
}

/// Le moteur de la phase 1 et les octets du module de ce jeu, tels que rustc et
/// `wasm-ld` les livrent.
pub fn module(nom: &str) -> (Moteur, Vec<u8>) {
    assert!(JEUX.contains(&nom), "jeu inconnu : {nom}");
    let fichier = dossier().join(format!("{}.wasm", crate_de(nom).replace('-', "_")));
    let octets =
        std::fs::read(&fichier).unwrap_or_else(|e| panic!("lire {} : {e}", fichier.display()));
    let moteur = Moteur::new(ReglagesMoteur::PHASE_1).expect("le moteur démarre");
    (moteur, octets)
}
