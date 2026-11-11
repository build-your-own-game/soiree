# La console de nos soirées

**Ce document ne doit être modifié que sur demande explicite de l'hôte.**

> Ce fichier s'adresse à toi, qui fais un jeu, et à l'agent de code qui l'écrit avec toi.

## Le but

On est plusieurs amis. Chacun fait coder un jeu par son agent. Un soir, on y joue tous ensemble chez l'hôte.

## Une console

Ce dépôt est une console de jeu. Une console impose peu de choses, mais les mêmes à tous les jeux : c'est ce qui fait qu'un jeu se branche et marche du premier coup. Elle prête aussi des briques toutes faites. Et elle évolue : quand elle change, les jeux suivent.

Tout le reste t'appartient : l'idée, les règles du jeu, le style, l'architecture.

## Se brancher

- **Une commande lance le jeu** sur l'ordinateur de l'hôte, la même pour tous : `cargo run --release`.
- **Le jeu s'affiche dans un navigateur**, sur l'écran de cet ordinateur, relié à la télé.
- **Un jeu peut avoir une télécommande** : une page que chaque joueur ouvre dans le navigateur de son téléphone. Tu la codes comme tu veux : boutons, dessin, glisser, vibrations.
- **Le jeu écoute sur le port que la console lui donne** (la variable `PORT`). Il ne choisit pas son adresse : c'est l'hôte qui décide par où passent les téléphones, le Wi-Fi de la maison ou un relais en ligne.

## Les règles

Toutes sont vérifiées par la machine.

- **Tout est en Rust**, pages comprises (compilées pour le navigateur), à la version fixée par la console.
- **Le formatage** est celui de `cargo fmt`.
- **Le linteur est dur** : Clippy `all` et `pedantic` en erreur.
  - Pas de `unsafe`.
  - Pas de `unwrap`, `expect`, `panic` ni d'index sans garde : ils arrêtent la partie.
  - Pas de `_ =>` sur une enum : il avale le cas qu'on ajoutera demain.
- **Une exception se justifie sur place** : `#[expect(lint, reason = "…")]`.

La liste complète, avec la raison de chaque règle, est dans `regles/`.

## Les briques

Du code que la console prête. Aujourd'hui il y en a une, celle dont se sert le jeu de départ : le téléphone trouve le jeu par un QR code et lui parle.

Tu peux l'emprunter ou faire autrement. Une brique ne devient obligatoire que le jour où une règle le dit.

## La vérification

`soiree check`, dans le dossier du jeu. Un jeu qui ne passe pas n'est pas joué à la soirée. Chaque refus dit quoi corriger, assez clairement pour que ton agent le fasse seul.

## La console évolue

La soirée a une version de la console. Tout jeu joué passe la vérification de cette version.

Quand une soirée montre un manque, l'hôte ajoute une règle ou une brique. Ton jeu ne passe alors plus : demande à ton agent de le remettre à niveau, la vérification lui dit quoi changer.
