# La soirée

Des amis font chacun un jeu avec un agent de code, puis on y joue tous ensemble un soir, chez l'hôte.
L'ordinateur de l'hôte est branché sur la TV ; les téléphones servent de manettes.
Les jeux sont écrits en Rust, tournent dans le navigateur, et tiennent tous le même plancher de qualité.

Ce dépôt est le commun : les idées, la couche réseau, la télécommande, l'outil `soiree` et le jeu de départ. Chaque jeu vit dans son propre dépôt, dans l'organisation `build-your-own-game`.

Tu ne codes pas ? Commence par [`REJOINDRE.md`](REJOINDRE.md) : donne-le à ton assistant IA, il te guidera.

## Commencer un jeu

Ces étapes s'adressent à toi ou à ton agent de code. Chaque commande se lance dans un terminal.

1. **Installer Rust**, avec rustup : suis https://rustup.rs. Ferme puis rouvre le terminal.
2. **Installer l'outil `soiree`**, à la version de la soirée :
   ```sh
   cargo install --locked --git https://github.com/build-your-own-game/soiree --tag v0.2.0 soiree
   ```
   La première installation prend quelques minutes.
3. **Vérifier l'ordinateur** :
   ```sh
   soiree doctor
   ```
   Il dit ce qui manque (la bonne version de Rust, la cible WebAssembly, wasm-bindgen, Node, Chrome, WebKit, Toxiproxy, git) et donne pour chacun le lien officiel. Il n'installe rien lui-même. Installe ce qui manque, puis relance-le jusqu'à ce qu'il soit tout vert.
4. **Créer le jeu**, dans le dossier où tu ranges tes projets :
   ```sh
   soiree new mon-jeu
   cd mon-jeu
   ```
   Le nom : des minuscules, des chiffres et des tirets, comme `course-de-canards`.
5. **Vérifier le jeu, créer son dépôt dans l'organisation et pousser.**
   Le premier contrôle crée aussi `Cargo.lock`, nécessaire à la CI.
   Avec [GitHub CLI](https://cli.github.com) :
   ```sh
   soiree check
   git init -b main
   git add -A
   git commit -m "Le jeu de départ"
   gh repo create build-your-own-game/mon-jeu --private --source . --push
   ```
   Sans `gh` : crée le dépôt `mon-jeu` sur https://github.com/organizations/build-your-own-game/repositories/new, puis suis les commandes que GitHub affiche pour « push an existing repository ».
6. **Lire `AGENTS.md`**, à la racine du jeu. Si tu travailles avec un agent de code, demande-lui de le lire et de le suivre : c'est lui qui décrit la suite.

C'est fini quand `soiree check` passe dans le dossier du jeu.

## Les commandes

| Commande | Quand | Durée | Ce qu'elle fait |
|---|---|---|---|
| `soiree doctor` | à l'installation | secondes | vérifie l'ordinateur et dit quoi installer |
| `soiree new <nom>` | une fois par jeu | secondes | crée le jeu depuis `modele/` |
| `soiree check` | après chaque changement | secondes à minutes | formatage, Clippy, tests, règles communes, items du jeu reliés à leur test |
| `soiree check --fast` | en boucle serrée | secondes | les mêmes contrôles, sans formatage, Clippy ni tests |
| `soiree mutants` | à la demande | variable | vérifie que les tests détectent des mutations ; rapports sans bloquer check/certify |
| `soiree certify` | à la fin d'une étape | 5 à 10 min | le harnais complet ; écrit `certification/` |
| `soiree upgrade` | quand tu veux | secondes à minutes | passe le jeu à la version du binaire installé, après vérification de son étiquette publiée |
| `soiree status` | chez l'hôte | secondes | dans un dossier de plusieurs dépôts : ce qui n'est pas commité ou pas poussé |
| `soiree host-setup` | chez l'hôte, une fois par machine | secondes à minutes | installe le dossier de l'hôte : `AGENTS.md`, le `[patch]` local, `soiree/` et ses jeux |

`soiree help` les rappelle.

## Où trouver le reste

| Pour | Lis |
|---|---|
| les idées, le niveau d'exigence, les principes d'ingénierie | [`MANIFESTE.md`](MANIFESTE.md) |
| ce que chaque jeu tient le soir même | [`CERTIFICATION.md`](CERTIFICATION.md) |
| brancher un jeu sur la TV et les téléphones | [`salon/README.md`](salon/README.md) |
| l'app iPhone qui sert de manette | [`remote/CONTRAT.md`](remote/CONTRAT.md) |
| les jeux de la soirée | [`catalogue.toml`](catalogue.toml) |
| ce qui change d'une version à l'autre | [`CHANGELOG.md`](CHANGELOG.md) |
| modifier le commun | [`AGENTS.md`](AGENTS.md) |

Une question, un besoin dans salon, une règle qui gêne : ouvre une issue sur ce dépôt.
