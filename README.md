# La soirée

Des amis font chacun écrire un jeu par leur agent de code, puis on y joue tous ensemble un soir, dans la même pièce : un ordinateur branché sur la télé, et, selon le jeu, le clavier, des manettes ou des téléphones. L'idée, le pari et les principes sont dans [`MANIFESTE.md`](MANIFESTE.md) ; les règles qui en découlent, dans [`CONTRAT.md`](CONTRAT.md).

Ce dépôt est le commun : ce qui est écrit une fois et partagé par tous les jeux. Chaque jeu vit dans son propre dépôt, dans l'organisation `build-your-own-game` ; la liste est dans [`catalogue.toml`](catalogue.toml).

## Où en est le commun

**Le commun est en cours de reconstruction, sur la branche `refonte`.** L'ancien commun (salon, la télécommande iPhone, le modèle de jeu, le harnais de certification) a été retiré ; il reste lisible à l'étiquette `avant-refonte` (`git show avant-refonte:<chemin>`). Aucune version du nouveau commun n'est publiée.

La **phase 1** est écrite : un jeu écrit avec la crate `jeu`, compilé en module WebAssembly, réécrit, chargé dans wasmtime, joué par le cœur de la partie sur une horloge simulée, et éprouvé, sans navigateur ni réseau. Les trois jeux de la soirée (Punchline, Sidequest, Tier list) sont reconstruits sur elle. Le 2026-11-04, sur le plan standard, chacun échoue E19, le contrôle des budgets, et lui seul (les autres sont réussis, ou sans objet avec leur raison) ; ses seuils restent des points de départ non mesurés (voir `conception/MESURES-PHASE-1.md`).

On ne peut donc pas encore **jouer** : il n'y a ni page, ni relais, ni télé. Voir « Ce qui n'existe pas encore ».

## Ce qui fait foi

Dans l'ordre, le premier l'emporte :

1. [`MANIFESTE.md`](MANIFESTE.md) et [`CONTRAT.md`](CONTRAT.md). Ils ne changent qu'à la demande du mainteneur.
2. [`conception/INTERFACE.md`](conception/INTERFACE.md) : l'API Rust d'un jeu, le contrat binaire entre le module et le commun, le découpage en crates, l'ordre de construction, les décisions D1 à D38. S'il contredit le contrat, le contrat gagne.
3. [`conception/REPONSES-PROVISOIRES.md`](conception/REPONSES-PROVISOIRES.md) : les réponses provisoires aux questions du § 13 d'INTERFACE.md, en attendant celles du mainteneur.
4. Les leçons de l'ancien code, dans [`lecons/sources/`](lecons/sources/) (identifiants `SAL-`, `REM-`, `OUT-`, `HIS-`, `CAT-`, `PUN-`, `SQ-`, `TL-`), et le cahier de chaque jeu (`CAHIER.md`, dans son dépôt).

[`conception/BESOINS-DES-JEUX.md`](conception/BESOINS-DES-JEUX.md) répond, besoin par besoin, à ce que les trois jeux ont demandé au commun pendant la phase 1. [`conception/MESURES-PHASE-1.md`](conception/MESURES-PHASE-1.md) donne ce qui a été mesuré sur eux : des propositions de budgets, pas des budgets.

## Les crates

Toutes dans [`crates/`](crates/), un seul workspace. La carte complète, le graphe des dépendances et ce qui est figé pour les autres crates sont dans [`crates/README.md`](crates/README.md).

| Crate | Rôle |
|---|---|
| `jeu-abi` | l'enveloppe `Appel` / `Retour` / `EntreeFil`, une seule définition des octets échangés entre le module et le commun ; `no_std` |
| `jeu-derive` | les macros `#[jeu::message]`, `#[jeu::etat]`, `#[derive(Variantes)]` |
| `jeu` | ce qu'écrit l'auteur d'un jeu, la colle du module et `jeu::module!` ; `no_std` |
| `jeu-natif` | la copie native d'un jeu, pour aller vite pendant le développement ; l'épreuve la compare au module |
| `reecriture` | valider et réécrire un module : carburant, profondeur, NaN, croissance de la mémoire, globales, pile |
| `moteur-wasmtime` | le module réécrit, chargé dans wasmtime |
| `partie` | le cœur de la partie qui fait foi : une fonction de ses entrées, sans horloge, réseau ni WebAssembly |
| `epreuve` | la soirée simulée, les contrôles E1 à E19, et le binaire `soiree-epreuve` |
| `exemples/devine` | le jeu d'exemple complet (INTERFACE.md, § 9) |
| `exemples/ardoise` | un jeu sans téléphone, écran seul |
| `exemples/fautifs` | un jeu sain, et un jeu fautif par contrôle de l'épreuve : chacun doit faire échouer exactement le sien |

`outil/` est l'ancien outil `soiree`, hors du workspace ; il décrit l'ancien système et n'est pas encore réécrit.

## Construire et éprouver un jeu

Il faut Rust, à la version de [`rust-toolchain.toml`](rust-toolchain.toml) (1.99), avec la cible `wasm32-unknown-unknown` ; `rustup` les installe d'après ce fichier. Les commandes ci-dessous ont été lancées le 2026-11-04 (les épreuves par leur binaire compilé en `--release`) ; leurs durées sont dans `conception/MESURES-PHASE-1.md`. Au 2026-11-04, `cargo test --workspace` a un échec : `refus_ne_change_rien`, dans `exemples/devine/tests/regles.rs`.

**Le commun**, depuis ce dossier :

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Un module**, avec le profil `module` (`panic = "abort"`, LTO, débordements vérifiés), par exemple celui de `devine` :

```sh
cargo build -p devine-module --target wasm32-unknown-unknown --profile module
```

Le module est dans `target/wasm32-unknown-unknown/module/devine_module.wasm`. Il ne doit rien importer : le contrôle E1 de l'épreuve le constate et refuse un module qui importe.

**L'épreuve d'un module** :

```sh
cargo run --release -p epreuve --bin soiree-epreuve -- target/wasm32-unknown-unknown/module/devine_module.wasm
```

Le binaire ne connaît aucun jeu : sans copie native, E9 et E14 ne comparent le module qu'à lui-même, et le rapport le dit. Avec la copie native de `devine` ou d'un fautif :

```sh
cargo run --release -p epreuve --example natif -- devine target/wasm32-unknown-unknown/module/devine_module.wasm
```

Un jeu fait de même avec sa propre copie native : Punchline et Sidequest ont une crate `<jeu>-epreuve`, Tier list un exemple `epreuve` (voir le README de chaque jeu).

Options : `--court` (une soirée de deux minutes et une soirée longue d'une heure, au lieu de deux soirées de trois minutes et six heures), `--graine HEX`, `--soirees N`, `--duree S`, `--memoire S`, `--controles E1,E4,…`, `--json CHEMIN`. Le rapport lisible va sur la sortie standard, le rapport JSON à côté du module (`<module>.epreuve.json`). Chaque contrôle y est réussi, échec du jeu (avec de quoi le rejouer), panne de l'outil, sans objet (avec sa raison) ou pas exécuté. Le code de sortie vaut 0 quand le jeu passe les contrôles demandés, 1 sinon ; 64 pour une ligne de commande mal formée, 66 pour un module illisible, 70 pour un moteur qui ne démarre pas. Un plan court ou réduit n'est pas le plan standard : son succès ne vaut que pour lui.

**Un jeu de la soirée** dépend du commun par chemin (`../../soiree/crates/…`) : aucune étiquette du nouveau commun n'existe. Il ne se compile donc que dans le dossier de l'hôte, à côté de `soiree/`.

## Ce qui n'existe pas encore

- **Le navigateur.** Ni la page de la télé, ni la page téléphone, ni la partie qui fait foi dans un onglet. Le module ne tourne aujourd'hui que dans wasmtime, en natif.
- **Le relais.** Rien ne relie encore de vrais appareils ; l'épreuve simule les téléphones, leurs coupures et leurs retards.
- **Le rendu.** La scène d'un jeu est validée (E16), pas dessinée ; ni son, ni mesure du texte, ni composants.
- **La prédiction sur appareil.** La fenêtre et la tête existent dans le cœur ; exécuter l'étape sur un téléphone pour prédire viendra en phase 2.
- **L'outil `soiree` réécrit.** `outil/` est l'ancien outil : `soiree check`, `soiree certify` et `soiree new` décrivent l'ancien système et ne connaissent pas les nouvelles crates. Ne les lance pas sur un jeu reconstruit.
- **Une version publiée** du nouveau commun, et donc les dépendances git des jeux, `soiree upgrade` et le `[patch]` local du dossier de l'hôte.
- **L'app iPhone.** L'ancienne télécommande (`remote/`) est retirée avec l'ancien commun.
- **Les budgets.** Les seuils que juge E19 sont des points de départ choisis, non mesurés ; le mainteneur les fixe dans `CONTRAT.md`.

## Les anciens documents

`CERTIFICATION.md`, `VERIFICATION.md`, `REPETITION.md`, `REJOINDRE.md`, `certifications/`, `recherche/`, `archive/` et `outil/` décrivent encore l'ancien système (salon, la télécommande, `soiree check`, `soiree certify`). Ils restent là en attendant la décision du mainteneur ; ne suis pas leur processus. `CONTRAT.md` renvoie encore à certains d'entre eux.

## Où trouver le reste

| Pour | Lis |
|---|---|
| le pari et les principes | [`MANIFESTE.md`](MANIFESTE.md) |
| les règles du commun et des jeux | [`CONTRAT.md`](CONTRAT.md) |
| l'interface d'un jeu et le découpage du commun | [`conception/INTERFACE.md`](conception/INTERFACE.md) |
| les crates, une par une | [`crates/README.md`](crates/README.md) |
| les leçons de l'ancien code | [`lecons/sources/`](lecons/sources/) |
| ce qui a été mesuré | [`conception/MESURES-PHASE-1.md`](conception/MESURES-PHASE-1.md) |
| ce qui change d'une version à l'autre | [`CHANGELOG.md`](CHANGELOG.md) |
| modifier le commun | [`AGENTS.md`](AGENTS.md) |

Une question, un besoin du commun, une règle qui gêne : ouvre une issue sur ce dépôt.
