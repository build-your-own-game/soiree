# Pour l'agent qui modifie la console

> Ce fichier s'adresse à l'agent de code qui travaille dans ce dépôt, quel qu'il soit. Tous les jeux de la soirée dépendent de ce dépôt.

Lis `MANIFESTE.md`, puis `CONTRAT.md`. Ne les modifie pas sans demande explicite de l'hôte.

## Ce que contient ce dépôt

| Dossier | Contenu |
|---|---|
| `regles/` | les règles : `rust-toolchain.toml`, `clippy.toml`, `lints.toml`, et `conseils.toml`, ce que `soiree check` dit quand une règle refuse. L'outil les embarque. |
| `outil/` | l'outil `soiree` : `new`, `check`, `publish`, `rules`, `manifeste`, et `games` pour l'hôte |
| `REJOINDRE.md` | le texte qu'un ami colle dans son assistant IA pour commencer |
| `CONTRAT.md` | le contrat de la cartouche : ce qu'un jeu fournit, ce que la console fournit. L'outil l'embarque (`soiree contrat`). |
| `cartouche/` | la brique « cartouche » : le contrat (`Machine`, `State`, `Input`), le tirage au hasard des commandes (`draw`), le journal d'une partie (`journal`) et le banc qui joue des parties (`bench`). Sans système, comme la machine qui l'emprunte. |
| `telecommande/` | la brique « télécommande » : le programme de tous les jeux (`console`, sur l'ordinateur : il fait tourner la machine, sert les pages, écrit le journal) et le lien d'une page vers le jeu (`web`, dans le navigateur) |
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

## Le contrat de la cartouche

Un jeu est une cartouche : une machine et des pages. La console est le programme. `CONTRAT.md` dit ce que chacun fournit ; il est fait pour ne plus bouger.

- **Le contrat se change par ajout.** Une méthode nouvelle avec une valeur par défaut, une porte de plus : aucun jeu n'a à changer. Retirer ou changer le sens d'une ligne du contrat oblige à réécrire tous les jeux : ne le fais que sur demande de l'hôte.
- **Ce que la console apprend à faire s'appuie sur les propriétés du contrat**, sans rien demander aux jeux : rejouer une partie, la reprendre, la rattraper. Avant d'ajouter quelque chose au contrat, cherche si la console peut le faire seule.
- **Une contrainte tient par construction, pas par une règle.** Le jeu n'a pas de programme à lui (`src/main.rs` est le même pour tous, `soiree check` le vérifie) : il n'y a donc nulle part où cacher une règle du jeu hors de la machine. La machine se compile sans système : elle ne peut pas lire l'horloge.

## La machine d'un jeu

Chaque jeu déclare sa machine : le crate qui porte ses règles (`[package.metadata.soiree] machine = true`) et dont le type `Jeu` remplit `cartouche::Machine`. Même graine et mêmes entrées, même partie, sur l'ordinateur comme dans un navigateur.

- **Par principe** : `soiree check` compile la machine pour une cible sans système (`wasm32v1-none`). L'horloge, les fichiers, le réseau, le hasard du système, `HashMap` et les fonctions mathématiques du système n'y existent pas : aucune liste à tenir, et une dépendance qui s'en sert est refusée aussi. C'est une compilation sans lancement, rapide.
- **`glam`** y est permis avec `libm` et `scalar-math` seulement : sans elles, il calcule avec des instructions propres au processeur et le dernier chiffre change (essai fait : 20 000 pas, différents par défaut, identiques avec ces deux options).
- **Par le banc** (`cartouche/src/bench.rs`, lancé par `outil/src/machine.rs`) : l'exemple `banc` de la machine, le même fichier dans tous les jeux, joue des parties sans une ligne écrite par le jeu. Des pages arrivent et partent, envoient ce que la machine suggère (`Machine::suggest`) ou des commandes tirées au hasard depuis leur type (`cartouche::draw`), le temps passe par rafales. `soiree check` le lance deux fois en natif, puis en WebAssembly (`outil/src/wasm.rs`, un interpréteur embarqué : rien à installer à côté), et compare les lignes d'empreinte. Puis `--long`, en natif : plus de parties, la reprise depuis la photo comparée à la partie jouée d'une traite, le temps de chaque pas.
- **L'empreinte** est celle des octets de l'état (`cartouche::fingerprint`) : le jeu ne l'écrit plus. Un champ oublié n'existe plus ; un état caché hors de la photo se voit à la reprise.
- **Un jeu peut garder un exemple `empreinte`**, pour des parties écrites d'avance qui vont là où le hasard ne va pas : `soiree check` le lance comme le banc.
- `modele/machine/` montre la forme attendue.

## Ce qui tourne pendant la soirée

`telecommande::console::run` est le programme de tous les jeux. Il donne le temps (`TICKS_PER_SECOND`, au plus un dixième de seconde de rattrapage), n'envoie une vue que si elle a changé (au plus soixante par seconde), porte les effets une fois, et écrit dans le dossier du jeu, sous `.soiree/` :

- **le journal** de chaque lancement (`.soiree/journal/`) : la graine et chaque entrée, avec le numéro de son pas. `soiree rejouer <fichier>` refait la partie et vérifie ses repères ;
- **l'état gardé** (`.soiree/etat`) des jeux qui le demandent (`REMEMBERS`). Un état illisible est mis de côté, jamais écrasé.

## Avant de dire que c'est fini

Depuis la racine :

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p telecommande --target wasm32-unknown-unknown -- -D warnings
cargo check -p cartouche --lib --target wasm32v1-none
cargo test --workspace
cargo run -p soiree -- check modele
```

Puis `soiree check` dans chaque jeu du dossier de l'hôte (`games/*`). Un jeu qui ne passe plus : soit tu corriges ici, soit c'est le changement voulu et tu remets le jeu à niveau.

## Versions

- Une version de la console est une étiquette git sur ce dépôt (`v0.4.0`, `v0.5.0`…). Son numéro est celui de `[workspace.package]` dans `Cargo.toml`.
- Les jeux empruntent les deux briques à cette étiquette, et `soiree new` écrit ces dépendances.
- `README.md` et `REJOINDRE.md` nomment l'étiquette à installer. Un test de l'outil vérifie que c'est celle de la version courante.
- Committer, étiqueter et pousser demandent l'accord de l'hôte.
