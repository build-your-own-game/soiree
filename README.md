# La soirée

Des amis font chacun un jeu avec un agent de code, puis on y joue tous ensemble un soir, chez l'hôte.
L'ordinateur de l'hôte est branché sur la TV ; les téléphones servent de manettes.
Les jeux sont écrits en Rust, tournent dans le navigateur, et tiennent tous le même plancher de qualité.

Ce dépôt est le commun : les idées, la couche réseau, la télécommande, l'outil `soiree` et le jeu de départ. Chaque jeu vit dans son propre dépôt, dans l'organisation `build-your-own-game`.

Tu ne codes pas ? Commence par [`REJOINDRE.md`](REJOINDRE.md) : donne-le à ton assistant IA, il te guidera.

La version **0.3.0** introduit les contrats de vérification exécutables.
Leur portée et leurs limites sont décrites dans [`VERIFICATION.md`](VERIFICATION.md) ;
les [résultats de validation](certification/contracts-v0.3.0/README.md) distinguent
les contrôles satisfaits des garanties qui restent non attestées.

## Commencer un jeu

Ces étapes s'adressent à toi ou à ton agent de code. Chaque commande se lance dans un terminal.

1. **Installer Rust**, avec rustup : suis https://rustup.rs. Ferme puis rouvre le terminal.
2. **Installer l'outil `soiree`**, à la version de la soirée :
   ```sh
   cargo install --locked --git https://github.com/build-your-own-game/soiree --tag v0.3.0 soiree
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

| Commande | Quand | Ce qu'elle fait |
|---|---|---|
| `soiree doctor` | à l'installation | vérifie l'ordinateur et dit quoi installer |
| `soiree new <nom>` | une fois par jeu | crée le jeu depuis `modele/` |
| `soiree contracts` | avant de vérifier | résout les engagements de `verification.toml` et les minima communs en plan |
| `soiree contracts --write-view` | après un changement d’engagement | actualise la vue dans `CERTIFICATION.md` |
| `soiree check --fast` | en boucle serrée | contrôle les déclarations et le socle statique ; les tests restent non attestés |
| `soiree check` | avant de valider le code | ajoute formatage, Clippy natif/WASM, tests réellement découverts par Cargo/libtest et doctests |
| `soiree mutants` | à la demande | vérifie que les tests détectent des mutations ; rapports séparés |
| `soiree certify` | pour éprouver le jeu réel | exécute les scénarios ordinaires du harnais ; écrit les observations et leur évaluation dans `certification/` |
| `soiree certify -- --only ui,panic` | pour un retour ciblé | atteste uniquement les scénarios demandés et la santé du contrôleur |
| `soiree upgrade` | après une publication | passe le jeu à la version du binaire installé, après vérification de son étiquette publiée |
| `soiree status` | chez l'hôte | dans un dossier de plusieurs dépôts : ce qui n'est pas commité ou pas poussé |
| `soiree host-setup` | chez l'hôte, une fois par machine | installe le dossier de l'hôte : `AGENTS.md`, le `[patch]` local, `soiree/` et ses jeux |

`soiree help` les rappelle.

Le code de sortie dit si le **périmètre demandé** a passé. Un passage rapide ou
un scénario isolé ne certifient pas le reste du jeu. Les rapports distinguent
méthode, résultat, hypothèses et provenance ; une intention écrite n’est pas
un test passé. Les durées et les gains dans la boucle d’itération restent à
mesurer. Le format et la migration sont dans [`VERIFICATION.md`](VERIFICATION.md).

## Où trouver le reste

| Pour | Lis |
|---|---|
| les idées, le niveau d'exigence, les principes d'ingénierie | [`MANIFESTE.md`](MANIFESTE.md) |
| ce que chaque jeu tient le soir même | [`CERTIFICATION.md`](CERTIFICATION.md) |
| les contrats exécutables, les rapports et leurs limites | [`VERIFICATION.md`](VERIFICATION.md) |
| brancher un jeu sur la TV et les téléphones | [`salon/README.md`](salon/README.md) |
| l'app iPhone qui sert de manette | [`remote/CONTRAT.md`](remote/CONTRAT.md) |
| les jeux de la soirée | [`catalogue.toml`](catalogue.toml) |
| ce qui change d'une version à l'autre | [`CHANGELOG.md`](CHANGELOG.md) |
| modifier le commun | [`AGENTS.md`](AGENTS.md) |

Une question, un besoin dans salon, une règle qui gêne : ouvre une issue sur ce dépôt.
