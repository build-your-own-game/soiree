# Validation du commun publié — 27 octobre 2026

Le jeu utilise l'étiquette publiée `v0.1.0`, commit
`d868b761aeaacad1c523c4ce1d9f0e4ffd396c62`. `soiree.toml` demande la même
version et les consignes utilisent les documents générés dans `.soiree/`.

Une copie des fichiers du jeu a été vérifiée hors du dossier de l'hôte,
sans son `[patch]` local. Les lockfiles existants ont été résolus avec
`cargo metadata`, sans changer les versions déjà verrouillées :

```sh
cargo metadata --format-version 1
cargo metadata --manifest-path fuzz/Cargo.toml --format-version 1
soiree check --locked
```

Ces commandes réussissent. La vérification complète passe le formatage,
Clippy natif et WASM et les tests : **0 erreur, 0 avertissement**.
Les deux lockfiles résolus sont recopiés dans le dépôt.

Les cinq crates `remote-protocol`, `salon-core`, `salon-crypto`, `salon-host`
et `salon-web` viennent toutes de la source exacte suivante, et non d'un chemin
sur l'ordinateur de l'hôte :

```text
git+https://github.com/build-your-own-game/soiree?tag=v0.1.0#d868b761aeaacad1c523c4ce1d9f0e4ffd396c62
```

Le lockfile séparé du fuzz utilise également cette source pour `salon-core`
et ajoute les dépendances nécessaires à son empreinte SHA-256.

La première vérification a trouvé une référence documentaire à un dossier
runtime absent d'un clone neuf. Le README précise désormais que data/journals/
est créé au premier lancement ; aucun dossier factice n'a été ajouté à la copie.

Lors du balayage du dépôt Git, Cargo affiche des diagnostics sur les noms
`{{nom}}` des manifests modèles non substitués, puis résout correctement les
crates demandées. La vérification verrouillée complète réussit.

Les scénarios navigateur ont été mesurés avant cet alignement de métadonnées.
Leurs rapports et empreintes restent conservés sans modification.
