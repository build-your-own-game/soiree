# Changements

Chaque version a sa section, de la plus récente à la plus ancienne. « Passer à cette version » s'adresse à l'agent d'un jeu (voir `AGENTS.md`).

## v0.1.0 — pas encore étiquetée

La première version : salon, la télécommande iPhone, les règles communes, le harnais de certification, le catalogue d'items par mécanique, l'outil `soiree` et le modèle de jeu.

### Passer à cette version

1. Installe l'outil : `cargo install --locked --git https://github.com/build-your-own-game/soiree --tag v0.1.0 soiree`.
2. Un jeu neuf : `soiree new <nom>`. Un jeu existant : ajoute `soiree.toml` (`version = "0.1.0"`), remplace les chemins vers salon par `{ git = "https://github.com/build-your-own-game/soiree", tag = "v0.1.0" }`, puis `soiree check`.
