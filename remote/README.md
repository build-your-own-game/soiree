# Télécommande

L'app iPhone qui sert de manette à tous les jeux de la soirée : plein écran, vibrations, plusieurs doigts à la fois, gyroscope, ce que le navigateur ne permet pas. Le joueur scanne le QR de la TV ; **c'est le jeu qui choisit la télécommande**, et il en change selon la phase de jeu.

Pour brancher un jeu : `CONTRAT.md`.

## Lancer sur le simulateur ou un iPhone

Prérequis : Xcode 27, Rust (version fixée par `rust-toolchain.toml`, les cibles iOS s'installent seules).

```sh
open ios/Remote.xcodeproj
```

Dans Xcode, choisis un simulateur ou ton iPhone, puis ⌘R. Pour un vrai iPhone, choisis ton équipe dans *Signing & Capabilities* et change l'identifiant `fr.buildyourowngame.remote` s'il est déjà pris.

Le build Xcode compile le cœur Rust et génère ses liaisons Swift avant de
compiler l'app, y compris depuis un clone neuf. `cargo xtask ios` reste
disponible pour construire séparément les bibliothèques et les liaisons des
cibles iPhone et simulateur ; cette préparation est facultative.

## Essayer avec le faux jeu

```sh
cargo run -p remote-demo
```

Il affiche un QR code et l'adresse du jeu :
- **sur un iPhone** (même Wi-Fi), scanne le QR depuis l'app ;
- **dans le simulateur**, qui n'a pas de caméra, tape l'adresse, ou ouvre le lien qu'il affiche :

  ```sh
  xcrun simctl openurl booted 'telecommande://join?url=http%3A%2F%2F127.0.0.1%3A8095%2Fplay'
  ```

Ensuite, tape une touche puis Entrée dans le terminal du faux jeu :
- **`1` à `7`** imposent l'un des sept presets ;
- **`w`** envoie une page libre en JavaScript, qui essaie tout ce que l'app ajoute aux pages ;
- **`r`** envoie une page libre écrite en Rust avec le crate `remote-web` (construis-la d'abord avec `cargo xtask page`, qui demande le CLI `wasm-bindgen 0.2.129`) ;
- **`s` et `x`** font vibrer le téléphone.

Le terminal affiche tout ce que le téléphone envoie.

## Ce qui existe

- **L'accueil :** le prénom du joueur, puis un scanner de QR code ou une adresse à taper.
- **Les liens :** `telecommande://join?url=…` ouvre l'app sur un jeu.
- **Sept presets,** toutes en paysage : deux pouces, croix rétro, volant, pointeur, buzzer, dessin, Nintendo 64.
- **Les compositions :** n'importe quels boutons, sticks, croix et outils de dessin, placés par le jeu.
- **La page libre** du jeu, en plein écran, avec tout ce qu'un navigateur ne donne pas : vibrations sur mesure, capteurs, orientation, identité du joueur, UDP, micro et caméra.
- **La croix en haut à gauche** quitte la partie, après confirmation.

En bas d'une télécommande, les octets que le jeu reçoit.

Le volant et le pointeur ne marchent qu'avec les capteurs d'un vrai iPhone ; le pointeur suit aussi le doigt.
