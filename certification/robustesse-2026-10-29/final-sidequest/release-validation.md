# Validation de publication — 27 octobre 2026

Sidequest dépend du commun publié **v0.1.0**, commit `d868b761aeaacad1c523c4ce1d9f0e4ffd396c62`. L’étiquette annotée distante a été vérifiée par `git ls-remote` : objet `095a7044840ada8cbd0fc314c5b41b431a87e6ff`, commit visé `d868b761aeaacad1c523c4ce1d9f0e4ffd396c62`.

## Vérification indépendante du dossier de l’hôte

Les fichiers de travail, y compris les nouveaux manifestes et la documentation, ont été copiés dans `/tmp/soiree-release-sidequest-g4auxqpt`. Les dossiers ignorés data/, target/ et .soiree/ ainsi que le remplacement local `.cargo/config.toml` de l’hôte ne sont pas copiés. Le dossier .soiree/ a été recréé par l’outil publié.

Depuis cette copie, avec `CARGO_TARGET_DIR=/Users/axel2/Projects/build-your-own-game/games/sidequest/target` pour réutiliser les seuls artifacts de compilation :

```sh
cargo metadata --format-version 1
(cd fuzz && cargo metadata --format-version 1)
(cd fuzz && cargo metadata --locked --format-version 1)
soiree check --locked
```

Chaque commande termine avec le **code 0**. Le dernier contrôle indique **0 erreur, 0 avertissement** : formatage, Clippy natif et WASM, tests complets, modules de page générés, catalogue du jeu, configuration du harnais et dépendances Git. Aucun navigateur de certification n’est relancé pour cette migration de métadonnées.

Les anciens lockfiles ont été conservés en entrée de `cargo metadata`, puis les lockfiles résolus dans la copie ont été recopiés dans le dépôt. Aucune version de dépendance tierce déjà présente n’a changé ; le graphe fuzz reçoit les dépendances SHA-256 désormais requises par salon-core. Les trois dépendances salon et les deux dépendances remote sont à la même étiquette publiée.

## Origine effective des crates

La source Cargo de toutes les lignes ci-dessous est :

`git+https://github.com/build-your-own-game/soiree?tag=v0.1.0#d868b761aeaacad1c523c4ce1d9f0e4ffd396c62`

| Graphe | Crate | Version |
|---|---|---|
| racine | remote-protocol | 0.1.0 |
| racine | remote-web | 0.1.0 |
| racine | salon-core | 0.1.0 |
| racine | salon-crypto | 0.1.0 |
| racine | salon-host | 0.1.0 |
| racine | salon-web | 0.1.0 |
| fuzz | salon-core | 0.1.0 |

Les chemins des manifestes correspondants sont sous le cache Git de Cargo, dans le checkout d868b76 ; aucun ne pointe vers le salon du dossier de l’hôte. Le chemin de cache de compilation partagé ne remplace aucune dépendance.

## Périmètre conservé

Les 94 fichiers Rust, deux fichiers HTML, deux feuilles CSS et le shader de la copie sont byte-identiques à ceux du dépôt. La migration porte sur les manifestes, les lockfiles, les règles de version et la documentation autonome. Les anciens rapports de certification et de mutants restent conservés tels quels ; leurs empreintes décrivent leur passage antérieur à cette migration de dépendances.

Empreintes SHA-256 des lockfiles validés :

- Cargo.lock : `8a62a33e066fe28ffad5bf346b91d9161fd921bdab57b6d5a109e3f22f6027aa`
- fuzz/Cargo.lock : `44aeb07a5535e77190d96837061034e8770b9774bdef18e96297905a1814123c`

Le journal de `soiree check --locked` est conservé localement dans `/tmp/soiree-sidequest-release-check.log`. Les métadonnées Cargo de preuve sont dans `/tmp/soiree-sidequest-root-metadata.json` et `/tmp/soiree-sidequest-fuzz-locked-metadata.json`. Cette vérification n’effectue aucun commit ni push.
