//! Le passage par internet : l'étagère rejoint le relais de l'hôte, et les
//! téléphones n'ont plus besoin d'être sur son Wi-Fi.
//!
//! Le relais (`relais/`, sur un serveur joignable depuis internet) reçoit les
//! téléphones. L'ordinateur de l'hôte, lui, n'est pas joignable : c'est
//! l'étagère qui va vers le relais. Elle y tient un fil ouvert, où le relais
//! lui dit chaque téléphone qui arrive ; pour chacun, elle ouvre une
//! connexion de plus vers le relais, et la joint, octet pour octet, à sa
//! propre porte des téléphones. Pour le jeu, rien ne change : un téléphone
//! venu par internet arrive comme un téléphone du Wi-Fi.
//!
//! L'hôte le règle par deux variables : `SOIREE_INTERNET`, l'adresse du
//! relais (celle que les téléphones ouvrent), et `SOIREE_INTERNET_CLE`, la
//! clé qu'il partage avec lui. Sans elles, l'étagère ne propose rien.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};

const ADDRESS_VARIABLE: &str = "SOIREE_INTERNET";
pub const KEY_VARIABLE: &str = "SOIREE_INTERNET_CLE";
/// Là où l'étagère rejoint le relais, quand ce n'est pas l'adresse des
/// téléphones : un relais essayé sans serveur web devant lui.
const DOOR_VARIABLE: &str = "SOIREE_INTERNET_PORTE";
/// Ce que l'étagère demande au relais (voir `relais/src/main.rs`).
const WIRE_PATH: &str = "/_relais/etagere";
const DOOR_PATH: &str = "/_relais/porte/";
const PROTOCOL: &str = "soiree-relais";
const HEAD_MAX: usize = 8 * 1024;
const PATIENCE: Duration = Duration::from_secs(10);
/// Le relais dit quelque chose au moins tous les quarts de minute : un fil
/// muet plus longtemps que ça est un fil coupé.
const SILENCE: Duration = Duration::from_secs(45);
const BEAT: Duration = Duration::from_secs(15);
const RETRY: Duration = Duration::from_secs(2);

/// Une adresse où se connecter.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Target {
    secure: bool,
    host: String,
    port: u16,
}

impl Target {
    /// Lit `https://nom`, `https://nom:port` ou `http://…`, sans chemin.
    fn read(address: &str) -> Option<Self> {
        let (secure, rest) = match address.split_once("://")? {
            ("https", rest) => (true, rest),
            ("http", rest) => (false, rest),
            _ => return None,
        };
        let rest = rest.trim_end_matches('/');
        if rest.is_empty() || rest.contains(['/', '?', '#', '@', ' ']) {
            return None;
        }
        let (host, port) = match rest.rsplit_once(':') {
            Some((host, port)) => (host, port.parse().ok()?),
            None => (rest, if secure { 443 } else { 80 }),
        };
        (!host.is_empty()).then(|| Self {
            secure,
            host: host.to_owned(),
            port,
        })
    }
}

/// Ce que l'hôte a réglé pour passer par internet.
#[derive(Clone)]
pub struct Passage {
    /// L'adresse que les téléphones ouvrent, sans barre à la fin.
    phones: String,
    /// Là où l'étagère rejoint le relais.
    door: Target,
    key: String,
}

impl Passage {
    /// Lit les variables de l'hôte. `Ok(None)` : il n'a rien réglé.
    pub fn from_env() -> Result<Option<Self>, String> {
        let read = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        let Some(address) = read(ADDRESS_VARIABLE) else {
            return Ok(None);
        };
        let key = read(KEY_VARIABLE).ok_or_else(|| {
            format!(
                "{ADDRESS_VARIABLE} est donnée sans {KEY_VARIABLE} : il faut la clé que le relais \
                 attend (`fnox set {KEY_VARIABLE}`)."
            )
        })?;
        let wrong = |name: &str, value: &str| {
            format!("{name} n'est pas une adresse (`https://nom`, sans chemin) : {value}")
        };
        if Target::read(&address).is_none() {
            return Err(wrong(ADDRESS_VARIABLE, &address));
        }
        let door = read(DOOR_VARIABLE).unwrap_or_else(|| address.clone());
        let door = Target::read(&door).ok_or_else(|| wrong(DOOR_VARIABLE, &door))?;
        Ok(Some(Self {
            phones: address.trim_end_matches('/').to_owned(),
            door,
            key,
        }))
    }

    /// L'adresse que les téléphones ouvrent.
    pub fn phones(&self) -> &str {
        &self.phones
    }

    /// Rejoint le relais et lui amène les téléphones à ce port de cet
    /// ordinateur, jusqu'à ce que le lien soit lâché.
    pub fn open(&self, local: u16) -> Link {
        let linked = Arc::new(AtomicBool::new(false));
        let (stop, stopped) = oneshot::channel::<()>();
        let (passage, flag) = (self.clone(), Arc::clone(&linked));
        let _: std::thread::JoinHandle<()> = std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            runtime.block_on(async {
                tokio::select! {
                    () = passage.hold(local, &flag) => {}
                    _ = stopped => {}
                }
            });
            flag.store(false, Ordering::Relaxed);
        });
        Link {
            linked,
            stop: Some(stop),
        }
    }

    /// Tient le fil du relais, et le reprend chaque fois qu'il casse.
    async fn hold(&self, local: u16, linked: &AtomicBool) {
        loop {
            if let Ok(mut wire) = self.dial(WIRE_PATH).await {
                linked.store(true, Ordering::Relaxed);
                self.listen(&mut wire, local).await;
                linked.store(false, Ordering::Relaxed);
            }
            tokio::time::sleep(RETRY).await;
        }
    }

    /// Écoute le fil : un numéro par ligne, un téléphone à aller chercher.
    async fn listen(&self, wire: &mut Stream, local: u16) {
        let mut beat = tokio::time::interval(BEAT);
        let mut line = String::new();
        let mut heard = [0_u8; 256];
        loop {
            let read = tokio::select! {
                read = tokio::time::timeout(SILENCE, wire.read(&mut heard)) => read,
                // De quoi garder le fil ouvert en chemin, dans ce sens aussi.
                _ = beat.tick() => {
                    if wire.write_all(b"\n").await.is_err() {
                        return;
                    }
                    continue;
                }
            };
            let Ok(Ok(count @ 1..)) = read else {
                return;
            };
            for byte in heard.iter().take(count) {
                if *byte != b'\n' {
                    line.push(char::from(*byte));
                    continue;
                }
                if let Ok(id) = line.trim().parse::<u64>() {
                    let passage = self.clone();
                    let _: tokio::task::JoinHandle<()> =
                        tokio::spawn(async move { passage.fetch(id, local).await });
                }
                line.clear();
            }
        }
    }

    /// Va chercher un téléphone au relais et l'amène à la porte des
    /// téléphones de l'étagère, comme s'il arrivait du Wi-Fi.
    async fn fetch(&self, id: u64, local: u16) {
        let Ok(mut phone) = self.dial(&format!("{DOOR_PATH}{id}")).await else {
            return;
        };
        let Ok(mut door) = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, local)).await else {
            return;
        };
        let _: std::io::Result<()> = door.set_nodelay(true);
        let _: std::io::Result<(u64, u64)> =
            tokio::io::copy_bidirectional(&mut phone, &mut door).await;
    }

    /// Ouvre une connexion vers le relais et lui fait une demande. Quand il
    /// l'accepte, la connexion ne porte plus que des octets.
    async fn dial(&self, path: &str) -> std::io::Result<Stream> {
        let dialing = async {
            let tcp = TcpStream::connect((self.door.host.as_str(), self.door.port)).await?;
            let _: std::io::Result<()> = tcp.set_nodelay(true);
            let mut stream: Stream = if self.door.secure {
                Box::new(secure(tcp, &self.door.host).await?)
            } else {
                Box::new(tcp)
            };
            let ask = format!(
                "GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: Upgrade\r\nUpgrade: {PROTOCOL}\r\n\
                 Authorization: Bearer {}\r\n\r\n",
                self.door.host, self.key
            );
            stream.write_all(ask.as_bytes()).await?;
            let mut head: Vec<u8> = Vec::new();
            while !head.ends_with(b"\r\n\r\n") {
                if head.len() >= HEAD_MAX {
                    return Err(std::io::Error::other("réponse du relais trop longue"));
                }
                head.push(stream.read_u8().await?);
            }
            let accepted = head
                .split(|byte| *byte == b' ')
                .nth(1)
                .is_some_and(|status| status == b"101");
            if accepted {
                Ok(stream)
            } else {
                Err(std::io::Error::other("le relais refuse"))
            }
        };
        tokio::time::timeout(PATIENCE, dialing)
            .await
            .unwrap_or_else(|_| Err(std::io::Error::other("le relais ne répond pas")))
    }
}

/// Une connexion vers le relais, chiffrée ou non.
trait Duplex: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Duplex for T {}
type Stream = Box<dyn Duplex>;

/// Chiffre la connexion, en vérifiant que le relais est bien ce nom-là.
async fn secure(
    tcp: TcpStream,
    host: &str,
) -> std::io::Result<tokio_rustls::client::TlsStream<TcpStream>> {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let mut config = ClientConfig::builder_with_provider(Arc::new(
        tokio_rustls::rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(std::io::Error::other)?
    .with_root_certificates(roots)
    .with_no_client_auth();
    // La demande s'écrit en HTTP/1.1 : c'est lui qui sait changer de protocole.
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let name = ServerName::try_from(host.to_owned()).map_err(std::io::Error::other)?;
    TlsConnector::from(Arc::new(config))
        .connect(name, tcp)
        .await
}

/// Le lien tenu avec le relais. Le lâcher le ferme, et tout ce qu'il portait.
pub struct Link {
    linked: Arc<AtomicBool>,
    stop: Option<oneshot::Sender<()>>,
}

impl Link {
    /// Le relais est-il joint, là, maintenant ?
    pub fn linked(&self) -> bool {
        self.linked.load(Ordering::Relaxed)
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _: Result<(), ()> = stop.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_is_a_name_with_or_without_a_port() {
        let read = |address| Target::read(address).map(|at| (at.secure, at.host, at.port));
        assert_eq!(
            read("https://relais.exemple.com"),
            Some((true, "relais.exemple.com".to_owned(), 443))
        );
        assert_eq!(
            read("https://relais.exemple.com/"),
            Some((true, "relais.exemple.com".to_owned(), 443))
        );
        assert_eq!(
            read("http://127.0.0.1:9102"),
            Some((false, "127.0.0.1".to_owned(), 9102))
        );
        // Ni chemin, ni identifiants, ni autre protocole.
        assert_eq!(read("https://relais.exemple.com/t"), None);
        assert_eq!(read("https://moi@relais.exemple.com"), None);
        assert_eq!(read("ftp://relais.exemple.com"), None);
        assert_eq!(read("relais.exemple.com"), None);
        assert_eq!(read("https://"), None);
    }
}
