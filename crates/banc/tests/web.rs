//! La règle « mêmes lints » de `web/` (PHASE-3, P31, § 8.3), et son témoin.

#![cfg(test)]

use std::path::PathBuf;

fn soiree() -> PathBuf {
    banc::commun()
}

/// `web/` a les règles d'écriture du commun, à l'identique.
#[test]
fn web_a_les_lints_du_commun() {
    let s = soiree();
    let e = banc::web::ecarts(&s, &s.join("web"));
    assert!(e.is_empty(), "web/ s'écarte du commun :\n{}", e.join("\n"));
}

/// Le contrôle négatif de la règle : le témoin a ses propres `[workspace.lints]`
/// (`unwrap_used` permis), un profil `release` sans contrôle de débordement, et un
/// membre qui ne prend pas les lints du workspace. La règle doit nommer les trois.
#[test]
fn le_temoin_aux_lints_differents_fait_echouer_la_regle() {
    let s = soiree();
    let e = banc::web::ecarts(&s, &s.join("web/temoins/lints-differents"));
    let tout = e.join("\n");
    assert!(
        e.iter()
            .any(|l| l.starts_with("workspace.lints.clippy.unwrap_used vaut \"allow\"")),
        "{tout}"
    );
    assert!(
        e.iter()
            .any(|l| l.starts_with("temoin : [lints] doit être `workspace = true`")),
        "{tout}"
    );
    assert!(
        e.iter()
            .any(|l| l.starts_with("profile.release.overflow-checks vaut false")),
        "{tout}"
    );
}

/// La télé dessine ce que l'épreuve mesure, par le même code (PHASE-3, § 2.9, lot 9) :
/// chaque paquet dont dépend `mise-en-page` a la même version dans les deux `Cargo.lock`.
#[test]
fn la_mesure_des_textes_a_les_memes_versions_des_deux_cotes() {
    let s = soiree();
    let lire = |f: &str| std::fs::read_to_string(s.join(f)).unwrap();
    let racines = banc::web::dependances_tierces(&s, "mise-en-page").unwrap();
    for attendue in [
        "cosmic-text",
        "unicode-linebreak",
        "unicode-segmentation",
        "serde",
        "postcard",
    ] {
        assert!(
            racines.iter().any(|r| r == attendue),
            "{attendue} manque : {racines:?}"
        );
    }
    assert!(
        !racines.iter().any(|r| r == "proptest" || r == "sha2"),
        "{racines:?}"
    );
    let e = banc::web::ecarts_de_versions(&lire("Cargo.lock"), &lire("web/Cargo.lock"), &racines);
    assert!(e.is_empty(), "les versions diffèrent :\n{}", e.join("\n"));
}

/// Son contrôle négatif : une version de `cosmic-text` changée dans `web/` se voit.
#[test]
fn une_autre_version_de_cosmic_text_se_voit() {
    let s = soiree();
    let lire = |f: &str| std::fs::read_to_string(s.join(f)).unwrap();
    let web = lire("web/Cargo.lock").replace(
        "name = \"cosmic-text\"\nversion = \"0.19.0\"",
        "name = \"cosmic-text\"\nversion = \"0.19.1\"",
    );
    assert_ne!(web, lire("web/Cargo.lock"), "le témoin n'a rien changé");
    let racines = banc::web::dependances_tierces(&s, "mise-en-page").unwrap();
    let e = banc::web::ecarts_de_versions(&lire("Cargo.lock"), &web, &racines);
    assert_eq!(
        e,
        ["cosmic-text est à 0.19.1 dans web/, à 0.19.0 dans le commun"]
    );
}
