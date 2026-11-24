//! Le relais de la console.
//!
//! D'ordinaire, les téléphones rejoignent l'ordinateur de l'hôte par son
//! Wi-Fi. Quand l'hôte l'allume sur son étagère (`soiree play`), ils passent
//! par ce relais, sur un serveur joignable depuis internet : ils n'ont plus
//! besoin d'être sur le même réseau. Le jeu, lui, tourne toujours sur
//! l'ordinateur de l'hôte.
//!
//! Le relais ne lit rien de ce qu'il porte. Il a deux portes :
//!
//! - celle des téléphones : chaque connexion qui y arrive est tenue en
//!   attente, et son numéro est dit à l'étagère ;
//! - celle de l'étagère, qui vient d'elle-même (l'ordinateur de l'hôte n'est
//!   pas joignable depuis internet). Elle y tient une connexion ouverte, où
//!   le relais lui dit les numéros ; pour chacun, elle ouvre une connexion de
//!   plus, que le relais joint à celle du téléphone, octet pour octet.
//!
//! Sans étagère reliée, il n'y a pas de soirée : un téléphone lit une page
//! qui le dit, et rien n'est transmis. L'étagère prouve qui elle est par une
//! clé que l'hôte partage avec le relais (`RELAIS_CLE`).
//!
//! Devant le relais, un serveur web donne le HTTPS et choisit la porte : les
//! chemins sous `/_relais/` vont à celle de l'étagère, tout le reste à celle
//! des téléphones (`deploy/Caddyfile`).
//!
//! Par la porte de l'étagère passe aussi le guichet (`counter.rs`) : le
//! relais y prête à un jeu les secrets que l'hôte lui a confiés
//! (`RELAIS_SECRET_NOM`), pour qu'un ami qui reçoit chez lui n'ait pas besoin
//! des clés. Deux portes, et pas une : ce serveur
//! réemploie ses connexions d'une demande à l'autre, et une connexion jointe
//! à un téléphone ne doit jamais recevoir une demande de l'étagère.

mod counter;

use std::collections::BTreeMap;
use std::process::ExitCode;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};

/// La clé que l'étagère présente. Sans elle, le relais ne démarre pas.
const KEY_VARIABLE: &str = "RELAIS_CLE";
const KEY_MIN: usize = 24;
/// Les ports des deux portes, et les variables qui en donnent d'autres.
const PHONES_VARIABLE: &str = "RELAIS_PORT_TELEPHONES";
const PHONES_PORT: u16 = 8080;
const SHELF_VARIABLE: &str = "RELAIS_PORT_ETAGERE";
const SHELF_PORT: u16 = 8081;
/// Ce que l'étagère demande : tenir le fil, ou venir chercher un téléphone.
const WIRE_PATH: &str = "/_relais/etagere";
const DOOR_PATH: &str = "/_relais/porte/";
/// Le protocole que l'étagère et le relais se parlent, une fois la demande
/// acceptée : des octets, tels quels.
const PROTOCOL: &str = "soiree-relais";
/// L'en-tête d'une demande ne dépasse pas cette taille, et arrive vite.
const HEAD_MAX: usize = 8 * 1024;
const PATIENCE: Duration = Duration::from_secs(10);
/// Sans rien à dire, le relais envoie une ligne vide à l'étagère : un fil
/// muet finit par être coupé en chemin.
const BEAT: Duration = Duration::from_secs(15);
/// L'étagère dit quelque chose au moins à chaque battement, elle aussi : une
/// étagère muette plus longtemps que ça n'est plus là (un ordinateur en
/// veille, un réseau coupé sans prévenir).
const SILENCE: Duration = Duration::from_secs(45);

const NO_PARTY: &str = "<!doctype html><html lang=\"fr\"><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<meta http-equiv=\"refresh\" content=\"3\"><title>Soirée</title>\
<body style=\"margin:0;min-height:100vh;display:grid;place-items:center;background:#0b0820;color:#f3efff;\
font:600 1.3rem/1.4 system-ui,sans-serif;text-align:center\">\
<p style=\"padding:2rem\">Pas de soirée en cours.<br>La page réessaie toute seule.</p>";

/// Ce que les deux portes se partagent.
struct State {
    /// L'étagère reliée : son rang, pour qu'une étagère qui part ne défasse
    /// pas celle qui l'a remplacée, et de quoi lui dire un numéro.
    shelf: Option<(u64, mpsc::UnboundedSender<u64>)>,
    /// Les téléphones qui attendent que l'étagère vienne les chercher.
    waiting: BTreeMap<u64, TcpStream>,
    /// Le prochain numéro de téléphone, et le prochain rang d'étagère.
    next: u64,
    shelves: u64,
}

struct Relay {
    key: String,
    state: Mutex<State>,
    /// Le rang de l'étagère reliée, 0 quand il n'y en a pas : ce qu'une
    /// étagère portait s'arrête quand elle part.
    linked: watch::Sender<u64>,
}

impl Relay {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn main() -> ExitCode {
    match serve() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}

fn serve() -> Result<(), String> {
    let key = std::env::var(KEY_VARIABLE).unwrap_or_default();
    let key = key.trim().to_owned();
    if key.len() < KEY_MIN {
        return Err(format!(
            "Le relais attend la clé de l'hôte dans la variable {KEY_VARIABLE} : au moins {KEY_MIN} \
             caractères, les mêmes que dans SOIREE_INTERNET_CLE sur son ordinateur."
        ));
    }
    let phones = port(PHONES_VARIABLE, PHONES_PORT)?;
    let shelf = port(SHELF_VARIABLE, SHELF_PORT)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("le relais ne démarre pas : {error}"))?;
    runtime.block_on(async {
        let phones_door = listen(phones).await?;
        let shelf_door = listen(shelf).await?;
        let (linked, _) = watch::channel(0);
        let relay = Arc::new(Relay {
            key,
            state: Mutex::new(State {
                shelf: None,
                waiting: BTreeMap::new(),
                next: 1,
                shelves: 0,
            }),
            linked,
        });
        println!("relais : les téléphones au port {phones}, l'étagère au port {shelf}");
        loop {
            tokio::select! {
                arrived = phones_door.accept() => {
                    if let Ok((stream, _)) = arrived {
                        let _: tokio::task::JoinHandle<()> =
                            tokio::spawn(phone(Arc::clone(&relay), stream));
                    }
                }
                arrived = shelf_door.accept() => {
                    if let Ok((stream, _)) = arrived {
                        let _: tokio::task::JoinHandle<()> =
                            tokio::spawn(shelf_side(Arc::clone(&relay), stream));
                    }
                }
            }
        }
    })
}

fn port(variable: &str, default: u16) -> Result<u16, String> {
    match std::env::var(variable) {
        Ok(text) => text
            .trim()
            .parse()
            .map_err(|_| format!("{variable} n'est pas un port : {text}")),
        Err(_) => Ok(default),
    }
}

async fn listen(port: u16) -> Result<TcpListener, String> {
    TcpListener::bind(("0.0.0.0", port))
        .await
        .map_err(|error| format!("le port {port} est pris : {error}"))
}

/// Un téléphone arrive : il attend que l'étagère vienne le chercher.
async fn phone(relay: Arc<Relay>, stream: TcpStream) {
    let id = match relay.hold(stream) {
        Ok(id) => id,
        Err(stream) => return refuse(stream).await,
    };
    tokio::time::sleep(PATIENCE).await;
    let left = relay.state().waiting.remove(&id);
    if let Some(stream) = left {
        refuse(stream).await;
    }
}

impl Relay {
    /// Met un téléphone en attente et dit son numéro à l'étagère. Sans
    /// étagère reliée, le téléphone est rendu.
    fn hold(&self, stream: TcpStream) -> Result<u64, TcpStream> {
        let mut state = self.state();
        let Some((_, calls)) = &state.shelf else {
            return Err(stream);
        };
        let calls = calls.clone();
        let id = state.next;
        state.next = state.next.wrapping_add(1);
        let _: Option<TcpStream> = state.waiting.insert(id, stream);
        // Si l'étagère vient de partir, le téléphone sera refermé à la fin de l'attente.
        let _: Result<(), mpsc::error::SendError<u64>> = calls.send(id);
        Ok(id)
    }
}

/// Dit à un téléphone qu'il n'y a pas de soirée, et referme.
async fn refuse(mut stream: TcpStream) {
    // Sa demande d'abord : une réponse écrite avant de l'avoir lue se perd.
    let _: Option<String> = head(&mut stream, HEAD_MAX).await;
    let reply = format!(
        "HTTP/1.1 503 Service Unavailable\r\nContent-Type: text/html; charset=utf-8\r\n\
         Cache-Control: no-store\r\nRetry-After: 3\r\nConnection: close\r\n\
         Content-Length: {}\r\n\r\n{NO_PARTY}",
        NO_PARTY.len()
    );
    let _: std::io::Result<()> = stream.write_all(reply.as_bytes()).await;
    let _: std::io::Result<()> = stream.shutdown().await;
}

/// Lit l'en-tête d'une demande, jusqu'à sa ligne vide, sans rien lire de plus.
async fn head(stream: &mut TcpStream, max: usize) -> Option<String> {
    let read = async {
        let mut bytes: Vec<u8> = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            if bytes.len() >= max {
                return None;
            }
            bytes.push(stream.read_u8().await.ok()?);
        }
        String::from_utf8(bytes).ok()
    };
    tokio::time::timeout(PATIENCE, read).await.ok().flatten()
}

/// Ce que demande l'en-tête d'une demande venue par la porte de l'étagère.
#[derive(Debug, PartialEq, Eq)]
enum Ask {
    /// Tenir le fil où le relais dit les téléphones qui arrivent.
    Wire,
    /// Venir chercher le téléphone de ce numéro.
    Door(u64),
    /// Poser une question au guichet.
    Counter,
    /// La clé manque ou n'est pas la bonne.
    Stranger,
    /// Rien que le relais connaisse.
    Unknown,
}

fn ask(head: &str, key: &str) -> Ask {
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split(' ');
    let (method, path) = (first.next(), first.next());
    let presented = lines.find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("authorization")
            .then(|| value.trim().strip_prefix("Bearer "))
            .flatten()
    });
    if !presented.is_some_and(|presented| same(presented.trim(), key)) {
        return Ask::Stranger;
    }
    match (method, path) {
        (Some("GET"), Some(WIRE_PATH)) => Ask::Wire,
        (Some("POST"), Some(counter::PATH)) => Ask::Counter,
        (Some("GET"), Some(path)) => path
            .strip_prefix(DOOR_PATH)
            .and_then(|id| id.parse().ok())
            .map_or(Ask::Unknown, Ask::Door),
        (Some(_) | None, Some(_) | None) => Ask::Unknown,
    }
}

/// Deux clés égales, comparées sans s'arrêter à la première différence : le
/// temps de la réponse ne dit rien de la clé.
fn same(presented: &str, key: &str) -> bool {
    presented.len() == key.len()
        && presented
            .bytes()
            .zip(key.bytes())
            .fold(0_u8, |differs, (a, b)| differs | (a ^ b))
            == 0
}

/// Une connexion arrive par la porte de l'étagère.
async fn shelf_side(relay: Arc<Relay>, mut stream: TcpStream) {
    let Some(head) = head(&mut stream, counter::HEAD_MAX).await else {
        return;
    };
    match ask(&head, &relay.key) {
        Ask::Wire => wire(&relay, stream).await,
        Ask::Door(id) => door(&relay, stream, id).await,
        Ask::Counter => counter_side(stream, &head).await,
        Ask::Stranger => answer(stream, "401 Unauthorized").await,
        Ask::Unknown => answer(stream, "404 Not Found").await,
    }
}

/// Une question au guichet : le relais y met les secrets, la pose, et rend
/// la réponse. Le journal ne dit que l'hôte appelé et ce qu'il a répondu.
async fn counter_side(mut stream: TcpStream, head: &str) {
    let asked = match counter::read(head) {
        Ok(call) => {
            let mut body = vec![0_u8; call.length];
            let read = tokio::time::timeout(PATIENCE, stream.read_exact(&mut body)).await;
            if matches!(read, Ok(Ok(_))) {
                tokio::task::spawn_blocking(move || counter::ask(&call, &body, &counter::secret))
                    .await
                    .unwrap_or_else(|_| {
                        Err(counter::Refusal {
                            status: "500 Internal Server Error",
                            why: "le guichet s'est arrêté".to_owned(),
                        })
                    })
            } else {
                return;
            }
        }
        Err(refusal) => Err(refusal),
    };
    let asked = match asked {
        Ok((host, status, body)) => {
            println!("relais : guichet, {host} répond {status}");
            Ok((status, body))
        }
        Err(refusal) => {
            println!("relais : guichet, refusé ({})", refusal.why);
            Err(refusal)
        }
    };
    let _: std::io::Result<()> = stream.write_all(&counter::reply(asked)).await;
    let _: std::io::Result<()> = stream.shutdown().await;
}

async fn answer(mut stream: TcpStream, status: &str) {
    let reply = format!("HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n");
    let _: std::io::Result<()> = stream.write_all(reply.as_bytes()).await;
    let _: std::io::Result<()> = stream.shutdown().await;
}

/// Accepte la demande : à partir d'ici, la connexion ne porte plus que des octets.
async fn accept(stream: &mut TcpStream) -> std::io::Result<()> {
    let reply = format!(
        "HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: {PROTOCOL}\r\n\r\n"
    );
    stream.write_all(reply.as_bytes()).await
}

/// Le fil de l'étagère : tant qu'il tient, la soirée est ouverte.
async fn wire(relay: &Relay, mut stream: TcpStream) {
    if accept(&mut stream).await.is_err() {
        return;
    }
    let (calls, mut called) = mpsc::unbounded_channel();
    let rank = {
        let mut state = relay.state();
        state.shelves = state.shelves.wrapping_add(1).max(1);
        // Une étagère qui arrive remplace la précédente, et ce qui l'attendait.
        state.shelf = Some((state.shelves, calls));
        state.waiting.clear();
        state.shelves
    };
    let _: u64 = relay.linked.send_replace(rank);
    println!("relais : une étagère est reliée");
    let (mut from_shelf, mut to_shelf) = stream.split();
    let mut beat = tokio::time::interval(BEAT);
    let mut heard = [0_u8; 64];
    let mut spoke = tokio::time::Instant::now();
    loop {
        let silent = spoke.checked_add(SILENCE).unwrap_or(spoke);
        let line = tokio::select! {
            () = tokio::time::sleep_until(silent) => break,
            id = called.recv() => match id {
                Some(id) => format!("{id}\n"),
                // Une autre étagère a pris la place.
                None => break,
            },
            _ = beat.tick() => "\n".to_owned(),
            // Ce que l'étagère écrit ne compte pas : seulement qu'elle soit là.
            read = from_shelf.read(&mut heard) => match read {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    spoke = tokio::time::Instant::now();
                    continue;
                }
            },
        };
        if to_shelf.write_all(line.as_bytes()).await.is_err() {
            break;
        }
    }
    let mut state = relay.state();
    if state
        .shelf
        .as_ref()
        .is_some_and(|(linked, _)| *linked == rank)
    {
        state.shelf = None;
        state.waiting.clear();
        drop(state);
        let _: u64 = relay.linked.send_replace(0);
        println!("relais : l'étagère est partie");
    }
}

/// L'étagère vient chercher un téléphone : les deux connexions n'en font
/// plus qu'une, jusqu'à ce que l'une se ferme ou que l'étagère parte.
async fn door(relay: &Relay, mut stream: TcpStream, id: u64) {
    let waiting = relay.state().waiting.remove(&id);
    let Some(mut phone) = waiting else {
        return answer(stream, "404 Not Found").await;
    };
    if accept(&mut stream).await.is_err() {
        return;
    }
    // Un geste de manette n'attend pas qu'un paquet se remplisse.
    let _: std::io::Result<()> = stream.set_nodelay(true);
    let _: std::io::Result<()> = phone.set_nodelay(true);
    let mut linked = relay.linked.subscribe();
    let rank = *linked.borrow_and_update();
    tokio::select! {
        _ = tokio::io::copy_bidirectional(&mut stream, &mut phone) => {}
        _ = linked.wait_for(|now| *now != rank) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "une-cle-assez-longue-pour-le-relais";

    fn request(path: &str, key: &str) -> String {
        format!(
            "GET {path} HTTP/1.1\r\nHost: relais\r\nConnection: Upgrade\r\nUpgrade: {PROTOCOL}\r\n\
             authorization: Bearer {key}\r\n\r\n"
        )
    }

    #[test]
    fn the_shelf_asks_for_its_wire_or_for_a_phone() {
        assert_eq!(ask(&request(WIRE_PATH, KEY), KEY), Ask::Wire);
        assert_eq!(ask(&request("/_relais/porte/42", KEY), KEY), Ask::Door(42));
        assert_eq!(ask(&request("/_relais/porte/x", KEY), KEY), Ask::Unknown);
        assert_eq!(ask(&request("/t", KEY), KEY), Ask::Unknown);
        let counter = format!(
            "POST {} HTTP/1.1\r\nAuthorization: Bearer {KEY}\r\n\r\n",
            counter::PATH
        );
        assert_eq!(ask(&counter, KEY), Ask::Counter);
        // Le guichet ne se demande pas sans la clé.
        let stranger = format!("POST {} HTTP/1.1\r\n\r\n", counter::PATH);
        assert_eq!(ask(&stranger, KEY), Ask::Stranger);
        assert_eq!(ask(&request(counter::PATH, KEY), KEY), Ask::Unknown);
    }

    #[test]
    fn without_the_key_nobody_is_the_shelf() {
        assert_eq!(
            ask(&request(WIRE_PATH, "une-autre-cle"), KEY),
            Ask::Stranger
        );
        assert_eq!(ask(&request(WIRE_PATH, ""), KEY), Ask::Stranger);
        assert_eq!(
            ask("GET /_relais/etagere HTTP/1.1\r\n\r\n", KEY),
            Ask::Stranger
        );
        // Une clé plus longue qui commence comme la bonne n'est pas la bonne.
        let longer = format!("{KEY}x");
        assert_eq!(ask(&request(WIRE_PATH, &longer), KEY), Ask::Stranger);
    }
}
