//! Des soirées en mémoire sur le module de `devine`, réécrit et chargé dans wasmtime
//! (PHASE-3, § 8.1, lot 4b) : la vraie autorité sur le vrai moteur, de vrais appareils
//! qui jouent les coups du bot du jeu par le vrai chemin, le relais en mémoire, et les
//! pannes de la graine, dont l'autorité tuée et relancée depuis la garde. Les trois jeux
//! se jouent de même par l'exemple `soiree_en_memoire`.
//!
//! `cfg(test)` marque tout ce binaire comme du code de test : les fonctions d'aide y
//! ont droit à `unwrap`, `expect`, `panic!` et aux index (CONTRAT, « Les règles
//! d'écriture »).

#![cfg(test)]

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use jeu_abi::Oui;
use moteur_wasmtime::{InstantaneWasm, Moteur, ReglagesMoteur};
use salle::simulation::{Pannes, Soiree, verifier_l_historique};

/// Le module de `devine`, compilé une fois par binaire de test, dans un dossier à part
/// (`target/verif-module`) : `cargo test` tient le verrou de `target/`.
fn octets() -> &'static [u8] {
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

/// Une soirée de `devine` : la chaîne entière écrite, chaque geste au plus une fois, tout
/// geste accusé écrit, tout ce qui attend accusé à la fin, et la garde finale qui
/// reprend la même partie. Rend l'empreinte de la chaîne.
fn soiree_de_devine(graine: u64) -> [u8; 32] {
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).expect("le moteur démarre");
    let charge = moteur.charger(octets()).expect("le module de devine");
    let mut s = Soiree::new(|| charge.fabrique(), graine, Oui::Oui, Pannes::SOIREE);
    let b = s.jouer(4, 60_000_000);
    eprintln!("devine, graine {graine} : {b:?}");
    assert!(b.arret.is_none(), "{b:?}");
    assert!(b.redemarrages > 0, "{b:?}");
    // Devine va lentement : une manche toutes les quinze secondes environ, un essai par
    // joueur et par manche.
    assert!(b.autorite.gestes_places >= 10, "{b:?}");
    let historique = s.relais.historique();
    let h = verifier_l_historique::<InstantaneWasm>(&s.cle_de_garde(), s.salle(), &historique)
        .expect("la chaîne de la soirée");
    assert_eq!(h.en_double, Vec::<u64>::new());
    // SAL-56 : tout geste accusé est dans la chaîne écrite.
    assert_eq!(s.accuses_non_ecrits(&h), Vec::<String>::new());
    for (k, t) in s.telephones.iter().enumerate() {
        let jumeau = s.telephones[..k].iter().any(|u| u.jeton == t.jeton);
        if !jumeau {
            assert_eq!(t.appareil.en_file(), 0, "téléphone {k}");
        }
    }
    s.comparer_a_la_garde()
        .expect("la garde reprend la même partie");
    let mut e = blake3::Hasher::new();
    for (c, o) in &historique {
        e.update(format!("{c:?}").as_bytes());
        e.update(o);
    }
    *e.finalize().as_bytes()
}

#[test]
fn une_soiree_de_devine_en_memoire_survit_a_ses_pannes() {
    let _: [u8; 32] = soiree_de_devine(11);
}

/// SAL-09 : la même graine refait la même soirée, case pour case.
#[test]
fn une_soiree_en_memoire_se_refait_a_l_identique_sal09() {
    assert_eq!(soiree_de_devine(12), soiree_de_devine(12));
}
