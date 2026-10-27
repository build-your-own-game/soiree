# {{nom}}

> Ce fichier s'adresse à l'agent de code qui fait ce jeu, quel qu'il soit. La personne avec qui tu travailles en est l'auteur et le seul maître : ce dépôt est le sien.

## Avant d'écrire du code

Lis, dans `.soiree/` (écrit par `soiree`, à la version de `soiree.toml`) :
- `MANIFESTE.md` : le pari, le niveau d'exigence, les principes. Applique-le.
- `CERTIFICATION.md` : le plancher que tout jeu tient le soir même.
- `salon/README.md` : comment la TV, les téléphones et les manettes rejoignent le jeu. Tu ne touches jamais un socket ni une reconnexion : c'est salon.
- `remote/CONTRAT.md`, seulement si le jeu utilise l'app iPhone.

Si `.soiree/` manque, lance `soiree check` : il le recrée. Si `soiree` manque, lance `soiree doctor` après l'avoir installé (voir le README du dépôt commun).

## Deux boucles, deux vitesses

**La boucle rapide, après chaque changement : `soiree check`.** Quelques secondes : formatage, Clippy avec les règles communes, tests, contrôles du manifeste. Elle doit rester verte. Ne lance rien de plus lourd pendant que tu itères.

**La boucle lourde, à la fin d'une étape : `soiree certify`.** 5 à 10 minutes : le vrai serveur, la TV et 8 téléphones dans un navigateur sans écran, coupures, réseau lent, entrées hostiles, endurance. Lance-la en arrière-plan quand l'étape te paraît finie, pas avant. Corrige chaque ✗, relance, puis committe `certification/` avec le code. Un ~ ou un – n'est pas un échec : lis le détail.

Entre les deux, joue : lance le jeu (`cargo run --release -p {{nom}}-host`), ouvre la TV et des téléphones (`/play?seat=1`, `/play?seat=2`…), regarde des captures. Si ton environnement a un navigateur ou un simulateur iOS, sers-t'en.

## La certification propre à ce jeu

`CERTIFICATION.md` à la racine liste ce que **ce jeu-ci** doit tenir, en plus des items communs. Chaque item nomme son test de propriétés ; `soiree check` vérifie que chaque test nommé existe et passe.

Ne pars pas d'une page blanche : `.soiree/certifications/` est un catalogue d'items rangés par mécanique (physique, vote, tour par tour, score…), chacun tiré d'un vrai bug de jeu.
1. Repère les mécaniques de ton jeu et lis les fichiers correspondants.
2. Prends **tous** les items qui s'appliquent, adapte-les à ton jeu, et note `origine: <ID>`.
3. Un item que tu écartes alors que sa mécanique est dans ton jeu : dis pourquoi en une ligne, dans `CERTIFICATION.md`.
4. Refais ce tri quand le jeu gagne une mécanique.

Ajoute ensuite ce que le catalogue n'a pas : chaque règle de ton jeu qui peut casser devient un item. Bons items : ce qu'un joueur pressé, un téléphone en retard ou une égalité peuvent casser.

## Un bug trouvé en jouant

1. Reproduis-le : une suite d'entrées ou un replay, qui devient un test.
2. Écris dans `LEARNINGS.md` pourquoi aucun item ne l'a vu.
3. Ajoute ou durcis un item dans le `CERTIFICATION.md` du jeu. Si la faille n'est pas propre à ton jeu, propose-la au catalogue, sans attendre qu'on te le demande : ouvre une pull request sur le dépôt commun qui ajoute l'item au bon fichier de `certifications/`, au format du catalogue, avec un lien vers le test de ton jeu qui l'a trouvée. L'hôte décide de l'ajouter ou non ; en attendant, l'item reste dans le `CERTIFICATION.md` de ton jeu. Si tu ne peux pas ouvrir de pull request toi-même, écris-en le contenu pour l'auteur.

## Ce que tu ne modifies pas

- Le dépôt commun (salon, la télécommande, les règles, le harnais). S'il manque quelque chose à salon, ou si une règle commune gêne sans raison, ouvre une issue sur `https://github.com/{{org}}/soiree` qui explique le besoin et ce que tu as essayé. Si tu ne peux pas ouvrir d'issue toi-même, écris-en le texte pour l'auteur.
- `rust-toolchain.toml`, `clippy.toml` et les lints de `Cargo.toml` : `soiree` les écrit. Tu peux durcir une règle ; pour en assouplir une, il faut `#[expect(lint, reason = "…")]` à l'endroit précis, jamais `#[allow]`.
- `.soiree/` : régénéré à chaque `soiree check`.

## Travailler avec l'auteur

- **Il ne code peut-être pas.** Demande-le-lui au début s'il ne l'a pas dit. S'il ne code pas, c'est toi le développeur : parle-lui du jeu (règles, sensation, ce qui fait rire), pas de Rust. Montre-lui le jeu qui tourne plutôt que du code.
- **Le jeu reste jouable à tout moment.** Avance par petites couches qui marchent.
- **Committe et pousse souvent,** avec des messages qui disent ce qui change pour le joueur. Ce dépôt est sur GitHub : c'est sa sauvegarde et c'est là que l'hôte le récupère.
- **Pour que le jeu soit proposé à la soirée,** il doit être dans `catalogue.toml` du dépôt commun : ouvre une pull request qui y ajoute une ligne, une seule fois.

## Ce jeu

*À remplir par toi, au fil du projet : en une phrase, ce qu'est le jeu ; les commandes pour le lancer et le tester ; ce que contient chaque crate ; les décisions qu'un autre agent doit connaître avant d'y toucher.*
