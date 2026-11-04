//! Le maximum de mémoire du module d'un fautif (§ 3.1, § 3.6), commun à toutes les
//! crates de `modules/` : sans lui, `wasm-ld` n'en écrit aucun, et la passe `valider`
//! de la réécriture refuse le module. Il vaut `socle::DECLARATION.limites.memoire`,
//! 64 pages de 64 Kio. Le fautif `declaration-incoherente` déclare moins, exprès.

/// `limites.memoire` de la déclaration du socle, en pages de 64 Kio.
const PAGES: u64 = 64;

fn main() {
    println!("cargo::rerun-if-changed=../../build-module.rs");
    // Pour le module seulement : l'éditeur de liens natif ne connaît pas cette option.
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        let octets = PAGES.saturating_mul(65_536);
        println!("cargo::rustc-link-arg-cdylib=--max-memory={octets}");
    }
}
