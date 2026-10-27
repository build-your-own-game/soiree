# Pour l'agent qui touche à salon

Lis et applique `../MANIFESTE.md` et `../CERTIFICATION.md`.

- La spécification est la documentation de `crates/core/src/lib.rs`. Une garantie qu'elle annonce a son test dans `crates/core/tests/salon.rs` ou `crates/core/tests/properties.rs` ; une garantie nouvelle aussi.
- `salon-core` reste `#![no_std]`, sans horloge ni hasard : le temps et les octets entrent par les appels.
- Chaque leçon tirée d'une référence va dans `LEARNINGS.md`, avec sa source.
- Les jeux ne contiennent aucune logique réseau : ni jeton, ni reconnexion, ni battement, ni limite de débit. Si un jeu en a besoin, c'est ici qu'elle manque.
- Avant de déclarer fini : `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --target wasm32-unknown-unknown -p salon-core -p salon-web -- -D warnings`, `cargo test`, puis `soiree check .` (l'outil de ce dépôt, `../outil/`).
