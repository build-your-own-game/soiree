# Le dossier de l'hôte

> Ce fichier s'adresse à l'agent de code qui travaille dans ce dossier, quel qu'il soit. Il vit hors de tout dépôt, chez l'hôte seulement.

## Ici, plusieurs dépôts

Ce dossier n'est pas un dépôt git. Il en contient plusieurs, chacun avec son historique, son amont sur GitHub et ses propres règles :

```
./
├── AGENTS.md              ce fichier, dans aucun dépôt
├── .cargo/config.toml     le [patch] local, dans aucun dépôt
├── fnox.toml              le certificat du HTTPS, chiffré, dans aucun dépôt
├── soiree/                https://github.com/build-your-own-game/soiree : la console
└── games/
    ├── punchline/         https://github.com/build-your-own-game/punchline
    ├── sidequest/         https://github.com/build-your-own-game/sidequest
    └── tier-list/         https://github.com/build-your-own-game/tier-list
```

Avant de toucher un fichier, sache dans quel dépôt il est. Puis lis l'`AGENTS.md` de ce dépôt : c'est lui qui fait foi pour ce qui s'y trouve.
- `soiree/AGENTS.md` : pour modifier la console (les règles, le contrat, l'outil, les briques, le jeu de départ).
- `games/<jeu>/AGENTS.md` : pour modifier un jeu.

Le manifeste de la console est `soiree/MANIFESTE.md`, et le contrat que chaque jeu remplit est `soiree/CONTRAT.md`. Ne les modifie pas sans demande explicite de l'hôte.

## Committer et pousser

- Un commit appartient à un seul dépôt. Un changement qui touche `soiree/` et un jeu fait au moins deux commits, un dans chaque dépôt.
- Lance `git` depuis le dossier du dépôt (`git -C games/sidequest …`), jamais depuis ici : ici, il n'y a pas de dépôt.
- Avant de déclarer fini, et avant de rendre la main : `git -C <dépôt> status -sb` dans chaque dépôt, pour dire ce qui n'est pas commité ou pas poussé.
- Pousse chaque dépôt séparément. Ne pousse rien sans l'accord de l'hôte s'il ne te l'a pas déjà donné.

## L'outil `soiree`

Ici, l'outil s'installe depuis le clone de la console :

```sh
cargo install --locked --path soiree/outil
```

Réinstalle-le après chaque changement dans `soiree/regles/` ou `soiree/outil/` : `soiree check` applique les règles du binaire installé.

`soiree games`, lancé ici, récupère dans `games/` tous les jeux de l'organisation et dit lesquels passent la vérification. `soiree help` liste le reste.

## L'étagère, le soir de la soirée

`soiree play`, lancé ici, ouvre l'étagère dans le navigateur : la jaquette de chaque jeu, et un clic pour le lancer.

- **Au démarrage**, elle récupère les jeux, vérifie chacun et range de quoi le lancer sans rien compiler (`games/.etagere/`). La première fois prend plusieurs minutes par jeu ; ensuite, un jeu qui n'a pas changé est prêt tout de suite. Lance-la avant que les invités arrivent.
- **Pendant la soirée**, rien ne bouge : ce qu'un ami pousse sur GitHub ne change pas le jeu rangé.
- **Mettre un jeu à jour** : la touche M sur l'étagère, ou `soiree maj <jeu>` dans un autre terminal. Le dossier `games/<jeu>` est pris tel qu'il est s'il a des changements locaux, sinon mis à jour depuis GitHub. L'ancienne version reste jouable, et n'est remplacée que si la nouvelle passe la vérification.
- **Dans un jeu**, Échap maintenu ouvre le menu de la console (un appui court reste au jeu) : reprendre, recommencer, quitter vers l'étagère. Le jeu est en pause pour tout le monde tant qu'il est ouvert.
- **Les téléphones** scannent le QR code de chaque jeu : on ne joue qu'à ce qu'on a scanné. Un téléphone resté sur la page du jeu précédent lit le nom du nouveau jeu et qu'il faut scanner son QR code. Le QR code reste dans le menu de la console (Échap maintenu), pour un retardataire. « Recommencer » garde les joueurs ; relancer un jeu depuis l'étagère redemande de scanner. Ils passent tous par l'étagère, qui les amène au jeu en cours : au premier lancement, macOS demande une fois si `soiree` peut recevoir des connexions. Réponds oui, sinon aucun téléphone n'arrive.

Lance-la depuis un terminal ouvert dans ce dossier : c'est lui qui donne aux jeux le certificat du HTTPS (voir plus bas).

## La console et les jeux, côte à côte

Un jeu emprunte les deux briques de la console par étiquette git, ici comme chez les amis :

```toml
cartouche = { git = "https://github.com/build-your-own-game/soiree", tag = "vX.Y.Z" }
telecommande = { git = "https://github.com/build-your-own-game/soiree", tag = "vX.Y.Z" }
```

Pour changer la console et un jeu en même temps, décommente le `[patch]` de `.cargo/config.toml`. Cargo lit ce fichier pour tout projet placé sous ce dossier et remplace alors les briques par les dossiers `soiree/cartouche` et `soiree/telecommande` :
- Un changement dans une brique se voit tout de suite dans les jeux, sans étiquette ni push.
- Le jeu compile ici avec une console que les amis n'ont pas. Chez eux, c'est l'étiquette qui compte.
- L'étiquette nommée dans le `Cargo.toml` du jeu doit exister sur GitHub : Cargo la récupère avant d'appliquer le `[patch]`.

Remets le `[patch]` en commentaire avant de vérifier et de committer un jeu : son `Cargo.lock` doit nommer l'étiquette, pas le dossier local.

## Un changement qui touche la console et un jeu

Dans l'ordre :

1. **Développe ensemble.** Décommente le `[patch]`, modifie `soiree/` et le jeu en même temps.
2. **Vérifie la console.** Suis `soiree/AGENTS.md`, puis `soiree check` dans chaque jeu.
3. **Étiquette la console.** Committe dans `soiree/`, étiquette la nouvelle version, pousse le commit **et** l'étiquette, avec l'accord de l'hôte.
4. **Passe le jeu à l'étiquette.** Remets le `[patch]` en commentaire. Dans le `Cargo.toml` du jeu, écris la nouvelle étiquette. Puis `soiree rules` et `soiree check`.
5. **Pousse le jeu.** Seulement maintenant.

**Ne pousse jamais un jeu qui dépend d'une console non publiée.**

## Le HTTPS sur le Wi-Fi

Safari ne donne les capteurs de mouvement d'un téléphone qu'à une page servie en HTTPS. L'app iPhone de la console s'en passe ; un téléphone qui joue dans son navigateur en a besoin.

Ici, **le HTTPS est le réglage par défaut** : `cargo run --release`, dans le dossier d'un jeu, lance le jeu en HTTPS. Le QR code mène à un nom en HTTPS ; le même port répond toujours en clair, pour l'écran (`http://localhost`) et pour l'app.

Trois pièces le font tenir :

- **`fnox.toml`**, ici, porte quatre variables chiffrées que la brique « télécommande » lit : `SOIREE_TLS_NOM` (le nom), `SOIREE_TLS_CERT` et `SOIREE_TLS_KEY` (son certificat et sa clé), `SOIREE_TLS_SUIVRE` (la commande qui fait suivre le nom).
- **Le terminal les charge tout seul** : `eval "$(fnox activate zsh)"` dans `~/.zshrc` les donne à tout ce qu'on lance sous ce dossier, et les retire quand on en sort. Hors d'un terminal (un outil qui lance le jeu lui-même), écris `fnox exec -- cargo run --release`.
- **Le nom suit l'ordinateur.** Son adresse change avec le réseau (la maison, un partage de connexion). Au lancement, si le nom ne mène pas ici, la brique lance la commande de `SOIREE_TLS_SUIVRE` et le dit ; les téléphones voient la nouvelle adresse dans la minute. Cette commande n'a que des identifiants limités à ce nom : elle ne peut rien changer d'autre.

Le nom, son enregistrement dynamique et le certificat viennent du dépôt infra (`tofu/soiree.tf`).

- **Avant une soirée**, renouvelle et recopie le certificat : il dure 90 jours. `infra tofu apply` le renouvelle quand il lui reste moins de 30 jours ; puis, depuis ce dossier :

  ```sh
  infra tofu output -raw soiree_https_cert | fnox set SOIREE_TLS_CERT
  infra tofu output -raw soiree_https_cle | fnox set SOIREE_TLS_KEY
  ```

- **Un jeu en clair, exprès** : `env -u SOIREE_TLS_NOM -u SOIREE_TLS_CERT -u SOIREE_TLS_KEY cargo run --release`.
- **Un téléphone n'arrive pas** alors que le jeu dit que le nom mène ici : attends une minute (le temps que l'ancienne adresse s'oublie), puis soupçonne la box : certaines refusent un nom public qui mène à une adresse privée. L'app iPhone, elle, passe par l'adresse en clair.

## Les téléphones par internet

D'ordinaire, les téléphones doivent être sur le Wi-Fi de l'ordinateur. Avec le relais de la console, déployé sur la prod (`soiree/relais/`, `infra deploy soiree`), ils passent par internet : la 4G suffit.

- **L'allumer** : la touche I, sur l'étagère. La ligne du bas dit où ça en est : éteint, allumé, ou allumé mais le relais ne répond pas. L'étagère s'en souvient : allumé un soir, il l'est encore au prochain `soiree play`, et le terminal le dit.
- **Il vaut pour tous les jeux, au prochain lancement.** Un jeu déjà lancé garde l'adresse qu'il a montrée : relance-le pour qu'il en change, et les téléphones rescannent son QR code. Quand le jeu en cours et l'interrupteur ne sont plus d'accord, la ligne du bas le dit et demande de le relancer.
- **Éteins-le avant un jeu en temps réel** (Jumpy, Tennis) : chaque geste fait l'aller-retour par le serveur.
- **Si le relais ne répond pas** au moment de lancer un jeu, les téléphones restent sur le Wi-Fi, et l'étagère le dit.
- Il n'y a pas de code de soirée : pendant que c'est allumé, qui connaît l'adresse peut prendre une place.

**Quand un téléphone rame**, `soiree reseau`, lancé ici pendant qu'un jeu tourne sur l'étagère, mesure ce qu'un téléphone voit de ce jeu : le délai d'un aller-retour, et si le lien tient sous une 4G faible ou une coupure. `soiree reseau --help` dit le reste. Il lui faut Toxiproxy (`brew install toxiproxy`). Son téléphone d'essai compte comme une page de plus : lance-le hors d'une vraie partie. C'est un outil à la demande : rien ne l'exige, ni pour lancer une soirée ni pour changer un jeu.

`fnox.toml`, ici, porte deux variables de plus : `SOIREE_INTERNET` (l'adresse du relais, celle que les téléphones ouvrent) et `SOIREE_INTERNET_CLE` (la clé que le relais attend, la même que `RELAIS_CLE` dans `PROD_SOIREE_ENV`, côté infra). Sans elles, l'étagère ne propose rien.

## Les secrets d'un jeu

Un jeu qui interroge internet (une IA qui juge) n'a jamais de clé dans son code : sa machine écrit `{NOM}` dans un en-tête, et la console y met la clé. **La clé locale d'abord, le guichet sinon.**

- **La clé locale.** L'hôte la confie au jeu par deux variables, dans le `fnox.toml` du dossier du jeu :

  ```sh
  fnox set SOIREE_SECRET_NOM          # la clé
  fnox set SOIREE_SECRET_NOM_POUR     # la seule adresse où elle a le droit d'aller : api.exemple.com
  ```

- **Le guichet.** Sans clé locale, un jeu lancé par l'étagère lui envoie la question telle quelle, `{NOM}` compris. L'étagère la fait suivre au relais (il faut `SOIREE_INTERNET` et `SOIREE_INTERNET_CLE`, que l'interrupteur « internet » soit allumé ou non), et c'est le relais qui y met la clé. Les clés restent donc sur le serveur : un ami qui reçoit chez lui n'en a besoin d'aucune. Côté serveur, elles sont dans `PROD_SOIREE_ENV` (infra), deux variables par clé :

  ```sh
  RELAIS_SECRET_NOM=…                 # la clé
  RELAIS_SECRET_NOM_POUR=api.exemple.com
  ```

  Le relais ne prête une clé qu'à l'adresse qui va avec, en HTTPS, et seulement à une étagère qui présente sa clé à lui.

Quand une clé manque ici et au guichet, le jeu continue sans réponse, et la console dit dans le terminal (le journal du jeu, `partie.log`, sous l'étagère) quelles variables donner.

## Ce qui n'est dans aucun dépôt

`AGENTS.md` et `.cargo/config.toml`, ici, ne sont sauvegardés nulle part. Leurs modèles sont dans `soiree/dossier/`. Pour les changer pour de bon, change le modèle, puis recopie-le ici.

`fnox.toml`, ici, porte le certificat du HTTPS : il n'a pas de modèle, il se refait depuis le dépôt infra (voir « Le HTTPS sur le Wi-Fi »).
