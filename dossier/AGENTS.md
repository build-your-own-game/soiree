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
- `soiree/AGENTS.md` : pour modifier la console (les règles, l'outil, les briques, le jeu de départ).
- `games/<jeu>/AGENTS.md` : pour modifier un jeu.

Le manifeste de la console est `soiree/MANIFESTE.md`. Ne le modifie pas sans demande explicite de l'hôte.

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

## La console et les jeux, côte à côte

Un jeu emprunte une brique de la console par étiquette git, ici comme chez les amis :

```toml
telecommande = { git = "https://github.com/build-your-own-game/soiree", tag = "vX.Y.Z" }
```

Pour changer la console et un jeu en même temps, décommente le `[patch]` de `.cargo/config.toml`. Cargo lit ce fichier pour tout projet placé sous ce dossier et remplace alors la brique par le dossier `soiree/telecommande` :
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

La brique « télécommande » répond en HTTPS quand l'hôte lui donne un nom et son certificat, par trois variables : `SOIREE_TLS_NOM`, `SOIREE_TLS_CERT`, `SOIREE_TLS_KEY`. Le même port répond toujours en clair : l'écran reste à `http://localhost`, et le QR code mène au nom en HTTPS.

Ici, ces variables sont dans `fnox.toml`, chiffrées. Pour lancer un jeu en HTTPS, depuis son dossier :

```sh
fnox exec -- cargo run --release
```

Sans `fnox exec`, le jeu se lance en clair, comme avant.

- **Le nom et le certificat viennent du dépôt infra** (`tofu/soiree.tf`) : un nom public qui mène à l'adresse de cet ordinateur sur le Wi-Fi, et un certificat Let's Encrypt, valable 90 jours.
- **L'adresse a changé** (autre Wi-Fi, autre box) : au lancement, le jeu le dit (« … mène à …, et cet ordinateur est à … »). Corrige `ip` dans `tofu/soiree.tf` du dépôt infra, puis `infra tofu apply`.
- **Avant une soirée**, renouvelle et recopie le certificat. `infra tofu apply` le renouvelle quand il lui reste moins de 30 jours ; puis, depuis ce dossier :

  ```sh
  infra tofu output -raw soiree_https_cert | fnox set SOIREE_TLS_CERT
  infra tofu output -raw soiree_https_cle | fnox set SOIREE_TLS_KEY
  ```

- **Un téléphone n'arrive pas** alors que le nom est bon : certaines box refusent un nom public qui mène à une adresse privée. L'app iPhone, elle, passe par l'adresse en clair.

## Ce qui n'est dans aucun dépôt

`AGENTS.md` et `.cargo/config.toml`, ici, ne sont sauvegardés nulle part. Leurs modèles sont dans `soiree/dossier/`. Pour les changer pour de bon, change le modèle, puis recopie-le ici.

`fnox.toml`, ici, porte le certificat du HTTPS : il n'a pas de modèle, il se refait depuis le dépôt infra (voir « Le HTTPS sur le Wi-Fi »).
