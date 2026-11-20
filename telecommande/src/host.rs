//! Le côté ordinateur : servir les pages et parler avec elles. Un détail de
//! la brique : [`crate::console`] s'en sert, les jeux n'y touchent pas.
//!
//! `Joined` arrive à chaque connexion, y compris quand une page revient après
//! une coupure : la console lui renvoie alors ce qu'elle doit afficher.

use std::collections::BTreeMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, TcpListener, ToSocketAddrs as _, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::connect_info::Connected;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, DefaultBodyLimit, Path as Named, Query, State};
use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::serve::IncomingStream;
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use qrcode::QrCode;
use qrcode::render::svg;
use serde::Deserialize;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio::sync::watch;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;

use cartouche::Answer;

use crate::store::{CONTENT_MAX, Refused, Store};
use crate::tls::{self, Identity};
use crate::{REPLACED, Role};

/// Le port quand la console n'en donne pas (chez toi, pendant que tu codes).
const DEFAULT_PORT: u16 = 8080;
/// Le jeu envoie un signe de vie à chaque page à ce rythme.
const HEARTBEAT: Duration = Duration::from_secs(3);
/// Une page muette aussi longtemps est partie : téléphone en veille, Wi-Fi perdu.
const SILENCE: Duration = Duration::from_secs(10);
/// Ce qu'un téléphone voit quand le jeu se joue sans télécommande.
const NO_REMOTES: &str = "<!doctype html>\n<html lang=\"fr\">\n<meta charset=\"utf-8\">\n\
    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
    <title>Soirée</title>\n\
    <body style=\"margin:0;min-height:100vh;display:grid;place-items:center;\
    font:1.4rem system-ui,sans-serif;background:#111;color:#eee;text-align:center\">\n\
    <p style=\"margin:2rem\">Ce jeu se joue sur le grand écran,<br>sans télécommande.</p>\n\
    </body>\n</html>\n";

/// Une page, reconnue d'une connexion à l'autre : un téléphone qui se met en
/// veille puis revient garde le même `Id`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Id(pub u64);

/// Ce qui arrive au jeu. Le texte d'une page est laissé tel quel : la console
/// le garde pour le journal avant de le lire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    /// Une page vient de se connecter, pour la première fois ou de nouveau.
    Joined(Id, Role),
    /// Une page a envoyé un texte.
    Text(Id, String),
    /// Une page n'est plus là.
    Left(Id),
    /// Le monde extérieur a répondu à une demande de la machine : le numéro
    /// que la console a donné à cette demande, et la réponse.
    Answer(u64, Answer),
    /// L'hôte a choisi quelque chose dans le menu de la console.
    Order(Order),
}

/// Ce que l'hôte demande à la console, depuis son menu. Seule une page
/// ouverte sur l'ordinateur de l'hôte peut le demander.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Order {
    /// Ouvre le menu : le jeu se fige pour tout le monde.
    Pause,
    /// Ferme le menu : le jeu reprend.
    Resume,
    /// Le jeu demande la pause : le temps s'arrête, sans le menu.
    Hold,
    /// Le jeu rend le temps.
    Release,
    /// Une partie neuve, sans relancer le jeu.
    Restart,
    /// Ferme le jeu et rend la main à l'étagère.
    Quit,
}

impl Order {
    fn read(text: &str) -> Option<Self> {
        match text {
            "pause" => Some(Self::Pause),
            "reprendre" => Some(Self::Resume),
            "tenir" => Some(Self::Hold),
            "lacher" => Some(Self::Release),
            "recommencer" => Some(Self::Restart),
            "quitter" => Some(Self::Quit),
            _ => None,
        }
    }
}

/// D'où vient une connexion : le menu de la console ne s'ouvre que depuis
/// l'ordinateur de l'hôte.
#[derive(Clone, Copy, Debug)]
struct Caller(tls::Origin);

impl Connected<IncomingStream<'_, tls::Listener>> for Caller {
    fn connect_info(stream: IncomingStream<'_, tls::Listener>) -> Self {
        Self(*stream.remote_addr())
    }
}

struct Peer {
    connection: u64,
    outbox: UnboundedSender<String>,
}

struct Shared {
    peers: Mutex<BTreeMap<Id, Peer>>,
    connections: AtomicU64,
    events: mpsc::Sender<Event>,
    /// Le QR code de l'adresse des téléphones. `None` : le jeu se joue sans
    /// télécommande, et aucune n'entre.
    qr: Option<String>,
    store: Arc<Store>,
    /// Ce que le menu de la console doit montrer, écrit par la console :
    /// chaque page le reçoit en arrivant, puis à chaque changement.
    console: watch::Sender<String>,
}

/// Ce qu'il faut pour ouvrir le jeu aux pages.
pub(crate) struct Opening<'a> {
    /// Le dossier des pages, à servir.
    pub(crate) pages: &'a Path,
    /// Le dossier où ranger ce que les pages confient.
    pub(crate) contents: PathBuf,
    /// Le chemin de la télécommande, par exemple `"/t"`.
    pub(crate) remote_path: &'a str,
    /// Faux : le jeu se joue sans télécommande.
    pub(crate) remotes: bool,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Le jeu, ouvert aux pages.
pub(crate) struct Room {
    shared: Arc<Shared>,
    events: mpsc::Receiver<Event>,
    remote_url: String,
}

impl Room {
    /// Sert le dossier `pages` et attend les connexions.
    ///
    /// Le port est celui que donne la console (la variable `PORT`). L'adresse
    /// que les téléphones ouvrent est celle de la variable `SOIREE_ADRESSE` si
    /// l'hôte passe par un relais, sinon celle de cet ordinateur sur le Wi-Fi.
    ///
    /// Quand l'hôte donne un nom et son certificat (`SOIREE_TLS_NOM`,
    /// `SOIREE_TLS_CERT`, `SOIREE_TLS_KEY`), cette adresse est en HTTPS, sur le
    /// même port : Safari ne donne les capteurs de mouvement d'un téléphone
    /// qu'à une page servie en HTTPS. L'écran reste à `http://localhost`. Si le
    /// nom ne mène pas à cet ordinateur, la commande de `SOIREE_TLS_SUIVRE`
    /// le met à jour (voir [`follow`]).
    ///
    /// Un chemin qui n'est pas un fichier du dossier reçoit `index.html` : une
    /// seule page peut donc être l'écran à `/` et la télécommande à `/t`.
    ///
    /// Un jeu sans télécommande n'a pas d'adresse pour les téléphones : celui
    /// qui ouvre quand même le chemin de la télécommande lit que le jeu se
    /// joue sur l'écran, et un lien de télécommande est refusé.
    ///
    /// Les pages confient ce qui est lourd à `/contenu` et le redemandent à
    /// `/contenu/<nom>` (voir [`crate::store`]).
    pub(crate) fn open(opening: Opening<'_>) -> io::Result<Self> {
        let Opening {
            pages,
            contents,
            remote_path,
            remotes,
        } = opening;
        let port = port()?;
        let relay = Relay::from_env()?;
        let (listener, door) = listen(port, relay.as_ref())?;
        let identity = Identity::from_env()?;
        let public = relay.as_ref().map_or(port, |relay| relay.public);
        let base = std::env::var("SOIREE_ADRESSE").map_or_else(
            |_| match &identity {
                Some(identity) => format!("https://{}:{public}", identity.name),
                None => format!("http://{}:{public}", lan_address()),
            },
            |address| address.trim_end_matches('/').to_owned(),
        );
        if let Some(identity) = &identity
            && remotes
        {
            follow(&identity.name, public);
        }
        let remote_url = if remotes {
            format!("{base}{remote_path}")
        } else {
            String::new()
        };
        let screen_url = format!("http://localhost:{port}/");
        let (sender, events) = mpsc::channel();
        let shared = Arc::new(Shared {
            peers: Mutex::new(BTreeMap::new()),
            connections: AtomicU64::new(0),
            events: sender,
            qr: if remotes {
                Some(qr_svg(&remote_url)?)
            } else {
                None
            },
            store: Arc::new(Store::open(contents)),
            console: watch::Sender::new(String::from("{}")),
        });
        let no_cache =
            SetResponseHeaderLayer::overriding(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        let files = ServeDir::new(pages).fallback(ServeFile::new(pages.join("index.html")));
        let mut app = Router::new()
            .route("/ws", get(upgrade))
            .route("/qr.svg", get(qr))
            .route("/soiree/console", get(console_upgrade));
        if !remotes {
            app = app.route(remote_path, get(|| async { Html(NO_REMOTES) }));
        }
        // Un contenu ne change jamais sous son nom : lui seul se garde en cache.
        let app = app
            .fallback_service(files)
            .layer(no_cache)
            .route(
                "/contenu",
                post(upload).layer(DefaultBodyLimit::max(CONTENT_MAX)),
            )
            .route("/contenu/{name}", get(download))
            .with_state(Arc::clone(&shared));
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let _: std::thread::JoinHandle<()> = std::thread::spawn(move || {
            let served = runtime.block_on(async move {
                let listener = tokio::net::TcpListener::from_std(listener)?;
                let door = door.map(tokio::net::TcpListener::from_std).transpose()?;
                let listener = tls::Listener::new(listener, door, identity.as_ref())?;
                axum::serve(
                    listener,
                    app.into_make_service_with_connect_info::<Caller>(),
                )
                .await
            });
            if let Err(error) = served {
                eprintln!("le serveur s'est arrêté : {error}");
            }
        });
        println!("Écran       : {screen_url}");
        if remotes {
            println!("Téléphones  : {remote_url}");
            // Deux onglets d'un même navigateur sont le même joueur : `?j=` en
            // fait des joueurs différents, pour essayer le jeu seul sur cet
            // ordinateur.
            let local = format!("http://localhost:{port}{remote_path}");
            println!("Pour essayer à plusieurs sur cet ordinateur, un onglet par joueur :");
            println!("              {local}?j=1   {local}?j=2   …");
        } else {
            println!("Téléphones  : aucun, ce jeu se joue sur l'écran");
        }
        Ok(Self {
            shared,
            events,
            remote_url,
        })
    }

    /// Le prochain événement, ou `None` si rien n'arrive pendant `wait`.
    pub(crate) fn next(&self, wait: Duration) -> Option<Event> {
        let deadline = Instant::now().checked_add(wait)?;
        let left = deadline.saturating_duration_since(Instant::now());
        self.events.recv_timeout(left).ok()
    }

    /// Envoie un texte déjà écrit à une page. Sans effet si elle n'est pas
    /// connectée : elle recevra ce qu'il faut à son prochain `Joined`.
    pub(crate) fn send(&self, to: Id, text: String) {
        if let Some(peer) = lock(&self.shared.peers).get(&to) {
            let _: Result<(), SendError<String>> = peer.outbox.send(text);
        }
    }

    /// Dit aux pages ce que le menu de la console doit montrer : un objet
    /// JSON déjà écrit (voir `menu.rs`).
    pub(crate) fn announce(&self, state: String) {
        let _: String = self.shared.console.send_replace(state);
    }

    /// Par où une réponse du monde extérieur arrive au jeu : elle sort de
    /// [`Room::next`] comme ce qui vient des pages, à son tour.
    pub(crate) fn answers(&self) -> mpsc::Sender<Event> {
        self.shared.events.clone()
    }

    /// Les contenus que les pages ont confiés.
    pub(crate) fn store(&self) -> Arc<Store> {
        Arc::clone(&self.shared.store)
    }

    /// L'adresse que les téléphones ouvrent. Le QR code de cette adresse est
    /// servi à `/qr.svg` : `<img src="/qr.svg">` suffit à l'afficher. Vide
    /// quand le jeu se joue sans télécommande.
    pub(crate) fn remote_url(&self) -> &str {
        &self.remote_url
    }
}

/// L'étagère reçoit les téléphones à la place du jeu (`SOIREE_RELAIS`, deux
/// ports séparés par `:`), toujours à la même adresse d'un jeu à l'autre.
struct Relay {
    /// Le port que les téléphones ouvrent, sur l'étagère.
    public: u16,
    /// Le port, sur cet ordinateur seulement, où elle les amène au jeu.
    door: u16,
}

/// Ouvre le port du jeu, et la porte des téléphones s'il y a un relais.
/// Lancé par l'étagère, le jeu n'écoute que cet ordinateur : c'est elle qui
/// reçoit les téléphones et les amène à leur porte.
fn listen(port: u16, relay: Option<&Relay>) -> io::Result<(TcpListener, Option<TcpListener>)> {
    let (listener, door) = match relay {
        Some(relay) => (
            TcpListener::bind((Ipv4Addr::LOCALHOST, port))?,
            Some(TcpListener::bind((Ipv4Addr::LOCALHOST, relay.door))?),
        ),
        None => (TcpListener::bind(("0.0.0.0", port))?, None),
    };
    listener.set_nonblocking(true)?;
    if let Some(door) = &door {
        door.set_nonblocking(true)?;
    }
    Ok((listener, door))
}

impl Relay {
    fn from_env() -> io::Result<Option<Self>> {
        let Ok(text) = std::env::var("SOIREE_RELAIS") else {
            return Ok(None);
        };
        text.split_once(':')
            .and_then(|(public, door)| {
                Some(Self {
                    public: public.parse().ok()?,
                    door: door.parse().ok()?,
                })
            })
            .map(Some)
            .ok_or_else(|| {
                io::Error::other(format!("SOIREE_RELAIS n'est pas `port:port` : {text}"))
            })
    }
}

fn port() -> io::Result<u16> {
    match std::env::var("PORT") {
        Ok(text) => text
            .parse()
            .map_err(|_| io::Error::other(format!("PORT n'est pas un port : {text}"))),
        Err(_) => Ok(DEFAULT_PORT),
    }
}

/// L'adresse de cet ordinateur sur le réseau local. Aucun paquet n'est
/// envoyé : on demande seulement au système par où il sortirait.
///
/// D'abord vers une adresse du réseau local lui-même (celle par laquelle les
/// appareils s'y annoncent) : un VPN détourne ce qui part vers internet, pas
/// ça. Demander par où l'on sort vers internet donnerait, sous VPN, l'adresse
/// du tunnel, où aucun téléphone n'arrive.
fn lan_address() -> IpAddr {
    let toward = |target: (&str, u16)| {
        UdpSocket::bind(("0.0.0.0", 0))
            .and_then(|socket| {
                socket.connect(target)?;
                socket.local_addr()
            })
            .ok()
            .map(|address| address.ip())
            .filter(|address| !address.is_loopback() && !address.is_unspecified())
    };
    toward(("224.0.0.251", 9))
        .or_else(|| toward(("192.0.2.1", 9)))
        .unwrap_or(IpAddr::from([127, 0, 0, 1]))
}

/// En HTTPS, les téléphones arrivent par un nom : s'il ne mène pas à cet
/// ordinateur sur le Wi-Fi, ils n'arriveront nulle part.
///
/// L'adresse de l'ordinateur change avec le réseau. Si l'hôte a donné de quoi
/// faire suivre le nom (`SOIREE_TLS_SUIVRE` : une commande, où `{adresse}`
/// est remplacé par l'adresse de cet ordinateur), on la lance, sans retarder
/// le jeu. Sinon on le dit tout de suite, plutôt que de laisser chercher
/// pendant la soirée.
fn follow(name: &str, port: u16) {
    let here = lan_address();
    let found: Vec<IpAddr> = (name, port)
        .to_socket_addrs()
        .map(|addresses| addresses.map(|address| address.ip()).collect())
        .unwrap_or_default();
    if found.contains(&here) {
        return;
    }
    let there = if found.is_empty() {
        "nulle part".to_owned()
    } else {
        found
            .iter()
            .map(IpAddr::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let stuck = format!(
        "Attention : {name} mène à {there}, et cet ordinateur est à {here} sur le Wi-Fi. \
         Les téléphones n'arriveront pas tant que le DNS de {name} ne donne pas {here}."
    );
    let Some(command) = std::env::var("SOIREE_TLS_SUIVRE")
        .ok()
        .filter(|command| !command.trim().is_empty())
    else {
        eprintln!("{stuck}");
        return;
    };
    // La commande porte les identifiants de l'hôte : on ne l'affiche jamais.
    let command = command.replace("{adresse}", &here.to_string());
    let name = name.to_owned();
    let _: std::thread::JoinHandle<()> = std::thread::spawn(move || {
        let (shell, flag) = if cfg!(windows) {
            ("cmd", "/C")
        } else {
            ("sh", "-c")
        };
        let ran = std::process::Command::new(shell)
            .args([flag, &command])
            .output();
        match ran {
            Ok(output) if output.status.success() => println!(
                "{name} menait à {there} : il mène maintenant à {here}. Les téléphones le \
                 verront d'ici une minute."
            ),
            Ok(output) => eprintln!(
                "{stuck}\nLa mise à jour du nom a échoué : {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
            Err(error) => {
                eprintln!("{stuck}\nLa mise à jour du nom n'a pas pu se lancer : {error}");
            }
        }
    });
}

fn qr_svg(url: &str) -> io::Result<String> {
    let code = QrCode::new(url.as_bytes()).map_err(io::Error::other)?;
    Ok(code
        .render::<svg::Color<'_>>()
        .min_dimensions(240, 240)
        .build())
}

async fn qr(State(shared): State<Arc<Shared>>) -> Response {
    let Some(qr) = shared.qr.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    (
        StatusCode::OK,
        [(CONTENT_TYPE, HeaderValue::from_static("image/svg+xml"))],
        qr,
    )
        .into_response()
}

/// Une page confie un contenu : ses octets, et son type dans l'en-tête. La
/// réponse est son ticket.
async fn upload(State(shared): State<Arc<Shared>>, headers: HeaderMap, body: Bytes) -> Response {
    let declared = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    match shared.store.put(&body, declared) {
        Ok(content) => Json(content).into_response(),
        Err(Refused::Empty) => (StatusCode::BAD_REQUEST, "Ce contenu est vide.").into_response(),
        Err(Refused::TooBig) => {
            (StatusCode::PAYLOAD_TOO_LARGE, "Ce contenu est trop lourd.").into_response()
        }
        Err(Refused::Disk(error)) => {
            eprintln!("un contenu n'a pas pu être rangé : {error}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Le contenu n'a pas pu être rangé.",
            )
                .into_response()
        }
    }
}

/// Une page redemande un contenu par le nom de son ticket. Il est servi
/// comme une donnée inerte : ouvert tel quel, il ne peut rien lancer.
async fn download(State(shared): State<Arc<Shared>>, Named(name): Named<String>) -> Response {
    let Some((bytes, content_type)) = shared.store.get(&name) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = HeaderValue::from_str(&content_type)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, content_type),
            (
                CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            ),
            (X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
            (
                CONTENT_SECURITY_POLICY,
                HeaderValue::from_static("sandbox; default-src 'none'"),
            ),
        ],
        bytes,
    )
        .into_response()
}

async fn console_upgrade(
    upgrade: WebSocketUpgrade,
    ConnectInfo(Caller(address)): ConnectInfo<Caller>,
    State(shared): State<Arc<Shared>>,
) -> Response {
    // Le menu ne s'ouvre que depuis l'ordinateur de l'hôte : un téléphone
    // sur le Wi-Fi reçoit ce qu'il doit montrer, et ne demande rien.
    let seat = if !address.relayed && address.address.ip().is_loopback() {
        Seat::Host
    } else {
        Seat::Guest
    };
    upgrade.on_upgrade(move |socket| console_link(socket, seat, shared))
}

/// D'où une page regarde le jeu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Seat {
    /// Sur l'ordinateur de l'hôte : elle peut ouvrir le menu.
    Host,
    /// Ailleurs sur le réseau : un téléphone.
    Guest,
}

/// Le lien du menu de la console avec une page, du début à la fin.
async fn console_link(socket: WebSocket, seat: Seat, shared: Arc<Shared>) {
    let local = seat == Seat::Host;
    let mut state = shared.console.subscribe();
    let (mut sink, mut stream) = socket.split();
    let first = state.borrow_and_update().clone();
    let hello = format!("{{\"ici\":{local}}}");
    if sink.send(Message::Text(hello.into())).await.is_err()
        || sink.send(Message::Text(first.into())).await.is_err()
    {
        return;
    }
    let mut beat = tokio::time::interval(HEARTBEAT);
    // Cette page tient la pause du jeu : si elle part, le temps reprend.
    let mut holding = false;
    loop {
        tokio::select! {
            changed = state.changed() => {
                if changed.is_err() {
                    break;
                }
                let text = state.borrow_and_update().clone();
                if sink.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    if let Some(order) = Order::read(text.as_str()).filter(|_| local) {
                        holding = match order {
                            Order::Hold => true,
                            Order::Release => false,
                            Order::Pause | Order::Resume | Order::Restart | Order::Quit => holding,
                        };
                        let _: Result<(), mpsc::SendError<Event>> =
                            shared.events.send(Event::Order(order));
                    }
                }
                Some(Ok(Message::Binary(_) | Message::Ping(_) | Message::Pong(_))) => {}
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
            },
            _ = beat.tick() => {
                if sink.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break;
                }
            }
        }
    }
    if holding {
        let _: Result<(), mpsc::SendError<Event>> =
            shared.events.send(Event::Order(Order::Release));
    }
}

#[derive(Deserialize)]
struct Hello {
    id: u64,
    role: Role,
}

async fn upgrade(
    upgrade: WebSocketUpgrade,
    Query(hello): Query<Hello>,
    State(shared): State<Arc<Shared>>,
) -> Response {
    // Un jeu sans télécommande : sa machine n'en voit jamais.
    if hello.role == Role::Remote && shared.qr.is_none() {
        return StatusCode::FORBIDDEN.into_response();
    }
    upgrade.on_upgrade(move |socket| serve(socket, Id(hello.id), hello.role, shared))
}

/// Une connexion, du début à la fin. Si la même page se reconnecte pendant
/// que celle-ci vit encore, la nouvelle la remplace sans que le jeu voie de
/// départ.
async fn serve(socket: WebSocket, id: Id, role: Role, shared: Arc<Shared>) {
    let connection = shared.connections.fetch_add(1, Ordering::Relaxed);
    let (outbox, mut outgoing) = unbounded_channel::<String>();
    let _: Option<Peer> = lock(&shared.peers).insert(id, Peer { connection, outbox });
    let _: Result<(), mpsc::SendError<Event>> = shared.events.send(Event::Joined(id, role));
    let (mut sink, mut stream) = socket.split();
    let mut beat = tokio::time::interval(HEARTBEAT);
    let mut heard = tokio::time::Instant::now();
    loop {
        tokio::select! {
            text = outgoing.recv() => {
                // `None` : une connexion plus récente de la même page a pris la
                // place. On le dit à celle-ci pour qu'elle ne revienne pas.
                let Some(text) = text else {
                    let replaced = CloseFrame {
                        code: REPLACED,
                        reason: "replaced".into(),
                    };
                    let _: Result<(), axum::Error> =
                        sink.send(Message::Close(Some(replaced))).await;
                    break;
                };
                if sink.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    heard = tokio::time::Instant::now();
                    let _: Result<(), mpsc::SendError<Event>> =
                        shared.events.send(Event::Text(id, text.as_str().to_owned()));
                }
                Some(Ok(Message::Binary(_) | Message::Ping(_) | Message::Pong(_))) => {
                    heard = tokio::time::Instant::now();
                }
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
            },
            _ = beat.tick() => {
                if heard.elapsed() > SILENCE
                    || sink.send(Message::Ping(Vec::new().into())).await.is_err()
                {
                    break;
                }
            }
        }
    }
    let mut peers = lock(&shared.peers);
    if peers
        .get(&id)
        .is_some_and(|peer| peer.connection == connection)
    {
        let _: Option<Peer> = peers.remove(&id);
        drop(peers);
        let _: Result<(), mpsc::SendError<Event>> = shared.events.send(Event::Left(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_orders_of_the_menu_are_read() {
        assert_eq!(Order::read("pause"), Some(Order::Pause));
        assert_eq!(Order::read("quitter"), Some(Order::Quit));
        assert_eq!(Order::read("{\"Tap\":1}"), None);
    }
}
