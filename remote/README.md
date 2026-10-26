# Télécommande

L'app iPhone qui sert de manette à tous les jeux de la soirée : plein écran, vibrations, plusieurs doigts à la fois, ce que le navigateur ne permet pas. Première manette : la Nintendo 64.

## Lancer sur le simulateur ou un iPhone

Prérequis : Xcode 27, Rust (version fixée par `rust-toolchain.toml`, les cibles iOS s'installent seules).

```sh
cargo xtask ios
open ios/Remote.xcodeproj
```

Dans Xcode, choisis un simulateur ou ton iPhone, puis ⌘R. Pour un vrai iPhone, choisis ton équipe dans *Signing & Capabilities* et change l'identifiant `fr.buildyourowngame.remote` s'il est déjà pris.

## Ce qui existe

- La manette N64 en paysage : stick, croix, A, B, boutons C, L, R, Z, Start, avec vibration à l'appui.
- En haut, les 4 octets qu'un jeu recevrait (le format de la vraie manette) et la position du stick.

Pas encore de connexion à un jeu : c'est la prochaine étape, avec le protocole décrit dans le manifeste.
