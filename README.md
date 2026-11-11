# La console de nos soirées

Des amis font chacun un jeu avec un agent de code, puis on y joue tous ensemble un soir, chez l'hôte. Son ordinateur est branché sur la télé ; les téléphones servent de télécommandes.

Ce dépôt est la console : les règles que tous les jeux suivent, l'outil qui les vérifie, les briques qu'elle prête et un jeu de départ. Chaque jeu vit dans son propre dépôt. Le pourquoi est dans [`MANIFESTE.md`](MANIFESTE.md).

## Commencer un jeu

1. **Installe Rust** avec rustup : https://rustup.rs. Ferme puis rouvre le terminal.
2. **Installe l'outil `soiree`**, à la version de la soirée :
   ```sh
   cargo install --locked --git https://github.com/build-your-own-game/soiree --tag v0.4.0 soiree
   ```
3. **Crée ton jeu**, dans le dossier où tu ranges tes projets :
   ```sh
   soiree new mon-jeu
   cd mon-jeu
   ```
4. **Lance-le** :
   ```sh
   cargo run --release
   ```
   Le terminal affiche deux adresses : celle de l'écran, à ouvrir sur l'ordinateur, et celle des téléphones.
5. **Donne `AGENTS.md` à ton agent de code** et dis-lui quel jeu tu veux.

## Les commandes

| Commande | Ce qu'elle fait |
|---|---|
| `cargo run --release` | lance le jeu |
| `soiree check` | vérifie que le jeu suit les règles et se branche ; un jeu qui ne passe pas n'est pas joué |
| `soiree rules` | écrit dans le jeu les règles de la version installée, après une mise à jour de la console |
| `soiree new <nom>` | crée un jeu depuis le jeu de départ |
| `soiree manifeste` | affiche le manifeste |

## Ce que contient ce dépôt

| Dossier | Contenu |
|---|---|
| `MANIFESTE.md` | le but, les règles, comment la console évolue |
| `regles/` | les règles vérifiées : version de Rust, linteur, réglages de Clippy |
| `outil/` | l'outil `soiree` |
| `telecommande/` | la brique « télécommande » : le téléphone trouve le jeu par un QR code et lui parle |
| `modele/` | le jeu de départ que copie `soiree new` |
| `dossier/` | les fichiers du dossier de l'hôte, qui range la console et ses jeux côte à côte |
