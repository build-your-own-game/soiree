# Le dossier de l'hôte

> Ce fichier s'adresse à l'agent de code qui travaille dans ce dossier, quel qu'il soit. Il vit hors de tout dépôt, chez l'hôte seulement.

## Ici, plusieurs dépôts

Ce dossier n'est pas un dépôt git. Il en contient plusieurs, chacun avec son historique, son amont sur GitHub et ses propres règles :

```
./
├── AGENTS.md              ce fichier, dans aucun dépôt
├── .cargo/config.toml     le [patch] local, dans aucun dépôt
├── soiree/                https://github.com/build-your-own-game/soiree : le commun
└── games/
    ├── punchline/         https://github.com/build-your-own-game/punchline
    ├── sidequest/         https://github.com/build-your-own-game/sidequest
    └── tier-list/         https://github.com/build-your-own-game/tier-list
```

Avant de toucher un fichier, sache dans quel dépôt il est. Puis lis l'`AGENTS.md` de ce dépôt : c'est lui qui fait foi pour ce qui s'y trouve.
- `soiree/AGENTS.md` : pour modifier le commun (salon, la télécommande, les règles, le harnais, le modèle).
- `games/<jeu>/AGENTS.md` : pour modifier un jeu.

## Committer et pousser

- Un commit appartient à un seul dépôt. Un changement qui touche `soiree/` et un jeu fait au moins deux commits, un dans chaque dépôt.
- Lance `git` depuis le dossier du dépôt (`git -C games/sidequest …`), jamais depuis ici : ici, il n'y a pas de dépôt.
- `soiree status` dit, pour chaque dépôt du dossier, ce qui n'est pas commité, pas poussé, ou n'a pas d'amont. Lance-le avant de déclarer fini, et avant de rendre la main.
- Pousse chaque dépôt séparément. Ne pousse rien sans l'accord de l'hôte s'il ne te l'a pas déjà donné.

## Le `[patch]` local

Chaque jeu dépend de salon par étiquette git :

```toml
salon-core = { git = "https://github.com/build-your-own-game/soiree", tag = "v0.1.0" }
```

Ici, `.cargo/config.toml` remplace ces dépendances par les dossiers de `soiree/`. Cargo lit ce fichier pour tout projet placé sous ce dossier. Conséquences :
- Un changement dans `soiree/salon` se voit tout de suite dans les trois jeux, sans étiquette ni push.
- Le jeu compile ici avec un salon que les amis n'ont pas. Chez eux, c'est l'étiquette qui compte.
- `soiree check` le voit (Cargo dit d'où vient chaque crate) et prévient : « salon-core vient d'un chemin local ».
- L'étiquette nommée dans le `Cargo.toml` du jeu doit quand même exister sur GitHub : Cargo la récupère avant d'appliquer le `[patch]`.
- Cargo affiche `patch … was not used in the crate graph` pour chaque crate qu'un jeu n'utilise pas. C'est normal.

Pour voir le jeu comme le voient les amis, sans le `[patch]` : clone-le hors de ce dossier et lance `soiree check` dans le clone.

## Un changement qui touche salon et un jeu

C'est le cas courant : un jeu a besoin de quelque chose que salon n'a pas encore. Dans l'ordre :

1. **Développe ensemble.** Modifie `soiree/salon` et le jeu en même temps. Le `[patch]` fait le lien. Le `Cargo.toml` du jeu garde l'ancienne étiquette.
2. **Vérifie le commun.** Suis `soiree/AGENTS.md` : `soiree check` et `soiree certify` sur le modèle et sur les trois jeux, `CHANGELOG.md` à jour.
3. **Étiquette salon.** Committe dans `soiree/`, étiquette la nouvelle version (`v0.2.0`), pousse le commit **et** l'étiquette (`git push --follow-tags`).
4. **Passe le jeu à l'étiquette.** Dans le jeu : `soiree upgrade`, ou à la main la nouvelle étiquette dans `Cargo.toml` et la version dans `soiree.toml`. Puis `soiree check`.
5. **Pousse le jeu.** Seulement maintenant.

**Ne pousse jamais un jeu qui dépend d'un salon non publié.** Chez un ami, ou sur la machine de la soirée, il ne compilerait pas, ou compilerait contre l'ancien salon. Si `soiree check` prévient d'un chemin local, demande-toi si le jeu a besoin de ce qui n'est pas encore étiqueté. Si oui, reviens à l'étape 3.

Les autres jeux n'ont rien à faire : ils restent sur leur étiquette et passent à la suivante quand on le décide.

## Ce qui n'est dans aucun dépôt

`AGENTS.md` et `.cargo/config.toml`, ici, ne sont sauvegardés nulle part. Leurs modèles sont dans `soiree/outil/dossier/`, embarqués dans l'outil.

`soiree host-setup` installe ce dossier, sur cette machine ou sur une neuve :
- il écrit ces deux fichiers depuis les modèles ; s'ils existent déjà et diffèrent, il dit ce qui diffère et ne les remplace qu'avec `--force` ;
- il clone `soiree/` s'il manque, puis, d'après `soiree/catalogue.toml`, les jeux de l'hôte dans `games/` (`--author <pseudo>`, ou `--all` pour tous) ;
- il ne touche jamais un dossier qui existe déjà, et prévient s'il n'a pas le bon amont.

`soiree host-setup --dry-run` montre tout sans rien faire. Pour changer ces fichiers pour de bon, change leur modèle dans `soiree/outil/dossier/`, puis relance `soiree host-setup --force`.
