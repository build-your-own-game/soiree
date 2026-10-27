# Salon

La couche commune qui relie l'hôte (l'ordinateur branché à la TV) à ses appareils : la page TV, les téléphones (navigateur ou app iPhone), les manettes et le clavier. Un jeu ne voit jamais un socket, un jeton ni une reconnexion. Il voit des **appareils** qui arrivent, partent, reviennent, envoient des **événements** et tiennent des **états**.

La spécification est la documentation de `salon-core` (`crates/core/src/lib.rs`) : les quatre idées, ce que salon garantit (item de certification par item), ce qu'il ne fait pas. Les leçons des références (str0m, renet, quinn, Colyseus, Nakama, boardgame.io, AirConsole, Jackbox, HappyFunTimes, geckos.io, Gaffer on Games, Quake 3) sont dans `LEARNINGS.md`.

| Crate | Rôle |
|---|---|
| `crates/core` | le protocole, sans entrées-sorties, `no_std` : `Host` et `Client` |
| `crates/host` | le serveur commun : pages, WebRTC (un seul port UDP), WebSocket, clé de la TV, porte de l'app iPhone, salle sur un seul fil |
| `crates/crypto` | la cryptographie de WebRTC (DTLS, certificat) en Rust seul, pour str0m |
| `crates/web` | la page : `Connection`, WebRTC puis WebSocket, jeton, minuteries, réveil |
| `crates/demo`, `crates/demo-page` | un mini-jeu pour tout essayer de bout en bout : l'hôte et ses messages, puis sa page |

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

### La version

Salon refuse une page construite depuis d'autres sources que l'hôte (`Refusal::Version`) : une page restée dans le cache d'un navigateur après un changement est refusée, au lieu de lire de travers des messages qui ont changé. Le numéro se calcule depuis les sources ; personne n'a à le monter à la main.

1. Copie `version.rs`, posé à côté de ce README (dans un jeu, `soiree check` les écrit tous les deux), comme `build.rs` du crate qui définit `Up` et `Down` (en général le cœur du jeu). Cargo lance tout seul un `build.rs` posé à côté de `Cargo.toml` ; sinon, donne son chemin dans `[package]`, avec `build = "…"`.
2. Dans ce crate :

   ```rust
   /// La version du jeu : salon refuse une page construite depuis d'autres sources.
   pub const VERSION: u32 = include!(concat!(env!("OUT_DIR"), "/version.rs"));
   ```

3. L'hôte passe `VERSION` à `salon_host::Options::new`, la page à `salon_web::Options::for_page`. Les deux compilent ce même crate : ils ont le même numéro exactement quand ils viennent des mêmes sources.

Chaque fichier du `src` de ce crate compte, commentaires et tests compris : un changement qui ne touche pas aux messages refuse aussi les anciennes pages, ce qui coûte un rechargement. Un autre numéro du jeu (version d'un journal, d'un replay) reste le sien : seul celui passé à salon se calcule ainsi.

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
- **Les manettes** : `host.add_local(now, Kind::Gamepad, …)`, puis `local_state`, `local_unplugged` et `local_plugged`.
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

### Les noms de la page

La page ne lit pas le DOM par chaîne (principe 8 du manifeste) : les règles de Clippy interdisent les méthodes à chaînes de `web_sys` (`get_element_by_id`, `query_selector`, `create_element`, `class_list`, `set_attribute`, `set_inner_html`, `dataset`, `style().set_property`…). `soiree dom` lit le HTML et le CSS de la page, et écrit un module Rust où chaque nom qu'ils déclarent est une constante typée :

```sh
soiree dom crates/web/src/page.rs --html ../../../web/index.html --css ../../../web/style.css
```

- Déclare le module à la racine du crate : `pub mod page;`.
- `page::id::X` (`.get()`, `.is(&el)`, `.give(&el)`), `page::class::X` (`.add_to`, `.remove_from`, `.is_on`, `.set(&el, bool)`, `.closest`), `page::data::X` pour les `data-*` (`.set(&el, &valeur)`, `.get::<T: FromStr>(&el)`), `page::attr::X`, `page::prop::X` (propriétés CSS, variables `--x` comprises), `page::tag::X.create()`, `page::document()`.
- La recette est en tête du fichier généré : `// html:`, `// css:`, puis ce que **seul le Rust** crée, ajouté par les options de la commande : `--id <id> <balise>`, `--data <nom>`, `--attr <nom>`, `--tag <balise>`. Les chemins sont relatifs au fichier généré.
- Après un changement de la page : `soiree dom` suivi du chemin du module, qui relit sa recette. Ne le modifie jamais à la main : `soiree check` le régénère et échoue s'il diffère.
- Le CSS est lu dans ses feuilles (`--css`), pas dans les `<style>` du HTML. Une classe n'existe que si le HTML ou le CSS la nomme.
- Du HTML en chaîne devient des éléments construits avec `page::tag::…`. Un SVG se construit avec `create_element_ns`, ses attributs déclarés par `--attr`. La valeur d'un `data-*` qui aiguille (`data-action="undo"`) est une enum avec `Display` et `FromStr`, lue par `page::data::ACTION.get::<Action>(&cible)`.
- Les noms d'événements (`"click"`) et de touches (`"Enter"`) restent des chaînes : c'est le vocabulaire du navigateur.

`crates/demo-page` en est un exemple complet, QR code en SVG compris.

### Les chemins de salon

Salon sert ses propres chemins à côté des pages du jeu : l'offre WebRTC, la clé de la TV, le WebSocket. Ils sont nommés une fois, dans `salon_core::route` (`OFFER`, `TV_KEY`, `SOCKET`) : un jeu qui en a besoin, pour ne pas les masquer par une page ou dans un test, les nomme par là, jamais en littéral.

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

## Journaux et diagnostics

`salon_core::journal` définit les enveloppes typées `Stored<H, E>` et
`Line<H, E>` : en-tête, entrée appliquée, marqueur, checkpoint et fin. Le jeu
choisit ses types d'en-tête et d'entrée. Il enregistre **au point où son cœur
applique l'entrée**, après fixation de l'ordre, avec son temps, sa graine,
ses réglages et son contenu initial. Une vue réseau ne suffit pas pour un
replay : l'empreinte `salon_core::journal::fingerprint(&état)` doit porter
sur l'état complet, générateur aléatoire compris.

Sur l'hôte, `salon_host::journal::Recorder::start(dossier, préfixe, en_tête)`
crée un fichier unique ; `start_file(chemin, en_tête)` conserve un nom choisi
par un jeu sans jamais écraser un fichier. `record(entrée)`, puis
`checkpoint(empreinte)`, écrivent les lots au fil de l'eau sur un thread.
`finish(empreinte)` marque une fin naturelle ; il n'est pas nécessaire pour
rejouer le dernier checkpoint d'un hôte tué. `Journal::<H, E>::read(chemin)`
refuse une ligne complète abîmée ou un numéro de séquence manquant et ne rend
que les entrées couvertes par le dernier checkpoint. Une dernière ligne
physiquement tronquée peut être abandonnée si un checkpoint antérieur a été
vérifié ; `truncated_tail` le signale explicitement. Le jeu recalcule l'état et compare l'empreinte.

La file du writer est bornée : une saturation ou une erreur arrête cet
enregistrement et renvoie une erreur au jeu, qui continue à tourner mais
ne doit jamais annoncer un replay complet. `failed()` permet de constater
une erreur disque asynchrone. `flush()` attend le disque : réservé aux
commandes, aux tests et aux threads HTTP, jamais à la simulation. À
l'ouverture d'un journal, seuls les fichiers `.jsonl` ordinaires du dossier
de journaux qui ont plus de **30 jours** et commencent par une enveloppe
typée Header de séquence zéro sont supprimés ; les fichiers étrangers restent intacts. Ce dossier reste
ignoré par Git ; les replays de test versionnés ne contiennent pas de
textes de joueurs.

Un jeu solo dont l'état vit dans la page peut utiliser
`salon_web::journal::Recorder::<H, E>::start(chemin_http, en_tête)`, avec les
mêmes méthodes, et brancher sur `Game::http` le
`salon_host::journal::browser_sink::<H, E>(dossier, chemin_http)`. Les
requêtes sont ordonnées et bornées : au plus 4 Mio sérialisés en attente,
transportés par fragments typés de 8 Kio sous la limite HTTP de 64 Kio. Le sink
valide les séquences et remplace une session abandonnée lors du rechargement
du même onglet. Les sessions et fragments restent valides pendant une
suspension ; leur vie se termine explicitement par remplacement ou Final,
avec des limites de 128 sessions et huit assemblages de 4 Mio.
Il valide les types avant de les
transmettre au writer et n'accepte que la page locale de l'hôte.
`error()` rend visible une erreur de livraison. L'état du jeu reste dans
son navigateur.

`Connection::open` installe le filet anti-panique commun. Une page solo
appelle `salon_web::install_panic_hook()` avant de démarrer (le recorder le
fait aussi). Le hook affiche « Petit souci, on recharge », envoie un
`Diagnostic::Panic` à l'hôte et programme le rechargement avec un callback
natif du navigateur, qui n'a plus besoin d'exécuter le WASM paniqué. Le
jeton salon retrouve la place. Après trois tentatives rapprochées, ou si
le stockage de session ne permet pas de compter les tentatives, un message
fixe remplace le rechargement.

Sur la TV, **F8** marque un bug. Le hook de diagnostic est installé par une
connexion `Kind::Tv` et par le recorder solo ; une autre page TV peut
appeler `salon_web::install_tv_diagnostics()`. L'hôte conserve les
diagnostics typés dans le sous-dossier de journaux de `Options::data` (ou du dossier local data) et
les remet dans l'ordre de la salle à `Game::diagnostic(now, &diagnostic)`,
pour que le jeu ajoute le marqueur à son journal actif. Le scénario de
certification ouvre explicitement `?salon-test-panic=1`, puis émet
`salon:test-panic` sur `document` : cela déclenche une vraie panique WASM,
pas un simple rechargement.

## Simuler l'hôte entier

`salon_host::driver::Driver<G>` est la boucle exécutée par la vraie salle :
les entrées réseau d'abord, les ticks dus ensuite, puis les événements
livrés au jeu. `Driver::new(now, config, tick_ms, game)` et
`step(now, inputs)` n'accèdent ni à une horloge ni à un socket. Les
`Input::{Opened, Received, Closed, Diagnostic}` produisent des
`Effect::{Transmit, Close}` ; `game()`, `game_mut()` et `host()` permettent
d'inspecter les invariants. Une horloge qui recule n'altère pas l'état.

`salon_host::simulation::Network::new(seed, faults, driver)` ajoute les
vrais `Client` salon, sur un faux réseau à graine. `add_phone(config, caps)`
ajoute un appareil, `run` et `run_until` avancent le temps virtuel,
`run_checked` vérifie les invariants après chaque événement ou échéance utile ; `cut`,
`freeze` et `wake` éprouvent les reconnexions. `Faults` règle pertes,
duplications et gigue. Les tests d'un jeu fournissent sa vraie salle comme
`Game`, envoient ses vrais `Up` par les clients et comparent son état complet.
