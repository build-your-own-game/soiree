# Pour l'agent qui touche à salon

Lis et applique `../MANIFESTE.md` et `../CERTIFICATION.md`.

- La spécification est la documentation de `crates/core/src/lib.rs`. Une garantie qu'elle annonce a son test dans `crates/core/tests/salon.rs` ou `crates/core/tests/properties.rs` ; une garantie nouvelle aussi. Ces deux fichiers parlent à salon par son API, sur le faux réseau de `crates/core/tests/net/`.
- Les autres fichiers de `crates/core/tests/` tiennent ce que ces scénarios ne voient pas, sans y remplacer une garantie :
  - `client_edges.rs`, `host_edges.rs`, `stream_edges.rs` et `types_edges.rs` fixent les bornes d'un côté seul : le dernier octet accepté, le numéro exact d'un accusé, une échéance. Ils sont nés des mutants survivants, et un nouveau mutant survivant y trouve sa place.
  - `wire/mod.rs` est une seconde lecture du format, champ à champ, pour lire et forger des paquets. Change-le à chaque changement de `PROTOCOL` : les types de salon restent privés, et un changement de forme sans nouveau `PROTOCOL` doit faire rougir ces tests.
  - `found_bugs.rs` reproduit chaque bug trouvé en lisant les mutants, avant son correctif. Corrigé, il reste comme régression. Ouvert, il dit ce que salon promet et porte un `#[ignore]` avec sa raison, que le correctif retire : un test ignoré ne prouve rien.
- `salon-core` reste `#![no_std]`, sans horloge ni hasard : le temps et les octets entrent par les appels.
- Chaque leçon tirée d'une référence va dans `LEARNINGS.md`, avec sa source.
- Les jeux ne contiennent aucune logique réseau : ni jeton, ni reconnexion, ni battement, ni limite de débit. Si un jeu en a besoin, c'est ici qu'elle manque.
- Avant de déclarer fini : `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --target wasm32-unknown-unknown -p salon-core -p salon-web -- -D warnings`, `cargo test`, puis `soiree check .` (l'outil de ce dépôt, `../outil/`).
