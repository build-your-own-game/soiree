//! Le côté ordinateur : servir les pages et parler avec elles.
//!
//! ```ignore
//! let pages = telecommande::pages::build(Path::new(env!("CARGO_MANIFEST_DIR")), "pages")?;
//! let room = Room::<Up, Down>::open(&pages, "/t")?;
//! loop {
//!     match room.next(Duration::from_millis(100)) {
//!         Some(Event::Joined { id, role }) => { /* montre-lui ce qu'il doit voir */ }
//!         Some(Event::Message { id, message }) => { /* un geste */ }
//!         Some(Event::Left { id }) => { /* parti, peut-être pour quelques secondes */ }
//!         None => { /* le temps passe */ }
//!     }
//! }
//! ```
//!
//! `Joined` arrive à chaque connexion, y compris quand une page revient après
//! une coupure : renvoie-lui alors ce qu'elle doit afficher.

use std::collections::BTreeMap;
use std::io;
use std::marker::PhantomData;
use std::net::{IpAddr, TcpListener, UdpSocket};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use qrcode::QrCode;
use qrcode::render::svg;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;

use crate::{Id, Role};

/// Le port quand la console n'en donne pas (chez toi, pendant que tu codes).
const DEFAULT_PORT: u16 = 8080;
/// Le jeu envoie un signe de vie à chaque page à ce rythme.
const HEARTBEAT: Duration = Duration::from_secs(3);
/// Une page muette aussi longtemps est partie : téléphone en veille, Wi-Fi perdu.
const SILENCE: Duration = Duration::from_secs(10);

/// Ce qui arrive au jeu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event<Up> {
    /// Une page vient de se connecter, pour la première fois ou de nouveau.
    Joined { id: Id, role: Role },
    /// Une page a envoyé un message.
    Message { id: Id, message: Up },
    /// Une page n'est plus là.
    Left { id: Id },
}

enum Raw {
    Joined(Id, Role),
    Text(Id, String),
    Left(Id),
}

struct Peer {
    connection: u64,
    outbox: UnboundedSender<String>,
}

struct Shared {
    peers: Mutex<BTreeMap<Id, Peer>>,
    connections: AtomicU64,
    events: mpsc::Sender<Raw>,
    qr: String,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Le jeu, ouvert aux pages.
pub struct Room<Up, Down> {
    shared: Arc<Shared>,
    events: mpsc::Receiver<Raw>,
    remote_url: String,
    screen_url: String,
    messages: PhantomData<fn(Down) -> Up>,
}

impl<Up: DeserializeOwned, Down: Serialize> Room<Up, Down> {
    /// Sert le dossier `pages` et attend les connexions.
    ///
    /// Le port est celui que donne la console (la variable `PORT`). L'adresse
    /// que les téléphones ouvrent est celle de la variable `SOIREE_ADRESSE` si
    /// l'hôte passe par un relais, sinon celle de cet ordinateur sur le Wi-Fi.
    /// `remote_path` est le chemin de la télécommande, par exemple `"/t"`.
    ///
    /// Un chemin qui n'est pas un fichier du dossier reçoit `index.html` : une
    /// seule page peut donc être l'écran à `/` et la télécommande à `/t`.
    pub fn open(pages: &Path, remote_path: &str) -> io::Result<Self> {
        let port = port()?;
        let listener = TcpListener::bind(("0.0.0.0", port))?;
        listener.set_nonblocking(true)?;
        let base = std::env::var("SOIREE_ADRESSE").map_or_else(
            |_| format!("http://{}:{port}", lan_address()),
            |address| address.trim_end_matches('/').to_owned(),
        );
        let remote_url = format!("{base}{remote_path}");
        let screen_url = format!("http://localhost:{port}/");
        let (sender, events) = mpsc::channel();
        let shared = Arc::new(Shared {
            peers: Mutex::new(BTreeMap::new()),
            connections: AtomicU64::new(0),
            events: sender,
            qr: qr_svg(&remote_url)?,
        });
        let no_cache =
            SetResponseHeaderLayer::overriding(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        let files = ServeDir::new(pages).fallback(ServeFile::new(pages.join("index.html")));
        let app = Router::new()
            .route("/ws", get(upgrade))
            .route("/qr.svg", get(qr))
            .fallback_service(files)
            .layer(no_cache)
            .with_state(Arc::clone(&shared));
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let _: std::thread::JoinHandle<()> = std::thread::spawn(move || {
            let served = runtime.block_on(async move {
                let listener = tokio::net::TcpListener::from_std(listener)?;
                axum::serve(listener, app).await
            });
            if let Err(error) = served {
                eprintln!("le serveur s'est arrêté : {error}");
            }
        });
        println!("Écran       : {screen_url}");
        println!("Téléphones  : {remote_url}");
        Ok(Self {
            shared,
            events,
            remote_url,
            screen_url,
            messages: PhantomData,
        })
    }

    /// Le prochain événement, ou `None` si rien n'arrive pendant `wait`.
    pub fn next(&self, wait: Duration) -> Option<Event<Up>> {
        let deadline = Instant::now().checked_add(wait)?;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.events.recv_timeout(left).ok()? {
                Raw::Joined(id, role) => return Some(Event::Joined { id, role }),
                Raw::Left(id) => return Some(Event::Left { id }),
                Raw::Text(id, text) => match serde_json::from_str(&text) {
                    Ok(message) => return Some(Event::Message { id, message }),
                    Err(error) => eprintln!("message illisible de {id:?} : {error}"),
                },
            }
        }
    }

    /// Envoie un message à une page. Sans effet si elle n'est pas connectée :
    /// elle recevra ce qu'il faut à son prochain `Joined`.
    pub fn send(&self, to: Id, message: &Down) {
        match serde_json::to_string(message) {
            Ok(text) => {
                if let Some(peer) = lock(&self.shared.peers).get(&to) {
                    let _: Result<(), SendError<String>> = peer.outbox.send(text);
                }
            }
            Err(error) => eprintln!("message impossible à écrire : {error}"),
        }
    }

    /// L'adresse que les téléphones ouvrent. Le QR code de cette adresse est
    /// servi à `/qr.svg` : `<img src="/qr.svg">` suffit à l'afficher.
    pub fn remote_url(&self) -> &str {
        &self.remote_url
    }

    /// L'adresse de l'écran, sur cet ordinateur.
    pub fn screen_url(&self) -> &str {
        &self.screen_url
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
fn lan_address() -> IpAddr {
    UdpSocket::bind(("0.0.0.0", 0))
        .and_then(|socket| {
            socket.connect(("192.0.2.1", 9))?;
            socket.local_addr()
        })
        .map_or(IpAddr::from([127, 0, 0, 1]), |address| address.ip())
}

fn qr_svg(url: &str) -> io::Result<String> {
    let code = QrCode::new(url.as_bytes()).map_err(io::Error::other)?;
    Ok(code
        .render::<svg::Color<'_>>()
        .min_dimensions(240, 240)
        .build())
}

async fn qr(State(shared): State<Arc<Shared>>) -> Response {
    (
        StatusCode::OK,
        [(CONTENT_TYPE, HeaderValue::from_static("image/svg+xml"))],
        shared.qr.clone(),
    )
        .into_response()
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
    upgrade.on_upgrade(move |socket| serve(socket, Id(hello.id), hello.role, shared))
}

/// Une connexion, du début à la fin. Si la même page se reconnecte pendant
/// que celle-ci vit encore, la nouvelle la remplace sans que le jeu voie de
/// départ.
async fn serve(socket: WebSocket, id: Id, role: Role, shared: Arc<Shared>) {
    let connection = shared.connections.fetch_add(1, Ordering::Relaxed);
    let (outbox, mut outgoing) = unbounded_channel::<String>();
    let _: Option<Peer> = lock(&shared.peers).insert(id, Peer { connection, outbox });
    let _: Result<(), mpsc::SendError<Raw>> = shared.events.send(Raw::Joined(id, role));
    let (mut sink, mut stream) = socket.split();
    let mut beat = tokio::time::interval(HEARTBEAT);
    let mut heard = tokio::time::Instant::now();
    loop {
        tokio::select! {
            text = outgoing.recv() => {
                // `None` : une connexion plus récente de la même page a pris la place.
                let Some(text) = text else { break };
                if sink.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    heard = tokio::time::Instant::now();
                    let _: Result<(), mpsc::SendError<Raw>> =
                        shared.events.send(Raw::Text(id, text.as_str().to_owned()));
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
        let _: Result<(), mpsc::SendError<Raw>> = shared.events.send(Raw::Left(id));
    }
}
