# Pour l'agent qui modifie la console

> Ce fichier s'adresse à l'agent de code qui travaille dans ce dépôt, quel qu'il soit. Tous les jeux de la soirée dépendent de ce dépôt.

Lis `MANIFESTE.md`. Ne le modifie pas sans demande explicite de l'hôte.

## Ce que contient ce dépôt

| Dossier | Contenu |
|---|---|
| `regles/` | les règles : `rust-toolchain.toml`, `clippy.toml`, `lints.toml`. L'outil les embarque. |
| `outil/` | l'outil `soiree` : `new`, `check`, `publish`, `rules`, `manifeste`, et `games` pour l'hôte |
| `REJOINDRE.md` | le texte qu'un ami colle dans son assistant IA pour commencer |
| `telecommande/` | la brique « télécommande » (`host` et `pages` sur l'ordinateur, `web` dans le navigateur) |
| `modele/` | le jeu de départ, un vrai jeu avec son propre workspace ; `soiree new` le copie |
| `dossier/` | les modèles des fichiers du dossier de l'hôte |

## Changer la console, c'est changer tous les jeux

- **Durcir une règle ou changer une brique fait échouer `soiree check` dans les jeux.** C'est voulu : c'est ainsi que la console force les jeux à suivre. Fais-le exprès, pour une raison que l'hôte a constatée, pas par précaution.
- **Chaque refus de `soiree check` dit quoi corriger**, assez clairement pour que l'agent d'un jeu le fasse seul, sans connaître ce dépôt.
- **Une règle se vérifie par la machine.** Si l'outil ne peut pas la vérifier, elle ne va pas dans le manifeste.
- **La mécanique va dans les outils, le sens dans les textes.** Une commande, une adresse, une étape à suivre : l'outil la dit au moment où elle sert. Les `AGENTS.md` et `REJOINDRE.md` ne gardent que le but et ce qui compte.
- **Avant d'ajouter une brique**, demande-toi si un seul jeu en a besoin. Si oui, sa place est dans ce jeu.

## Les règles

Leur source est `regles/`. La console se les applique : `Cargo.toml`, `rust-toolchain.toml` et `clippy.toml`, à la racine et dans `modele/`, en sont des copies. Un test de l'outil vérifie qu'elles sont identiques. Après un changement dans `regles/`, recopie la table dans le `Cargo.toml` de la racine et lance `soiree rules modele`.

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
