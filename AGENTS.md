# Pour l'agent qui modifie le commun

> Ce fichier s'adresse à l'agent de code qui travaille dans ce dépôt, quel qu'il soit. Ce dépôt est public, et tous les jeux de la soirée en dépendent.

Le commun est en reconstruction sur la branche `refonte` : la phase 1 est écrite (voir `README.md`, « Où en est le commun »). Ce fichier décrit comment travailler sur ce qui existe maintenant. L'ancien commun reste lisible à l'étiquette `avant-refonte`.

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
- Le moteur et le cœur (`moteur-wasmtime`, `partie`) ne dépendent jamais de `jeu` : ils ne connaissent aucun type de jeu (D6).

Ce qui est figé pour les autres crates, et le graphe des dépendances, sont dans `crates/README.md`.

## Le workspace

- Un seul workspace, `Cargo.toml` à la racine ; ses membres sont nommés un à un. Une crate de plus se décide là, avec sa raison : les crates du § 8 d'INTERFACE.md sont un découpage voulu.
- `outil/` est un workspace à part, exclu ; l'ancien code n'entre pas dans celui-ci.
- Les règles d'écriture du contrat sont dans `[workspace.lints]` et `clippy.toml`. Chaque membre écrit `[lints] workspace = true`. Une exception s'écrit `#[expect(lint, reason = "…")]` à l'endroit précis, jamais `#[allow]`.
- Pas de code `unsafe` : chaque crate qui n'en a pas besoin pose `#![forbid(unsafe_code)]`. Les deux seuls endroits où INTERFACE.md (§ 2.14) en exige sont les exports et l'allocateur que `jeu::module!` écrit dans une crate de module, et l'allocateur de `jeu::colle`, chacun sous `#[expect(unsafe_code, reason = "…")]`.
- Une dépendance tierce se déclare une fois, dans `[workspace.dependencies]`, avec sa raison en commentaire à côté d'elle. Un membre la prend par `x.workspace = true`. Le code tiers se choisit consciemment et se réduit (CONTRAT, « Le code tiers »).
- Une crate de module se compile pour WebAssembly avec le profil `module` ; en natif, `jeu::module!` n'y écrit rien.
- Identifiants, commentaires et documents en français.

## Construire, tester, éprouver

Depuis ce dossier :

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p devine-module --target wasm32-unknown-unknown --profile module
cargo run --release -p epreuve --example natif -- devine target/wasm32-unknown-unknown/module/devine_module.wasm
```

- Les modules des fautifs se compilent de même (`-p fautif-<nom>-module`), et `cargo run --release -p epreuve --example natif -- <nom> <module>` les éprouve avec leur copie native. `crates/epreuve/tests/epreuve` vérifie, sur le plan court, que chacun fait échouer exactement son contrôle.
- Les tests de `epreuve` prennent l'essentiel de la durée de `cargo test` (près de deux minutes le 2026-11-04, voir `conception/MESURES-PHASE-1.md`).
- Sur une machine de 16 Go, une seule commande lourde à la fois : deux compilations de wasmtime en parallèle saturent la mémoire. `CARGO_BUILD_JOBS=4` la borne encore.
- Quand un autre agent travaille dans le même dossier, préfère `cargo … -p <crate>` au workspace entier, et ne touche qu'aux fichiers de ta tâche.

## Les contrôles et leurs négatifs

- **Chaque contrôle de l'épreuve a un contrôle négatif** : un jeu de `crates/exemples/fautifs` qui doit le faire échouer, et lui seul (§ 7.2 ; OUT-27, HIS-03, SAL-168). Un contrôle de plus vient avec son fautif, sa crate de module dans `exemples/fautifs/modules/<nom>/`, son entrée dans l'exemple `natif` et son test dans `epreuve/tests/epreuve`.
- Un contrôle qui n'a pas tourné n'a pas réussi ; une panne de l'outil n'est pas un échec du jeu ; une exigence sans objet dit sa raison (OUT-01, OUT-02, OUT-07, HIS-05, HIS-06). Ne fais pas passer l'un pour l'autre.
- Un plan réduit (`--court`, `--controles`) reste visible dans le rapport. Ne présente pas son succès comme celui du plan standard.
- Un test qui existe à cause d'une leçon cite son identifiant (`SAL-`, `OUT-`, `PUN-`…) dans son nom ou son commentaire.

## Les mesures

- Aucun chiffre n'est présenté comme mesuré s'il ne l'est pas. Une mesure porte sa date, sa machine et la commande qui la refait.
- Les budgets que juge E19 (`epreuve::BUDGET_ENTREE`, `BUDGET_LECTURE`, `BUDGET_IMAGE`) sont des points de départ choisis. Les mesures de `conception/MESURES-PHASE-1.md` sont des propositions ; **c'est le mainteneur qui fixe les budgets, dans `CONTRAT.md`**.
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
| `conception/proposition-*.md` | les trois propositions d'où vient INTERFACE.md ; elles ne font plus foi |
| `crates/README.md` | la carte des crates, leur graphe, ce qui est figé |
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
