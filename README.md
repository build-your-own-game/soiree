# La console de nos soirées

Des amis font chacun un jeu avec un agent de code, puis on y joue tous ensemble un soir, chez l'hôte. Son ordinateur est branché sur la télé ; les téléphones servent de télécommandes.

Ce dépôt est la console : les règles que tous les jeux suivent, l'outil qui les vérifie, les briques qu'elle prête et un jeu de départ. Chaque jeu vit dans son propre dépôt, dans l'organisation `build-your-own-game`. Le pourquoi est dans [`MANIFESTE.md`](MANIFESTE.md).

## Commencer un jeu

**Avec un assistant IA** (le plus simple, même sans avoir jamais codé) : copie le texte de [`REJOINDRE.md`](REJOINDRE.md) et colle-le dans ton assistant. Il te guide jusqu'au bout.

**À la main** : installe Rust (https://rustup.rs), puis l'outil et un jeu :

```sh
cargo install --locked --git https://github.com/build-your-own-game/soiree --tag v0.4.1 soiree
soiree new mon-jeu
```

La suite est dans l'outil : `soiree help`.

## Ce que contient ce dépôt

| Dossier | Contenu |
|---|---|
| `MANIFESTE.md` | le but, les règles, comment la console évolue |
| `REJOINDRE.md` | le texte qu'un ami colle dans son assistant IA pour commencer |
| `regles/` | les règles vérifiées : version de Rust, linteur, réglages de Clippy |
| `outil/` | l'outil `soiree` |
| `telecommande/` | la brique « télécommande » : le téléphone trouve le jeu par un QR code et lui parle |
| `modele/` | le jeu de départ que copie `soiree new` |
| `dossier/` | les fichiers du dossier de l'hôte, qui range la console et ses jeux côte à côte |
