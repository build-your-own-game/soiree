//! Le maximum de mémoire du module de `zones-lourdes` (§ 3.1, § 3.6) : celui des autres
//! fautifs (`../../build-module.rs`) ne tient pas son brouillon de 32 Mio. Il vaut
//! `limites.memoire` de sa déclaration, 640 pages de 64 Kio.

/// `limites.memoire` de la déclaration de `ZonesLourdes`, en pages de 64 Kio.
const PAGES: u64 = 640;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    // Pour le module seulement : l'éditeur de liens natif ne connaît pas cette option.
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        let octets = PAGES.saturating_mul(65_536);
        println!("cargo::rustc-link-arg-cdylib=--max-memory={octets}");
    }
}
