# Mon jeu

> Ce fichier s'adresse à l'agent de code qui fait ce jeu, quel qu'il soit. La personne avec qui tu travailles en est l'auteur : ce dépôt est le sien.

## À quoi sert ce jeu

Il se joue en soirée, chez un ami, sur sa « console » : son ordinateur branché à la télé, et un téléphone par joueur. La console fixe quelques règles, les mêmes pour tous les jeux.

- `soiree manifeste` donne le but et les règles. Lis-le avant d'écrire du code.
- `soiree help` donne les commandes.

## Ce qui compte

- **Le jeu doit donner envie d'y rejouer.** L'idée, les règles et le style sont à toi et à l'auteur. Le code de départ n'est qu'un exemple : garde-le, change-le ou remplace-le.
- **Le jeu est une cartouche : une machine et des pages.** `soiree contrat` dit ce que chacune doit être ; lis-le avant d'écrire du code. La console est le programme : c'est elle qui reçoit les téléphones, donne le temps et fait tourner la machine. `jeu/src/main.rs` est le même dans tous les jeux, n'y touche pas.
- **Les règles du jeu vivent dans la machine** (`machine/`), et nulle part ailleurs : le salon, les places, les tours, les scores, qui a le droit de quoi. Tout ce qui lui arrive passe par une seule porte (`apply`) : une page arrive, part, envoie une commande, ou le temps passe. Elle se passe du système (`#![no_std]`) et tout son état s'écrit et se relit. C'est ce qui permet à la console de jouer des parties toute seule, de rejouer une partie de soirée et de reproduire un bug.
- **`suggest` dit ce que chaque page pourrait envoyer maintenant.** Tiens-le à jour quand le jeu grandit : c'est grâce à lui que les parties que joue `soiree check` avancent jusqu'au bout.
- **Un cas impossible se note, il ne se cache pas.** Une page peut envoyer n'importe quoi : ce qui n'a pas de sens s'ignore sans bruit. Mais un état que le jeu croyait impossible se note avec `cartouche::oups!("…")` avant de continuer.
- **Les pages ne décident de rien.** Elles montrent la vue que la machine leur donne, jouent ses effets (un son, une vibration) et envoient des commandes, le plus brut possible.
- **La machine ne fait rien dehors : elle demande.** La date, une question à internet, les octets d'un contenu, une sauvegarde : `asks` liste ce qu'elle veut (`cartouche::Ask`), la console le fait, et la réponse revient par `answer`. Prévois toujours le cas sans réponse (pas d'internet ce soir) : le jeu continue. Une clé secrète n'est jamais dans le code : écris `{NOM}` dans un en-tête, l'hôte confie la clé à la console. `soiree contrat` dit le reste.
- **Ce qui est lourd ne voyage pas dans une commande.** Une image ou un son qu'un joueur apporte : la page le confie à la console (`telecommande::web::upload`), et la commande, l'état et la vue n'en portent que le ticket (`cartouche::Content`).
- **Le jeu a un titre et une jaquette à lui.** Le titre et le nombre de joueurs sont dans `[workspace.metadata.soiree]` du `Cargo.toml` ; la jaquette est `jaquette.png`, à la racine. C'est ce qu'on voit sur l'étagère de la soirée, au moment de choisir à quoi jouer : fais-la avec l'auteur dès que le jeu a trouvé son idée. `soiree check` refuse celle du jeu de départ.
- **Échap maintenu appartient à la console.** Sur l'écran, il ouvre son menu (pause, recommencer, quitter), le même pour tous les jeux : le jeu n'a pas à savoir se quitter. Un appui court sur Échap reste au jeu. S'il en fait sa pause, elle doit vraiment arrêter la partie : `telecommande::web::pause()` demande à la console d'arrêter le temps pour tout le monde, de figer l'image et le son, et `resume()` le rend. Le jeu n'a plus qu'à montrer ses boutons.
- **La page du téléphone est libre.** Boutons, dessin au doigt, glisser, gyroscope, vibrations : tout ce qu'un navigateur de téléphone permet.
- **Joue une partie entière avant de dire qu'une étape est finie.** Au démarrage, le jeu affiche les adresses à ouvrir, pour l'écran et pour plusieurs joueurs.
- **`soiree check` décide si le jeu est joué à la soirée.** Quand il refuse, il dit quoi corriger : suis-le.
- **`soiree publish` met le jeu sur GitHub.** C'est sa sauvegarde, et c'est là que l'hôte le récupère. Fais-le tôt et souvent.

## Travailler avec l'auteur

- **Il ne code peut-être pas.** Demande-le-lui au début. S'il ne code pas, parle-lui du jeu (les règles, ce qui fait rire), pas de Rust, et montre-lui le jeu qui tourne plutôt que du code.
- **Le jeu reste jouable à tout moment.** Avance par petites étapes qui marchent.

## Ce jeu

*À remplir par toi, au fil du projet : en une phrase, ce qu'est le jeu ; ses règles ; les décisions qu'un autre agent doit connaître avant d'y toucher.*
