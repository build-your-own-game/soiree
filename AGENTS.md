# Pour l'agent qui modifie le commun

> Ce fichier s'adresse à l'agent de code qui travaille dans ce dépôt, quel qu'il soit. Ce dépôt est public, et tous les jeux de la soirée en dépendent.

Le commun est en reconstruction sur la branche `refonte` : les phases 1 et 2 sont écrites (voir `README.md`, « Où en est le commun »), et ce qui reste, avec ce que la phase 3 devra mesurer ou trancher, est dans `conception/RESTE-PHASE-2.md`. Ce fichier décrit comment travailler sur ce qui existe maintenant. L'ancien commun reste lisible à l'étiquette `avant-refonte`.

## Ce qui fait foi

Dans l'ordre, le premier l'emporte :

1. `MANIFESTE.md` et `CONTRAT.md`. **Ne les modifie pas sans demande explicite du mainteneur.** Si l'un d'eux te paraît faux ou bloquant, dis-le, sans le corriger.
2. `conception/INTERFACE.md` : l'API d'un jeu, le contrat binaire, le découpage en crates, les décisions D1 à D38. S'il contredit le contrat, le contrat gagne : note l'écart.
3. `conception/REPONSES-PROVISOIRES.md` : les réponses provisoires aux questions du § 13, chacune avec ce qui changerait si le mainteneur répondait autrement.
4. Les leçons de l'ancien code, `lecons/sources/*.md`, et le `CAHIER.md` de chaque jeu.

`CERTIFICATION.md`, `VERIFICATION.md`, `REPETITION.md`, `REJOINDRE.md`, `certifications/`, `recherche/`, `archive/` et `outil/` décrivent l'ancien système (salon, la télécommande, `soiree check`, `soiree certify`, `soiree new`). **N'en suis pas le processus, et ne lance pas la commande `soiree`** : l'outil n'est pas réécrit et ne connaît pas les nouvelles crates. Ces documents attendent la décision du mainteneur ; ne les supprime pas non plus de ta propre initiative.

## Committer et pousser

- Un commit appartient à un seul dépôt. Un changement qui touche `soiree/` et un jeu fait au moins deux commits, un dans chaque dépôt.
- Lance `git` depuis le dossier du dépôt, ou avec `git -C <dépôt> …`. Le dossier de l'hôte, au-dessus, n'est pas un dépôt.
- Ne pousse rien sans l'accord du mainteneur s'il ne te l'a pas déjà donné. Ne change pas de branche sans qu'il le demande.

## Chaque changement touche tous les jeux

Pendant la refonte, les trois jeux dépendent du commun **par chemin** (`../../soiree/crates/…`) : un changement ici se voit tout de suite dans chacun d'eux, sans étiquette.

- Avant d'ajouter quelque chose, demande-toi si un seul jeu en a besoin. Si oui, sa place est peut-être dans ce jeu. Un besoin qui revient dans deux jeux pèse plus lourd (MANIFESTE, « La méthode »).
- Un besoin d'un jeu se répond dans `conception/BESOINS-DES-JEUX.md` : accordé (avec ses tests), refusé (avec sa raison tirée du contrat) ou différé (avec ce qui le rouvrira).
- Un changement qui casse un jeu dit comment s'adapter, dans `CHANGELOG.md` et, pendant la refonte, dans `conception/BESOINS-DES-JEUX.md`.
- **Les octets échangés ont une seule définition, dans `jeu-abi`.** Changer la forme d'un `Appel`, d'un `Retour` ou d'une `EntreeFil` fait rougir `forme_figee` : un tel changement monte `jeu_abi::ABI` et réécrit à la main les octets attendus de `crates/jeu-abi/tests/abi/commun.rs`. Un variant s'ajoute à la fin de son enum.
- Changer ce que fait la réécriture monte `reecriture::VERSION` : l'empreinte d'un module réécrit en dépend.
- Changer la forme de ce que la garde tient (un type de `partie` gardé, l'instantané d'une instance) monte `partie::persistance::VERSION_GARDE` et réécrit à la main les octets figés de `crates/partie/tests/coeur/garde.rs` et `forme_garde.rs` (et, pour l'instantané d'un module, ceux des tests de `crates/moteur-commun/src/instantane.rs`). Un variant ajouté à une enum gardée ne compile plus dans `forme_garde.rs` tant qu'il n'a pas ses octets figés.
- Le moteur et le cœur (`moteur-commun`, `moteur-wasmtime`, `partie`) ne dépendent jamais de `jeu` : ils ne connaissent aucun type de jeu (D6).
- **Le protocole d'un appel a une seule définition, dans `moteur-commun`** (PHASE-3, P2) : le déroulé d'un appel, le classement d'un piège en faute ou en panne, l'instantané hors des quatre zones, l'empreinte et ce que le chargement vérifie. Un moteur (`moteur-wasmtime`, et `web/moteur-navigateur` depuis le lot 3a de la phase 3) n'écrit que l'`impl Machine` de ses primitives ; n'y recopie rien de ce qui se décide.

Ce qui est figé pour les autres crates, et le graphe des dépendances, sont dans `crates/README.md`.

## Le workspace

- Le workspace du commun, `Cargo.toml` à la racine ; ses membres sont nommés un à un. Une crate de plus se décide là, avec sa raison : les crates du § 8 d'INTERFACE.md sont un découpage voulu.
- **Un second workspace, `web/`** (PHASE-3, P31) : ce qui ne sert que dans le navigateur (`moteur-navigateur` aujourd'hui ; demain `autorite`, `ecran`, `telephone`, `rendu`), pour que `cargo test --workspace` du commun ne compile ni `wasm-bindgen` ni `wgpu`. Ses `[workspace.lints]`, son `clippy.toml`, son `rust-toolchain.toml` et ses profils (sauf `module`, qui ne sert qu'aux jeux) sont **égaux** à ceux de la racine, et chacun de ses membres écrit `[lints] workspace = true` : la règle « mêmes lints » le vérifie (`crates/banc/src/web.rs`, tenue par `crates/banc/tests/web.rs` avec son témoin `web/temoins/lints-differents/`, et relancée par `soiree-banc machine`). Changer une règle d'écriture se fait ici, puis à l'identique dans `web/`. Ses dépendances tierces se déclarent dans `web/Cargo.toml`, avec leur raison ; les crates du commun y sont prises par chemin.
- `outil/` est un workspace à part, exclu ; l'ancien code n'entre pas dans celui-ci.
- Les règles d'écriture du contrat sont dans `[workspace.lints]` et `clippy.toml`. Chaque membre écrit `[lints] workspace = true`. Une exception s'écrit `#[expect(lint, reason = "…")]` à l'endroit précis, jamais `#[allow]`.
- Pas de code `unsafe` : chaque crate qui n'en a pas besoin pose `#![forbid(unsafe_code)]`. Les deux seuls endroits où INTERFACE.md (§ 2.14) en exige sont les exports et l'allocateur que `jeu::module!` écrit dans une crate de module, et l'allocateur de `jeu::colle`, chacun sous `#[expect(unsafe_code, reason = "…")]`. Les exports de `#[wasm_bindgen]`, dans `web/`, n'en demandent pas (constaté au lot 3a, wasm-bindgen 0.2.129) : leurs crates posent aussi `#![forbid(unsafe_code)]`.
- Une dépendance tierce se déclare une fois, dans `[workspace.dependencies]`, avec sa raison en commentaire à côté d'elle. Un membre la prend par `x.workspace = true`. Le code tiers se choisit consciemment et se réduit (CONTRAT, « Le code tiers »).
- Une crate de module se compile pour WebAssembly avec le profil `module` ; en natif, `jeu::module!` n'y écrit rien.
- Identifiants, commentaires et documents en français.

## Construire, tester, éprouver

Depuis ce dossier :

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run --release -p epreuve --bin soiree-epreuve -- crates/exemples/devine
```

- **L'épreuve part des sources** : `soiree-epreuve <dossier du jeu>` (CONTRAT, « L'épreuve », étape 1). Le dossier est la racine du workspace du jeu, sa crate de règles ou sa crate de module ; `--module <crate>` choisit dans un workspace qui en a plusieurs, `--cible <dossier>` le dossier de compilation. L'épreuve lit le workspace (`cargo metadata`), vérifie les règles d'écriture (forme de la crate de module, `#![forbid(unsafe_code)]` et `#![no_std]`, lints, profils, `clippy.toml`, aucun `allow`, aucun `static` ni `thread_local!`, aucun `impl Forme`/`Etat`/`Message` écrit à la main, aucune crate par chemin hors du workspace du jeu, le même commun, et ce que les `build.rs` disent à Cargo), lance clippy en natif et pour `wasm32`, compile le module (profil `module`) et la copie native, puis joue. Le rapport nomme les sept étapes du CONTRAT, faites ou non, exigées ou non avec leur raison (`crates/epreuve/src/etapes.rs`) ; le code de sortie n'est 0 que si chaque étape exigée est faite.
- **La copie native, sans exemple écrit à la main.** La convention : la crate de module d'un jeu se réduit à `jeu::module!(<chemin du type du jeu>);` (plus `#![no_std]` ou `#![cfg_attr(target_arch = "wasm32", no_std)]`), et ne dépend que de `jeu` et de la crate de règles. L'épreuve y lit le type du jeu, et écrit hors du dépôt du jeu, dans `<cible>/soiree-epreuve/<crate de module>/`, un binaire de trois lignes qui donne `jeu_natif::FabriqueNative::<chemin>` à `epreuve::ligne_de_commande`. Ce binaire dépend de la crate de règles par chemin, de `jeu-natif` et `epreuve` de ce commun-ci, part du `Cargo.lock` du jeu et se compile sans réseau dans `<cible>/soiree-epreuve/cible` (partagé entre les jeux d'un même dossier de compilation). Le jeu doit donc dépendre de ce même commun (règle « même commun »). Une crate `<jeu>-epreuve` écrite à la main n'est plus nécessaire.
- **Un module seul** : `soiree-epreuve --wasm <module.wasm>` (ou le chemin du fichier). Le rapport le dit, l'étape 1 n'est pas faite, et le code de sortie n'est jamais 0. `cargo run --release -p epreuve --example natif -- <nom> <module>` éprouve ainsi un fautif ou `devine` avec sa copie native, sans recompiler.
- Les modules des fautifs se compilent avec `-p fautif-<nom>-module`. `crates/epreuve/tests/epreuve` vérifie, sur le plan court, que chacun fait échouer exactement son contrôle ; sur le plan standard, qui fait foi, `cargo test -p epreuve --test epreuve -- --ignored plan_standard` (3 125 s pour 35 jeux le 2026-11-05 sur la machine de l'hôte, quatre fils). Ces tests jouent le module et la copie native, sans l'étape 1. `crates/epreuve/tests/sources.rs` éprouve depuis leurs sources `devine`, `sain`, un fautif sans `static` et un fautif à `static`, et une crate de module qui fait plus que la macro.
- **Les fautifs depuis leurs sources.** `soiree-epreuve crates/exemples/fautifs/modules/<nom>` rend 0 pour `sain` et `avec-fenetre`, et 1 pour un fautif sans `static`, avec son seul contrôle en échec. Les cinq fautifs dont la faute est un `static` (`refuse-apres-avoir-ecrit`, `retient-un-refus`, `vue-qui-ecrit`, `secret-dans-un-static`, `etat-hors-enveloppe`) ont leur défaut dans la crate `fautifs-statiques` : depuis leurs sources, ils échouent aussi l'étape 1, dont la règle « rien hors de l'état » nomme le `static` (la forme les attrape avant la pression), et `refuse-apres-avoir-ecrit` fait échouer E8 avec E6 ; leur contrôle se lit dans le rapport, ou seul par le module et la copie native (exemple `natif`). Un défaut qui tient un `static` va dans `fautifs-statiques`, jamais dans `fautifs` : la règle lit `fautifs` pour chaque fautif, et un seul `static` y ferait échouer l'étape 1 à tous (RESTE-PHASE-2, point 1).
- Les tests de `epreuve` prennent l'essentiel de la durée de `cargo test` : le 2026-11-05, après `cargo clean`, `cargo test --workspace` a pris 575 s compilation comprise, dont 289 s pour `tests/epreuve` et 134 s pour `tests/sources.rs` (voir `conception/RESTE-PHASE-1.md`, « Contrôle final de la phase 2 »).
- **`web/`** se construit et se vérifie depuis son dossier, pour `wasm32-unknown-unknown` comme en natif :
  ```sh
  cd web
  cargo clippy --all-targets --target wasm32-unknown-unknown -- -D warnings
  cargo clippy --all-targets -- -D warnings
  cargo test            # ce qui se teste sans navigateur
  ```
  Ce qui demande un navigateur se joue par `soiree-banc`, depuis ce dossier-ci : `cargo run -p banc --bin soiree-banc -- machine` construit l'exemple `essais-navigateur` de `web/moteur-navigateur` et sa colle (`wasm-bindgen`, à la version épinglée), sert sa page, lance Chrome sans interface sur un profil neuf, le pilote par CDP et juge le rapport de la page (code 0 réussi ; 1 échec, dont la règle « mêmes lints » qui refuse `web/` et une page qui ne rapporte rien ou rapporte une erreur ; 2 pas exécuté : Chrome absent, construction impossible). Pas de `wasm-bindgen-test` : il demande `chromedriver` (PHASE-3, annexe).
- Sur une machine de 16 Go, une seule commande lourde à la fois : deux compilations de wasmtime en parallèle saturent la mémoire. `CARGO_BUILD_JOBS=4` la borne encore. Un navigateur ne se lance jamais pendant une compilation : `soiree-banc machine` construit d'abord, puis lance Chrome.
- Quand un autre agent travaille dans le même dossier, préfère `cargo … -p <crate>` au workspace entier, et ne touche qu'aux fichiers de ta tâche.

## Les contrôles et leurs négatifs

- **Chaque contrôle de l'épreuve a un contrôle négatif** : un jeu de `crates/exemples/fautifs` qui doit le faire échouer, et lui seul (§ 7.2 ; OUT-27, HIS-03, SAL-168). Un contrôle de plus vient avec son fautif, sa crate de module dans `exemples/fautifs/modules/<nom>/`, son entrée dans l'exemple `natif` et son test dans `epreuve/tests/epreuve`.
- Un contrôle qui n'a pas tourné n'a pas réussi ; une panne de l'outil n'est pas un échec du jeu ; une exigence sans objet dit sa raison (OUT-01, OUT-02, OUT-07, HIS-05, HIS-06). Ne fais pas passer l'un pour l'autre.
- Un plan réduit (`--court`, `--controles`) reste visible dans le rapport. Ne présente pas son succès comme celui du plan standard.
- Un test qui existe à cause d'une leçon cite son identifiant (`SAL-`, `OUT-`, `PUN-`…) dans son nom ou son commentaire.

## Les mesures

- Aucun chiffre n'est présenté comme mesuré s'il ne l'est pas. Une mesure porte sa date, sa machine et la commande qui la refait.
- Les budgets que juge E19 sont provisoires et s'écrivent à un seul endroit, `epreuve::BUDGETS_E19` (`crates/epreuve/src/budgets.rs`), avec leur dérivation : le temps d'une image du CONTRAT, la durée d'une unité de carburant et le coût fixe d'un appel mesurés dans wasmtime (`examples/etalonner.rs`, daté, avec sa machine), une marge choisie pour le navigateur et l'ordinateur de la soirée, et la part d'une image laissée au jeu. Change une mesure en la refaisant, jamais à la main ; ne présente pas la marge comme mesurée. Le rapport dit ces budgets provisoires ; **c'est le mainteneur qui fixe les budgets, dans `CONTRAT.md`**.
- Le carburant est compté par la réécriture : il est le même sur toute machine. Une durée, non : elle dépend de la machine et de sa charge.

## La documentation

Une phrase fausse produit du mauvais code chez tous les agents. Chaque fait a une seule source ; les autres documents y renvoient.

| Fichier | Son rôle, et lui seul |
|---|---|
| `MANIFESTE.md` | le pari et les principes |
| `CONTRAT.md` | les règles du commun et des jeux |
| `conception/INTERFACE.md` | la conception retenue : API, contrat binaire, crates, décisions |
| `conception/REPONSES-PROVISOIRES.md` | les réponses provisoires aux questions du § 13 |
| `conception/BESOINS-DES-JEUX.md` | la réponse du commun à chaque besoin des jeux |
| `conception/MESURES-PHASE-1.md` | ce qui a été mesuré, daté, avec sa machine |
| `conception/RESTE-PHASE-1.md` | ce que la phase 1 a laissé, point par point, coché avec le test qui le tient ; les vérifications de chaque chantier de la phase 2 |
| `conception/RESTE-PHASE-2.md` | ce qui reste après la phase 2, et ce que la phase 3 devra mesurer ou trancher |
| `conception/PHASE-3.md` | la conception retenue de la phase 3 : l'autorité dans le navigateur, le relais, le chiffrement, la télé, les pages téléphone, les décisions P1 à P35, les lots de construction et les questions au mainteneur |
| `conception/proposition-*.md` | les trois propositions d'où vient INTERFACE.md ; elles ne font plus foi |
| `crates/README.md` | la carte des crates, leur graphe, ce qui est figé ; celles de `web/` aussi |
| `lecons/sources/` | les leçons de l'ancien code, par identifiant |
| `CHANGELOG.md` | ce qui a changé, et comment s'adapter |
| `README.md` | par où commencer |
| `AGENTS.md` | comment travailler ici |

Les anciens documents (voir « Ce qui fait foi ») ne sont la source d'aucun fait sur le nouveau commun.

## Versions et `CHANGELOG.md`

- Une version est une étiquette annotée sur ce dépôt. Les étiquettes `v0.x` existantes sont celles de l'ancien commun ; le nouveau n'en a encore aucune. Une étiquette publiée ne bouge plus : une erreur se corrige par une nouvelle version.
- On suit semver. Tant qu'on est en `0.x`, un changement qui casse un jeu monte le chiffre du milieu ; une correction qui ne casse rien monte le dernier.
- Ce qui n'est pas encore étiqueté va dans la section « Non publié », en tête de `CHANGELOG.md`. À l'étiquette, elle prend le nom de la version et sa date.
- Chaque section dit ce qui change pour quelqu'un qui fait un jeu, puis « Passer à cette version » : des étapes qu'un agent qui ne connaît pas ce dépôt peut suivre sans rien deviner, avec le nom exact de ce qui change, et le code avant et après s'il le faut.
- Un changement de sens sans rupture de type est plus dangereux qu'une rupture : il s'y documente aussi, et possède un test (CONTRAT, « Les versions »).
