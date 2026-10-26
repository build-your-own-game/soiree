# Brancher un jeu sur la Télécommande

L'app iPhone de `remote/` sert de télécommande à tous les jeux. Le joueur n'y choisit rien : **ton jeu dit quelle télécommande afficher**, et il en change quand il veut (un buzzer pendant le quiz, une toile pendant le dessin). Garde ta page téléphone pour les joueurs qui n'ont pas l'app.

## Rejoindre

- Ton hôte sert un WebSocket au chemin `/remote` (`remote_protocol::PATH`), sur la même adresse et le même port que ta page téléphone.
- Le joueur scanne le QR de la TV depuis l'app. Pour une adresse `http://hôte:port/chemin?requête`, l'app se connecte à `ws://hôte:port/remote?requête`. La requête (code de salle…) passe telle quelle : ton QR actuel suffit.
- Scanné avec l'appareil photo de l'iPhone, le QR ouvre ta page web. Elle peut proposer un lien `telecommande://join?url=<adresse de la page, encodée>`, qui ouvre l'app sur ton jeu.

## Messages

Des frames WebSocket binaires, un message par frame, encodés par le crate `remote-protocol`. Ajoute-le avec `remote-protocol = { path = "../../remote/crates/protocol" }` ; il fixe le format, et ses tests vérifient l'aller-retour.

1. Le téléphone envoie `PhoneMessage::Hello { version, player, name }`.
   - `player` est un jeton de 16 octets, le même à chaque reconnexion : c'est l'identité du joueur, sans compte.
   - Si `version` diffère de `remote_protocol::VERSION`, réponds `GameMessage::Refuse { reason }` : l'app affiche la raison.
2. Ton jeu répond `GameMessage::Show(Remote)`, la télécommande à afficher. Renvoie-le pour en changer.
3. Le téléphone envoie `PhoneMessage::Input(InputState)` à chaque changement (au plus 60 fois par seconde), et toutes les 250 ms sinon. C'est un état complet : garde le dernier, ignore un `seq` plus petit que le précédent.
4. Les traits d'une toile arrivent dans l'ordre, en `PhoneMessage::Draw(DrawEvent)`. Le WebSocket les livre tous.
5. `GameMessage::Feedback(Feedback)` fait vibrer le téléphone (`Haptic::Success`, `Warning`…) et peut teinter son fond, par exemple « premier au buzzer ».
6. Si la connexion tombe, l'app se reconnecte seule et renvoie le même `Hello`.

## Décrire la télécommande : trois niveaux

1. **Un preset**, prêt à l'emploi : `Remote::Pad(presets::buzzer())`.
   - Il y en a sept dans `remote_protocol::presets` :

     | Preset | Contenu |
     |---|---|
     | `pad` | stick, A, B |
     | `cross` | croix, A, B, Select, Start |
     | `wheel` | volant par inclinaison, gaz, frein |
     | `pointer` | viser la TV |
     | `buzzer` | tout l'écran est le bouton |
     | `draw` | toile et outils |
     | `n64` | la manette N64 complète |

   - Chaque module (`presets::pad`, `presets::n64`…) donne les identifiants de ses contrôles.
   - `.label(id, "Sauter")`, `.color(id, 0xRRGGBB)`, `.with(contrôle)` et `.without(id)` les adaptent.
2. **Une composition** : `Remote::Pad(Layout { controls, tilt, pointer, canvas })`.
   - Tu places toi-même tes contrôles :
     - des boutons (rond, pilule, rectangle arrondi, ou tout l'écran) ;
     - des sticks (un par moitié d'écran) ;
     - des croix ;
     - des actions que l'app gère seule : couleur, épaisseur, annuler, effacer, recentrer.
   - Chacun se place à un décalage en points depuis l'un des neuf repères de l'écran (coins, milieux des bords, centre).
   - L'app applique ses règles de toucher : zones élargies, glissement d'un bouton à l'autre, stick sous le pouce, vibrations. Tu choisis quoi et où, elle s'occupe de la sensation.
   - Les presets sont des compositions comme les autres : lis `crates/protocol/src/presets.rs` pour t'en inspirer.
3. **Libre** : `Remote::Web { url }`.
   - L'app affiche ta page en plein écran (une `url` relative part de ton hôte), et ta page parle à ton jeu comme elle veut.
   - L'app lui ajoute seulement `window.telecommande` :
     - `vibrate("press" | "release" | "success" | "warning")` ;
     - `motion(rappel)`, qui donne la gravité et la rotation du téléphone environ 60 fois par seconde.
   - Rien d'autre n'est imposé.

## Lire un état

```rust
use remote_protocol::{GameMessage, PhoneMessage, Remote, decode, encode, presets};

// Un téléphone vient de dire Hello sur /remote :
send(encode(&GameMessage::Show(Remote::Pad(presets::pad().label(presets::pad::A, "Sauter")))));

// Chaque frame reçue de ce téléphone :
if let Ok(PhoneMessage::Input(input)) = decode::<PhoneMessage>(&frame) {
    let jump = input.is_pressed(presets::pad::A);
    let (x, y) = input.stick(presets::pad::STICK); // de -1 à 1, y vers le haut
}
```

Les autres lectures :
- **`input.tilt()`** donne le volant, de -1 à 1.
- **`input.pointer()`** donne le curseur sur la TV, de 0 à 1 en largeur et en hauteur.
- **Une croix** occupe quatre identifiants : haut, bas, gauche, droite, à partir du sien.

## Essayer sans jeu

Depuis `remote/`, lance le faux jeu :

```sh
cargo run -p remote-demo
```

Il affiche son QR, imprime les entrées reçues et passe d'une télécommande à l'autre au clavier. Son code (`crates/demo/src/main.rs`) est aussi un exemple complet de serveur `/remote`.
