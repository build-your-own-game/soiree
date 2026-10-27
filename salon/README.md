# Salon

La couche commune qui relie l'hôte (l'ordinateur branché à la TV) à ses appareils : la page TV, les téléphones (navigateur ou app iPhone), les manettes et le clavier. Un jeu ne voit jamais un socket, un jeton ni une reconnexion. Il voit des **appareils** qui arrivent, partent, reviennent, envoient des **événements** et tiennent des **états**.

La spécification est la documentation de `salon-core` (`crates/core/src/lib.rs`) : les quatre idées, ce que salon garantit (item de certification par item), ce qu'il ne fait pas. Les leçons des références (str0m, renet, quinn, Colyseus, Nakama, boardgame.io, AirConsole, Jackbox, HappyFunTimes, geckos.io, Gaffer on Games, Quake 3) sont dans `LEARNINGS.md`.

| Crate | Rôle |
|---|---|
| `crates/core` | le protocole, sans entrées-sorties, `no_std` : `Host` et `Client` |
| `crates/host` | le serveur commun : pages, WebRTC (un seul port UDP), WebSocket, clé de la TV, porte de l'app iPhone, salle sur un seul fil |
| `crates/crypto` | la cryptographie de WebRTC (DTLS, certificat) en Rust seul, pour str0m |
| `crates/web` | la page : `Connection`, WebRTC puis WebSocket, jeton, minuteries, réveil |
| `crates/demo`, `crates/demo-page` | un mini-jeu pour tout essayer de bout en bout |

## Brancher un jeu

### Les messages

Deux types, partagés par l'hôte et les pages (dans le cœur du jeu, sans dépendance au navigateur) :

```rust
#[derive(Serialize, Deserialize)]
pub enum Up { Join { name: String }, Answer { prompt: u32, text: String }, Pad(PadState) }   // appareils → hôte
#[derive(Serialize, Deserialize)]
pub enum Down { Phone(PhoneView), Tv(TvView), Buzz }                                       // hôte → appareils
```

Un **événement** (`send`) compte une fois : une réponse, un vote. Un **état** (`show`, `set_state`) est le dernier qui compte : ce qu'un écran doit afficher, les sticks d'une manette.

### L'hôte

```rust
struct Partie { /* l'état du jeu, indexé par DeviceId */ }

impl salon_host::Game for Partie {
    type Up = Up;
    type Down = Down;

    fn on(&mut self, now: Ms, event: DeviceEvent<Up>, host: &mut Host<Up, Down>) {
        match event {
            DeviceEvent::Arrived { device, kind: Kind::Tv, .. } => { /* host.show(device, &Down::Tv(…)) */ }
            DeviceEvent::Arrived { device, .. } => { /* un téléphone : lui montrer l'écran d'accueil */ }
            DeviceEvent::Left { device, why } => { /* absent : pause, tour sauté, place gardée… au jeu de choisir */ }
            DeviceEvent::Back { device } => { /* revenu : salon renvoie déjà son dernier état */ }
            DeviceEvent::Event { device, event, phase, at } => { /* refuser si `phase` est périmée */ }
            DeviceEvent::State { device, state } => { /* la dernière entrée tenue */ }
        }
    }

    fn tick(&mut self, now: Ms, host: &mut Host<Up, Down>) { /* échéances, bots, simulation */ }
}

fn main() {
    let mut options = salon_host::Options::new(port, dist, VERSION, "Mon jeu ready");
    options.pages.push(("/play".into(), "index.html".into()));
    options.data = Some(data_dir);             // les appareils reviennent eux-mêmes après un redémarrage
    options.app_page = Some("/play".into());   // l'app iPhone ouvre cette page
    salon_host::run(options, Partie::new()).unwrap_or_else(|error| eprintln!("{error}"));
}
```

Règles :

- **Tout est indexé par `DeviceId`.** Un téléphone qui revient garde le sien ; après un redémarrage de l'hôte (avec `options.data`), aussi, et il arrive par `Arrived` puisque le nouveau processus ne le connaissait pas.
- **Un `Arrived` appelle un `show`** : sans état, l'écran du téléphone reste vide.
- **`host.set_phase(n)`** à chaque changement de manche ou de question : chaque événement porte la phase vue par l'appareil, et le jeu refuse ce qui est périmé.
- **Les manettes** : `host.add_local(now, Kind::Gamepad, …)`, puis `local_state` et `local_present`.
- **Les requêtes HTTP du jeu** (télécharger des résultats) : `Game::http`. Les actions de l'organisateur passent par la TV, en événements : seule la TV a la clé, et seule une connexion de l'ordinateur lui-même peut être la TV.

### La page

```rust
let connection: Connection<Up, Down> = Connection::open(
    Options::for_page(VERSION, if tv { Kind::Tv } else { Kind::Phone }),
    move |happening| match happening {
        Happening::State(view) => { /* dessiner */ }
        Happening::Event(event) => { /* un son, une vibration */ }
        Happening::Status(status) => { /* « Reconnexion… », « Ouvert ailleurs », un refus */ }
        Happening::Reset => { /* l'hôte a redémarré : oublier l'écran, attendre le nouveau */ }
    },
);
connection.send(&Up::Join { name })?;
```

- La connexion vit autant que la page ; on peut la cloner.
- `Status::Replaced` : un autre onglet a pris la place. Proposer « Jouer ici » (`connection.resume()`).
- `?seat=2` donne une autre identité dans le même navigateur, pour essayer plusieurs téléphones ; `?salon=ws` force le WebSocket. La TV passe toujours par WebSocket : elle est sur l'ordinateur même.
- `connection.host_time()` donne l'heure de l'hôte, pour des comptes à rebours en phase avec la TV.

## Construire

Rust seul (la version du manifeste) : aucun compilateur C. str0m prend sa cryptographie de `crates/crypto`, en Rust du début à la fin ; ses propres fournisseurs compilent tous du C quelque part.

## Essayer

```bash
cargo run --release -p salon-demo
```

La TV est sur http://localhost:8095, les téléphones sur l'adresse affichée. `SALON_LOG=1` affiche chaque lien ouvert ou fermé, avec son transport.

Le pare-feu de macOS demande d'autoriser chaque nouveau binaire (chaque `cargo build` en est un). Sans autorisation, l'UDP de WebRTC qui vient du réseau est bloqué en silence et les téléphones passent par WebSocket, au bout de 3 s. Une page ouverte sur l'ordinateur même passe par la boucle locale, que le pare-feu laisse passer.

## Tester

```bash
cargo test
```

`crates/core/tests/` joue chaque garantie sur un faux réseau à graine, qui perd, retarde, double et réordonne les paquets, coupe des liens et gèle des téléphones. `PROPTEST_CASES=5000 cargo test` cherche plus loin.
