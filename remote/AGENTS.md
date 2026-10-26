# Télécommande (app iPhone)

Lis et applique `../MANIFESTE.md`. Ce dossier n'est pas un jeu : c'est l'app iPhone qui sert de télécommande à tous les jeux. Les leçons tirées des manettes existantes sont dans `LEARNINGS.md` : relis-le avant de toucher à la sensation d'une manette.

- `crates/protocol` (`remote-protocol`) : ce qu'une télécommande envoie à un jeu. Partagé avec les jeux et les faux téléphones. Brouillon : seule la manette N64 existe.
- `crates/core` (`remote-core`) : le cerveau. Disposition des presets, gestion des doigts, stick, vibrations à jouer. Pur Rust, sans IO, testé en natif.
- `crates/ffi` (`remote-ffi`) : la face Swift du cœur, générée par UniFFI. Les types y sont recopiés pour que le cœur reste sans code FFI.
- `xtask` : `cargo xtask ios` compile le cœur pour iOS et génère les liaisons Swift dans `ios/Generated` (ignoré par git).
- `ios/` : la coquille Swift 6 (SwiftUI). Elle ne décide rien : elle transmet les doigts à Rust, dessine ce que Rust dit et joue les vibrations.

## Commandes

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo clippy -p remote-protocol -p remote-core --target wasm32-unknown-unknown -- -D warnings
cargo clippy -p remote-ffi --target aarch64-apple-ios -- -D warnings
cargo test
cargo xtask ios          # une fois avant d'ouvrir Xcode, ensuite Xcode le relance à chaque build
open ios/Remote.xcodeproj
```

Le build Xcode lance `cargo xtask ios` pour la cible et le profil en cours : modifier le Rust puis ⌘R suffit.
