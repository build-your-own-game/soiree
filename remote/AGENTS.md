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
- `crates/web` (`remote-web`) : pour la page libre d'un jeu en Rust (WASM). Il enveloppe les messages entre la page et l'app (`CONTRAT.md`) ; leur forme est testée en natif dans `message.rs`.
- `remote-protocol::web` est l’unique contrat JSON page/app. `remote-web::message` le réexporte et `remote-ffi::web` le transforme en enums et records Swift générés, avec des conversions exhaustives. Les effets iOS consomment ces types ; toute sérialisation ou validation du contrat reste dans Rust.
- `crates/demo` (`remote-demo`) : un faux jeu pour tout essayer sans vrai jeu, avec une page libre en JavaScript (`/free`).
- `crates/demo-page` (`remote-demo-page`) : la page libre du faux jeu écrite en Rust (`/rust/`), construite par `cargo xtask page`.
- `xtask` : `cargo xtask ios` compile le cœur pour iOS et génère les liaisons Swift dans `ios/Generated` (ignoré par git) ; `cargo xtask page` construit la page Rust du faux jeu.
- `ios/` : la coquille Swift 6 (SwiftUI). Elle ne décide rien : elle tient le WebSocket, transmet les doigts et les capteurs à Rust, dessine ce que Rust dit et joue les vibrations. La page libre tourne dans WebKit : l'app n'y injecte qu'une donnée (`window.telecommande`), reçoit les messages de la page et lui répond par des événements.

## Commandes

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo clippy -p remote-protocol -p remote-core --target wasm32-unknown-unknown -- -D warnings
cargo clippy -p remote-ffi --target aarch64-apple-ios -- -D warnings
cargo test
cargo xtask web-contract # vraies liaisons Swift + scripts exécutés dans JavaScriptCore (macOS)
cargo xtask web-contract-browser # mêmes scripts et vrai SDK WASM dans WebKit et Chromium
soiree check .           # tout ce que le manifeste permet de vérifier par machine (../outil/)
cargo xtask ios          # facultatif : construit séparément les bibliothèques et les liaisons iOS
open ios/Remote.xcodeproj
```

Le build Xcode lance `cargo xtask ios` pour la cible et le profil en cours : ouvrir le projet d'un clone neuf puis ⌘R suffit, sans pré-génération. Les sorties Swift, en-tête, modulemap et bibliothèque sont déclarées dans la phase « Build Rust core » ; conserve ces dépendances pour que le build à froid reste valable. La phase reste exécutée à chaque build : Cargo vérifie les changements Rust transitifs.
