# Pour l'agent qui modifie la console

> Ce fichier s'adresse à l'agent de code qui travaille dans ce dépôt, quel qu'il soit. Tous les jeux de la soirée dépendent de ce dépôt.

Lis `MANIFESTE.md`. Ne le modifie pas sans demande explicite de l'hôte.

## Ce que contient ce dépôt

| Dossier | Contenu |
|---|---|
| `regles/` | les règles : `rust-toolchain.toml`, `clippy.toml`, `lints.toml`, et `conseils.toml`, ce que `soiree check` dit quand une règle refuse. L'outil les embarque. |
| `outil/` | l'outil `soiree` : `new`, `check`, `publish`, `rules`, `manifeste`, et `games` pour l'hôte |
| `REJOINDRE.md` | le texte qu'un ami colle dans son assistant IA pour commencer |
| `telecommande/` | la brique « télécommande » (`host` et `pages` sur l'ordinateur, `web` dans le navigateur) |
| `iphone/` | l'app iPhone « Soirée » (Swift) : une manette pour tous les jeux. Elle scanne le QR code, charge la télécommande du jeu et lui donne les capteurs de mouvement et les vibrations que Safari refuse. Son contrat avec les pages est dans `iphone/README.md`. |
| `modele/` | le jeu de départ, un vrai jeu avec son propre workspace ; `soiree new` le copie |
| `dossier/` | les modèles des fichiers du dossier de l'hôte |

## Changer la console, c'est changer tous les jeux

- **Durcir une règle ou changer une brique fait échouer `soiree check` dans les jeux.** C'est voulu : c'est ainsi que la console force les jeux à suivre. Fais-le exprès, pour une raison que l'hôte a constatée, pas par précaution.
- **Chaque refus de `soiree check` dit quoi corriger**, assez clairement pour que l'agent d'un jeu le fasse seul, sans connaître ce dépôt.
- **Une règle se vérifie par la machine.** Si l'outil ne peut pas la vérifier, elle ne va pas dans le manifeste.
- **La mécanique va dans les outils, le sens dans les textes.** Une commande, une adresse, une étape à suivre : l'outil la dit au moment où elle sert. Les `AGENTS.md` et `REJOINDRE.md` ne gardent que le but et ce qui compte.
- **Avant d'ajouter une brique**, demande-toi si un seul jeu en a besoin. Si oui, sa place est dans ce jeu.

## Les règles

Leur source est `regles/`. La console se les applique : `Cargo.toml`, `rust-toolchain.toml` et `clippy.toml`, à la racine et dans `modele/`, en sont des copies. Un test de l'outil vérifie qu'elles sont identiques. Après un changement dans `regles/`, recopie la table dans le `Cargo.toml` de la racine, lance `soiree rules modele`, puis recopie `modele/clippy.toml` et `modele/rust-toolchain.toml` à la racine.

- **Un refus oriente.** Quand tu ajoutes ou durcis une règle, écris dans `regles/conseils.toml` pourquoi la console la pose et par où sortir : `soiree check` l'affiche sous le refus. Une règle qui ne dit que « non » pousse l'agent d'un jeu vers une exception.
- **Un principe plutôt qu'une liste.** Avant d'interdire des noms un par un, cherche la vérification qui les refuse tous d'un coup : la machine d'un jeu ne liste pas ce qui est interdit, elle se compile sans système.

## La machine d'un jeu

Chaque jeu déclare sa machine : le crate qui porte ses règles (`[package.metadata.soiree] machine = true`). Même graine et mêmes entrées, même partie, sur l'ordinateur comme dans un navigateur.

- **Par principe** : `soiree check` compile la machine pour une cible sans système (`wasm32v1-none`). L'horloge, les fichiers, le réseau, le hasard du système, `HashMap` et les fonctions mathématiques du système n'y existent pas : aucune liste à tenir, et une dépendance qui s'en sert est refusée aussi. C'est une compilation sans lancement, rapide.
- **`glam`** y est permis avec `libm` et `scalar-math` seulement : sans elles, il calcule avec des instructions propres au processeur et le dernier chiffre change (essai fait : 20 000 pas, différents par défaut, identiques avec ces deux options).
- **Par l'exemple** (`outil/src/machine.rs`) : `soiree check` lance l'exemple `empreinte` de la machine deux fois en natif, puis en WebAssembly (`outil/src/wasm.rs`, un interpréteur embarqué : rien à installer à côté), et compare les lignes. C'est le dernier filet : il attrape ce que le principe ne voit pas, un entier dont la taille change (`usize`), un état global caché, une dépendance qui calcule autrement selon le processeur.
- `modele/machine/` montre la forme attendue, empreinte comprise.

## Avant de dire que c'est fini

Depuis la racine :

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p telecommande --target wasm32-unknown-unknown -- -D warnings
cargo test --workspace
cargo run -p soiree -- check modele
```

Puis `soiree check` dans chaque jeu du dossier de l'hôte (`games/*`). Un jeu qui ne passe plus : soit tu corriges ici, soit c'est le changement voulu et tu remets le jeu à niveau.

## Versions

- Une version de la console est une étiquette git sur ce dépôt (`v0.4.0`, `v0.5.0`…). Son numéro est celui de `[workspace.package]` dans `Cargo.toml`.
- Les jeux empruntent la brique à cette étiquette, et `soiree new` écrit cette dépendance.
- `README.md` et `REJOINDRE.md` nomment l'étiquette à installer. Un test de l'outil vérifie que c'est celle de la version courante.
- Committer, étiqueter et pousser demandent l'accord de l'hôte.
