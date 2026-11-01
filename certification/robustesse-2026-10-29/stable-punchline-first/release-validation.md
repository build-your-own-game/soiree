# Validation du commun publié — 27 octobre 2026

Le jeu dépend maintenant de l’étiquette Git publiée `v0.1.0` du commun,
commit `d868b761aeaacad1c523c4ce1d9f0e4ffd396c62`. `soiree.toml` demande
la même version. Les références des agents et du README utilisent les
documents générés dans `.soiree/` ; aucun dépôt voisin n’est nécessaire.

Une copie des fichiers de travail a été préparée dans un dossier temporaire
hors du dossier de l’hôte. Elle ne contient aucun `[patch]` local.
Le cache de compilation était partagé avec les autres validations ; cela
ne remplace aucune dépendance ni sa source Cargo.

Les anciens lockfiles ont été conservés puis résolus dans cette copie :

```sh
cargo metadata --format-version 1
cargo metadata --format-version 1 --manifest-path fuzz/Cargo.toml
soiree check --locked
cargo metadata --locked --format-version 1 --manifest-path fuzz/Cargo.toml
```

Les quatre commandes réussissent avec le code de sortie **0**. La vérification
complète passe le formatage, Clippy natif et WASM et les tests : **0 erreur,
1 avertissement**. Cet avertissement conserve la limite documentée des sondes
de latence de démarrage/annulation ; il ne signale aucune dépendance locale.

La résolution n’a modifié aucune version de paquet déjà verrouillée.
Le lockfile du fuzz ajoute seulement `salon-core` et les dépendances nécessaires
à son empreinte SHA-256. Les deux lockfiles ainsi résolus sont ceux du dépôt.

Les cinq crates communes ont toutes la source exacte :

```text
git+https://github.com/build-your-own-game/soiree?tag=v0.1.0#d868b761aeaacad1c523c4ce1d9f0e4ffd396c62
```

Il s’agit de `remote-protocol`, `salon-core`, `salon-crypto`, `salon-host`
et `salon-web`. Aucune ne vient d’un chemin local. La métadonnée du fuzz
confirme également cette source pour `salon-core`, avec son lockfile figé.

Lors de la découverte du dépôt Git, Cargo affiche des diagnostics
sur les noms à substituer `{{nom}}` dans les manifests du modèle. Il poursuit
la résolution avec succès ; la vérification verrouillée complète réussit.

Cette validation porte sur les dépendances publiées, les règles et les tests.
Elle ne remesure pas les scénarios navigateur. Les rapports précédents,
leurs empreintes et le complément C9 restent conservés tels quels.
