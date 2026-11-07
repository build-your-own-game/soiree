//! Le maximum de mémoire du module de `module-trop-gros` (§ 3.1, § 3.6) : celui des
//! autres fautifs (`../../build-module.rs`, 4 Mio) ne tient pas sa table de 4,15 Mio.
//! Il vaut `limites.memoire` de sa déclaration, 128 pages de 64 Kio.

/// `limites.memoire` de la déclaration de `ModuleTropGros`, en pages de 64 Kio.
const PAGES: u64 = 128;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    // Pour le module seulement : l'éditeur de liens natif ne connaît pas cette option.
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        let octets = PAGES.saturating_mul(65_536);
        println!("cargo::rustc-link-arg-cdylib=--max-memory={octets}");
    }
}
