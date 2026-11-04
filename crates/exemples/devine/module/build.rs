//! Le maximum de mémoire du module (§ 3.1, § 3.6) : sans lui, `wasm-ld` n'en écrit
//! aucun, et la passe `valider` de la réécriture refuse le module. Il vaut
//! `Devine::DECLARATION.limites.memoire` pages de 64 Kio ; le test
//! `module_wasm::le_module_n_importe_rien` de `devine` vérifie que les deux s'accordent.

/// `limites.memoire` de la déclaration de `devine`, en pages de 64 Kio.
const PAGES: u64 = 64;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    // Pour le module seulement : l'éditeur de liens natif ne connaît pas cette option.
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        let octets = PAGES.saturating_mul(65_536);
        println!("cargo::rustc-link-arg-cdylib=--max-memory={octets}");
    }
}
