# La soirée

Des amis font chacun un jeu avec un agent de code, puis on y joue tous ensemble un soir, chez l'hôte.
L'ordinateur de l'hôte est branché sur la TV ; les téléphones servent de manettes.
Les jeux sont écrits en Rust, tournent dans le navigateur, et tiennent tous le même plancher de qualité.

Ce dépôt est le commun : les idées, la couche réseau, la télécommande, l'outil `soiree` et le jeu de départ. Chaque jeu vit dans son propre dépôt, dans l'organisation `build-your-own-game`.

Tu ne codes pas ? Commence par [`REJOINDRE.md`](REJOINDRE.md) : donne-le à ton assistant IA, il te guidera.

Les contrats de vérification, leur portée et leurs limites sont décrits dans [`VERIFICATION.md`](VERIFICATION.md).

## Commencer un jeu

Ces étapes s'adressent à toi ou à ton agent de code. Chaque commande se lance dans un terminal.

Compte environ 7 minutes de machine jusqu'au premier `soiree check` réussi, puis 1 min 30 pour lancer le jeu, et 5 Go de disque : c'est la mesure d'un essai à froid du 2 novembre 2026 sur un Mac Apple M5 de 16 Go, hors temps humain. Un mot de passe de l'ordinateur se tape soi-même dans le terminal, jamais dans une conversation.

Sous Windows, ce parcours n'a pas encore été essayé, et `soiree certify` n'y est pas encore disponible : le harnais dépend de `lsof`, de `ps` et des groupes de processus POSIX. Parles-en à l'hôte avant d'installer quoi que ce soit.

1. **Installer Rust**, avec rustup : suis https://rustup.rs. Il faut d'abord un éditeur de liens, que rustup n'installe pas : sur Mac, lance `xcode-select --install` et valide la fenêtre qui s'ouvre ; sur Linux, installe le paquet `build-essential` ou son équivalent. Ferme ensuite puis rouvre le terminal. Un agent de code sans terminal interactif lance l'installateur avec `sh -s -- -y`, puis `. "$HOME/.cargo/env"`.
2. **Installer l'outil `soiree`**, à la version de la soirée :
   ```sh
   cargo install --locked --git https://github.com/build-your-own-game/soiree --tag v0.4.0 soiree
   ```
   La première installation prend quelques minutes.
3. **Vérifier l'ordinateur** :
   ```sh
   soiree doctor
   ```
   Il dit ce qui manque (l'éditeur de liens, la bonne version de Rust, la cible WebAssembly, wasm-bindgen, Node, Chrome, WebKit, Toxiproxy, git) et donne pour chacun le lien officiel. Il n'installe rien lui-même. Installe ce qui manque, puis relance-le jusqu'à ce qu'il ne reste aucune erreur. Les lignes marquées `–` ne bloquent pas : GitHub CLI y figure, et le WebKit du harnais se vérifie seulement après le premier `soiree certify`. Node sert à `soiree certify` ; `soiree check` s'en passe, sauf si `cert.json` déclare `ui.checks[].success`.
4. **Créer le jeu**, dans le dossier où tu ranges tes projets :
   ```sh
   soiree new mon-jeu
   cd mon-jeu
   ```
   Le nom : des minuscules, des chiffres et des tirets, comme `course-de-canards`.
5. **Vérifier le jeu, créer son dépôt dans l'organisation et pousser.**
   Le premier contrôle compile tout : c'est l'essentiel de la durée annoncée plus haut, et il n'affiche presque rien pendant ce temps. Un agent de code ne lui impose pas de délai court. Il crée aussi `Cargo.lock`, nécessaire à la CI. Il a réussi quand son résumé commence par `✓ Aucun problème trouvé` ; ce qui reste « hors de ce passage » relève de `soiree certify` et de la soirée. S'il dit `✗ À corriger` ou `⚠ Le passage n’a pas abouti`, la cause est juste en dessous, et le détail dans `certification/check.md`.
   Avec [GitHub CLI](https://cli.github.com), après avoir accepté l'invitation de l'organisation et lancé une fois `gh auth login` (toi-même : il ouvre le navigateur) :
   ```sh
   soiree check
   git init -b main
   git add -A
   git commit -m "Le jeu de départ"
   gh repo create build-your-own-game/mon-jeu --private --source . --push
   ```
   Sans `gh` : crée le dépôt `mon-jeu` sur https://github.com/organizations/build-your-own-game/repositories/new, puis suis les commandes que GitHub affiche pour « push an existing repository ». Git demandera alors un jeton GitHub, pas ton mot de passe : `gh` évite ce détour.
   Si `git commit` refuse parce qu'il ne connaît pas ton identité, renseigne `git config --global user.name` et `git config --global user.email`, puis relance-le. S'il dit l'avoir devinée, renseigne-les aussi, puis lance `git commit --amend --reset-author`.
6. **Lancer le jeu de départ**, en remplaçant `mon-jeu` :
   ```sh
   cargo run --release -p mon-jeu-host
   ```
   Le jeu est prêt à `Game ready`. Ouvre l'adresse de la ligne `TV (this computer)`, puis la même suivie de `/play?seat=1` : `Tap` fait monter le compteur de la TV. Si macOS demande d'accepter les connexions entrantes, autorise-les, sinon les téléphones n'atteindront pas le jeu. Ctrl+C arrête le jeu.
7. **Lire `AGENTS.md`**, à la racine du jeu. Si tu travailles avec un agent de code, demande-lui de le lire et de le suivre : c'est lui qui décrit la suite.

C'est fini quand le résumé de `soiree check` commence par `✓ Aucun problème trouvé` dans le dossier du jeu, que le dépôt est poussé et que le jeu de départ répond dans le navigateur.

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
| `soiree dom <fichier.rs>` | après un changement de page | réécrit le module typé des noms de la page |
| `soiree upgrade` | après une publication | passe le jeu à la version du binaire installé, après vérification de son étiquette publiée |
| `soiree status` | chez l'hôte | dans un dossier de plusieurs dépôts : ce qui n'est pas commité ou pas poussé |
| `soiree recherche plan` | avant d'ajouter du nouveau au commun | dit ce que le diff ajoute, le sous-système, le niveau, la fiche, ce qui manque et les étapes à suivre ; n'empêche rien |
| `soiree recherche verifie` | à la fin d'une fiche, avec réseau | vérifie en amont chaque commit et chaque citation des fiches ; une panne n'est pas un échec |
| `soiree recherche dette` | avant d'étiqueter le commun | liste les sous-systèmes de niveau 3 sans fiche et les trous ouverts ; informe, et ne bloque qu'à l'étiquette |
| `soiree host-setup` | chez l'hôte, une fois par machine | installe le dossier de l'hôte : `AGENTS.md`, le `[patch]` local, `soiree/` et ses jeux |

`soiree help` les rappelle.

`soiree check` et `soiree certify` finissent par le même résumé : **aucun problème
trouvé**, **à corriger** ou **le passage n’a pas abouti**. Ce qu’un humain doit
juger reste listé sans faire échouer la commande ; ce qui n’a pas pu être
vérifié la fait échouer. Le code de sortie porte sur le périmètre demandé. Un passage rapide ou
un scénario isolé ne certifient pas le reste du jeu. Les rapports distinguent
méthode, résultat, hypothèses et provenance ; une intention écrite n’est pas
un test passé. Les durées et les gains dans la boucle d’itération restent à
mesurer. Le format est dans [`VERIFICATION.md`](VERIFICATION.md).

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
