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
