# Pour l'agent qui modifie le commun

> Ce fichier s'adresse à l'agent de code qui travaille dans ce dépôt, quel qu'il soit. Ce dépôt est public, et tous les jeux de la soirée en dépendent.

Lis et applique `MANIFESTE.md` et `CERTIFICATION.md`.

## Chaque changement touche tous les jeux

Chaque jeu dépend d'une étiquette de ce dépôt. Une version figée ici, c'est ce que des amis compilent chez eux, avec un agent qui n'a jamais vu ce dépôt. Donc :
- Un changement qui casse un jeu existant est une nouvelle version majeure, même petit, et il dit comment s'adapter.
- Avant d'ajouter quelque chose, demande-toi si un seul jeu en a besoin. Si oui, sa place est peut-être dans ce jeu.
- Durcir une règle commune fait échouer `soiree check` chez tout le monde. Fais-le exprès, et dis-le dans `CHANGELOG.md`.

## Ce que contient ce dépôt

| Dossier | Pour | Ses règles |
|---|---|---|
| `salon/` | la couche réseau commune | `salon/AGENTS.md` |
| `remote/` | la télécommande iPhone et son contrat | `remote/AGENTS.md` |
| `outil/` | le CLI `soiree`, qui embarque les règles (`outil/regles/`), le dossier de l'hôte (`outil/dossier/`), le modèle, le catalogue et le harnais | ce fichier |
| `tools/cert/` | le harnais de certification (Node, Playwright, Toxiproxy), embarqué par `soiree certify` | ce fichier |
| `certifications/` | le catalogue d'items par mécanique | son `README.md` ; `soiree check .` vérifie son format |
| `modele/` | le jeu de départ que copie `soiree new` | ce fichier |
| `catalogue.toml` | les jeux de la soirée | une ligne par jeu, ajoutée par pull request |

## Salon

Reprises de `salon/AGENTS.md`, qui reste la référence :
- La spécification est la documentation de `salon/crates/core/src/lib.rs`. Une garantie qu'elle annonce a son test dans `salon/crates/core/tests/` ; une garantie nouvelle aussi.
- `salon-core` reste `#![no_std]`, sans horloge ni hasard : le temps et les octets entrent par les appels.
- Chaque leçon tirée d'une référence va dans `salon/LEARNINGS.md`, avec sa source.
- Les jeux ne contiennent aucune logique réseau : ni jeton, ni reconnexion, ni battement, ni limite de débit. Si un jeu en a besoin, c'est dans salon qu'elle manque.

## Les règles et le harnais

- Les règles (toolchain, lints, `clippy.toml`) sont dans `outil/regles/`, embarquées dans le binaire. `MANIFESTE.md` garde les idées, pas les fichiers de config.
- Un item commun de `CERTIFICATION.md` a son scénario dans le harnais, ou sa garantie dans salon. Un item sans vérification n'est pas un item : c'est une intention.
- Le harnais ne connaît un jeu que par son `cert.json`. N'y ajoute pas de cas propre à un jeu.

Les tests de l’outil exercent réellement une petite campagne de mutations du modèle.
Avant `cargo test` dans `outil/`, installe `cargo-mutants` à la version documentée
par `soiree mutants --help` (`cargo install --locked cargo-mutants --version 27.1.0`).
Ce prérequis concerne le développement du commun ; `soiree check` et
`soiree certify` d’un jeu ne lancent jamais les mutations.

## Versions

- Une version, c'est une étiquette sur ce dépôt : `v0.1.0`, `v0.2.0`… Elle fige ensemble salon, la télécommande, les règles, le harnais et le modèle.
- On suit semver. Tant qu'on est en `0.x`, un changement qui casse un jeu monte le chiffre du milieu (`v0.1.3` → `v0.2.0`) ; une correction qui ne casse rien monte le dernier.
- La version du paquet `soiree` (`outil/Cargo.toml`) est celle de l'étiquette. `soiree check` refuse un jeu dont `soiree.toml` demande une autre version.
- Une étiquette publiée ne bouge plus. Une erreur se corrige par une nouvelle version.

## `CHANGELOG.md`

Chaque version y a sa section, de la plus récente à la plus ancienne :

```markdown
## v0.2.0 — 2026-11-14

Ce qui change, en deux ou trois phrases, pour quelqu'un qui fait un jeu.

### Passer à cette version

1. `soiree upgrade`.
2. `DeviceEvent::Left` porte maintenant `why` : ajoute `..` aux motifs qui le déstructurent.
3. `soiree check`, puis `soiree certify`.
```

« Passer à cette version » s'adresse à l'agent d'un jeu, qui ne connaît pas ce dépôt. Écris des étapes qu'il peut suivre sans rien deviner : le nom exact de ce qui change, le code avant et après s'il le faut. S'il n'y a rien à faire au-delà de `soiree upgrade`, écris-le.

## Avant d'étiqueter

Dans l'ordre, et tout doit être vert :

1. Avec le binaire de ce que tu vas étiqueter : `cargo install --locked --path outil`, depuis la racine de ce dépôt.
2. `soiree check salon`, `soiree check remote` et `soiree check .` (le catalogue). Dans `outil/` : `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
3. `soiree check` puis `soiree certify` sur `modele/`.
4. Les mêmes sur chaque jeu du dossier de l'hôte (`games/*`). Le `[patch]` de son `.cargo/config.toml` les fait compiler contre ce salon-ci, avant toute étiquette. Un jeu qui casse : soit tu corriges ici, soit la section « Passer à cette version » dit exactement quoi changer dans le jeu.
5. `CHANGELOG.md` a sa section, avec « Passer à cette version ».
6. La version de `outil/Cargo.toml` est celle de l'étiquette.

Puis : commit, étiquette, `git push --follow-tags`, avec l'accord de l'hôte. Ensuite seulement, les jeux de l'hôte passent à l'étiquette (voir l'`AGENTS.md` du dossier de l'hôte).

## Le modèle

`modele/` est ce que reçoit un ami qui commence. Il passe `soiree check` et `soiree certify` à chaque version. Son `AGENTS.md` est écrit pour un agent qui n'a jamais vu ce dépôt, et pour un auteur qui ne code peut-être pas. `{{nom}}` y est remplacé par le nom du jeu, `build-your-own-game` par l'organisation, `{{version}}` par la version.
