# Le dossier de l'hôte

> Ce fichier s'adresse à l'agent de code qui travaille dans ce dossier, quel qu'il soit. Il vit hors de tout dépôt, chez l'hôte seulement.

## Ici, plusieurs dépôts

Ce dossier n'est pas un dépôt git. Il en contient plusieurs, chacun avec son historique, son amont sur GitHub et ses propres règles :

```
./
├── AGENTS.md              ce fichier, dans aucun dépôt
├── .cargo/config.toml     le [patch] local, dans aucun dépôt
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

## Ce qui n'est dans aucun dépôt

`AGENTS.md` et `.cargo/config.toml`, ici, ne sont sauvegardés nulle part. Leurs modèles sont dans `soiree/dossier/`. Pour les changer pour de bon, change le modèle, puis recopie-le ici.
