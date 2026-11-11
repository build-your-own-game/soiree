# Mon jeu

> Ce fichier s'adresse à l'agent de code qui fait ce jeu, quel qu'il soit. La personne avec qui tu travailles en est l'auteur : ce dépôt est le sien.

Ce jeu se joue en soirée, chez un ami, sur sa « console » : son ordinateur branché à la télé, et un téléphone par joueur. La console fixe quelques règles, les mêmes pour tous les jeux. Lance `soiree manifeste` et lis-le avant d'écrire du code.

## Les trois commandes

| Commande | Quand |
|---|---|
| `cargo run --release` | pour lancer le jeu et y jouer |
| `soiree check` | avant de dire qu'une étape est finie |
| `soiree rules` | quand `soiree check` dit que les règles ne sont pas en place |

`soiree check` vérifie les règles, le formatage, le linteur, les tests, puis lance le jeu et regarde s'il sert sa page. Chaque refus dit quoi corriger : suis-le. Un jeu qui ne passe pas n'est pas joué à la soirée.

## Jouer pour de vrai avant de dire que c'est fini

`soiree check` ne joue pas à ta place. Lance le jeu, ouvre l'écran et plusieurs télécommandes dans un navigateur, et fais une partie entière :

- l'écran : `http://localhost:8080/`
- un premier joueur : `http://localhost:8080/t?j=1`
- un deuxième : `http://localhost:8080/t?j=2`, et ainsi de suite.

Chaque numéro après `?j=` est un téléphone différent. Sans la variable `PORT`, le jeu écoute sur le port 8080.

## Comment ce jeu est rangé

| Dossier | Contenu |
|---|---|
| `jeu/` | le programme que lance `cargo run --release` : les règles du jeu (`game.rs`) et la boucle qui les relie aux pages (`main.rs`) |
| `messages/` | les messages échangés : `Up` (d'une page vers le jeu) et `Down` (du jeu vers une page) |
| `pages/` | l'écran et la télécommande, en Rust compilé pour le navigateur |
| `web/` | `index.html`, la feuille de style, les images |

Rien de ce rangement n'est imposé. Tu peux tout réorganiser tant que `soiree check` passe.

## La télécommande

La page que les joueurs ouvrent sur leur téléphone est à toi : boutons, dessin au doigt, glisser, gyroscope, vibrations, tout ce qu'un navigateur de téléphone permet. Elle est en Rust, comme le reste.

## La brique `telecommande`

Ce jeu emprunte une brique à la console. Elle fait trois choses :

- `telecommande::pages::build` compile les pages au démarrage du jeu ;
- `telecommande::host::Room` sert les pages, affiche le QR code (`/qr.svg`) et échange des messages avec elles : `Joined`, `Message`, `Left` ;
- `telecommande::web::connect` ouvre, dans une page, le lien vers le jeu, et le rouvre après une coupure.

À retenir :

- **`Joined` arrive à chaque connexion**, aussi quand un téléphone revient de veille. Renvoie-lui alors ce qu'il doit afficher.
- **Un téléphone garde son identité** (`Id`) quand il recharge la page. Garde sa place et son score.
- **`Left` ne veut pas dire que le joueur a quitté la partie** : son téléphone s'est peut-être mis en veille dix secondes.

Sa documentation complète est dans son code. Tu peux aussi t'en passer et faire autrement : la brique n'est pas obligatoire.

## Ce que tu ne modifies pas

`rust-toolchain.toml`, `clippy.toml` et la table `[workspace.lints]` de `Cargo.toml` : ce sont les règles de la console, écrites par `soiree rules`. Pour une exception, écris `#[expect(lint, reason = "…")]` à l'endroit précis, jamais `#[allow]`.

## Travailler avec l'auteur

- **Il ne code peut-être pas.** Demande-le-lui au début. S'il ne code pas, parle-lui du jeu (les règles, ce qui fait rire), pas de Rust, et montre-lui le jeu qui tourne plutôt que du code.
- **Le jeu reste jouable à tout moment.** Avance par petites étapes qui marchent.
- **Committe et pousse souvent.** Le dépôt GitHub est sa sauvegarde, et c'est là que l'hôte récupère le jeu.

## Ce jeu

*À remplir par toi, au fil du projet : en une phrase, ce qu'est le jeu ; ses règles ; les décisions qu'un autre agent doit connaître avant d'y toucher.*
