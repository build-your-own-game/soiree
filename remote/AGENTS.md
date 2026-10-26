# Télécommande (app iPhone)

Lis et applique `../MANIFESTE.md`. Ce dossier n'est pas un jeu : c'est l'app iPhone qui sert de télécommande à tous les jeux. Les leçons tirées des manettes existantes sont dans `LEARNINGS.md` : relis-le avant de toucher à la sensation d'une manette.

- `CONTRAT.md` : comment un jeu se branche sur la télécommande. À tenir à jour avec `crates/protocol`.
- `crates/protocol` (`remote-protocol`) : le contrat en code, utilisé aussi par les jeux. Messages du WebSocket `/remote`, `InputState`, `DrawEvent`, la description d'une télécommande (`Layout`, `Remote::Web`) et les sept presets, écrits comme des compositions ordinaires.
- `crates/core` (`remote-core`) : le cerveau, pur Rust, sans IO, testé en natif.
  - `controls` : le moteur de toucher, qui place une `Layout` sur un écran et applique les règles de sensation.
  - `remote` : une télécommande pad (contrôles, plus volant, pointeur ou toile si la `Layout` les demande).
  - `wheel`, `pointer`, `draw` : l'inclinaison, le curseur, la toile.
  - `session` : les messages échangés avec le jeu et leur rythme.
  - `join` : du QR scanné à l'adresse du WebSocket.
  - `feel` (tests) : la sensation, vérifiée sur tous les presets et plusieurs iPhone.
- `crates/ffi` (`remote-ffi`) : la face Swift du cœur, générée par UniFFI. Les types y sont recopiés pour que le cœur reste sans code FFI.
- `crates/demo` (`remote-demo`) : un faux jeu pour tout essayer sans vrai jeu.
- `xtask` : `cargo xtask ios` compile le cœur pour iOS et génère les liaisons Swift dans `ios/Generated` (ignoré par git).
- `ios/` : la coquille Swift 6 (SwiftUI). Elle ne décide rien : elle tient le WebSocket, transmet les doigts et les capteurs à Rust, dessine ce que Rust dit et joue les vibrations. La page libre tourne dans WebKit, avec `window.telecommande`.

## Commandes

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo clippy -p remote-protocol -p remote-core --target wasm32-unknown-unknown -- -D warnings
cargo clippy -p remote-ffi --target aarch64-apple-ios -- -D warnings
cargo test
../tools/check.sh .      # tout ce que le manifeste permet de vérifier par machine
cargo xtask ios          # une fois avant d'ouvrir Xcode, ensuite Xcode le relance à chaque build
open ios/Remote.xcodeproj
```

Le build Xcode lance `cargo xtask ios` pour la cible et le profil en cours : modifier le Rust puis ⌘R suffit.
