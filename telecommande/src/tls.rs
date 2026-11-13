//! Le HTTPS de l'hôte : le même port répond en clair et en chiffré.
//!
//! Safari ne donne les capteurs de mouvement d'un téléphone qu'à une page
//! servie en HTTPS. Quand l'hôte fournit un certificat (voir [`Identity`]),
//! le jeu répond en HTTPS aux téléphones, sur le même port : l'écran, sur
//! l'ordinateur, continue de s'ouvrir en clair à `http://localhost`.
//!
//! Le premier octet d'une connexion suffit à savoir : `0x16` ouvre une
//! poignée de main TLS, tout le reste est du HTTP en clair.

use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use rustls_pki_types::pem::PemObject as _;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::crypto::ring;
use tokio_rustls::server::TlsStream;

/// Le premier octet d'une poignée de main TLS.
const HANDSHAKE_BYTE: u8 = 0x16;
/// Une connexion qui ne dit rien aussi longtemps est abandonnée.
const PATIENCE: Duration = Duration::from_secs(10);
/// Les connexions prêtes qui attendent d'être servies.
const BACKLOG: usize = 64;

/// Le nom et le certificat sous lesquels le jeu répond en HTTPS.
///
/// Trois variables, données par l'hôte : `SOIREE_TLS_NOM` (le nom qui pointe
/// vers cet ordinateur sur le Wi-Fi), `SOIREE_TLS_CERT` (le certificat et sa
/// chaîne, en PEM) et `SOIREE_TLS_KEY` (la clé privée, en PEM). Ce sont les
/// contenus, pas des chemins : la clé ne passe par aucun fichier.
pub struct Identity {
    pub name: String,
    config: Arc<ServerConfig>,
}

impl Identity {
    /// L'identité donnée par l'hôte, ou `None` s'il n'en donne pas.
    pub fn from_env() -> io::Result<Option<Self>> {
        let read = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        let (name, cert, key) = match (
            read("SOIREE_TLS_NOM"),
            read("SOIREE_TLS_CERT"),
            read("SOIREE_TLS_KEY"),
        ) {
            (None, None, None) => return Ok(None),
            (Some(name), Some(cert), Some(key)) => (name, cert, key),
            (_, _, _) => {
                return Err(io::Error::other(
                    "HTTPS : il faut les trois variables SOIREE_TLS_NOM, SOIREE_TLS_CERT et \
                     SOIREE_TLS_KEY, ou aucune",
                ));
            }
        };
        Self::new(name, cert.as_bytes(), key.as_bytes()).map(Some)
    }

    fn new(name: String, cert: &[u8], key: &[u8]) -> io::Result<Self> {
        let unreadable = |what: &str, error: &dyn std::fmt::Display| {
            io::Error::other(format!("HTTPS : {what} ne se lit pas ({error})"))
        };
        let chain: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(cert)
            .collect::<Result<_, _>>()
            .map_err(|error| unreadable("SOIREE_TLS_CERT", &error))?;
        if chain.is_empty() {
            return Err(io::Error::other(
                "HTTPS : SOIREE_TLS_CERT ne contient aucun certificat",
            ));
        }
        let key = PrivateKeyDer::from_pem_slice(key)
            .map_err(|error| unreadable("SOIREE_TLS_KEY", &error))?;
        let mut config = ServerConfig::builder_with_provider(Arc::new(ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(io::Error::other)?
            .with_no_client_auth()
            .with_single_cert(chain, key)
            .map_err(|error| unreadable("le certificat ou sa clé", &error))?;
        // Le lien avec les pages est un WebSocket : il lui faut HTTP/1.1.
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok(Self {
            name,
            config: Arc::new(config),
        })
    }
}

/// Une connexion, en clair ou chiffrée.
pub enum Stream {
    Plain(TcpStream),
    Secure(Box<TlsStream<TcpStream>>),
}

impl AsyncRead for Stream {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_read(context, buffer),
            Self::Secure(stream) => Pin::new(stream.as_mut()).poll_read(context, buffer),
        }
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_write(context, bytes),
            Self::Secure(stream) => Pin::new(stream.as_mut()).poll_write(context, bytes),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_flush(context),
            Self::Secure(stream) => Pin::new(stream.as_mut()).poll_flush(context),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_shutdown(context),
            Self::Secure(stream) => Pin::new(stream.as_mut()).poll_shutdown(context),
        }
    }
}

/// Le port du jeu : il rend des connexions prêtes, en clair ou chiffrées.
pub struct Listener {
    local: SocketAddr,
    ready: mpsc::Receiver<(Stream, SocketAddr)>,
}

impl Listener {
    /// Écoute sur `listener`. Sans identité, seul le clair est servi.
    ///
    /// À appeler depuis le moteur asynchrone : chaque connexion est préparée
    /// à part, pour qu'un téléphone lent n'en fasse attendre aucun autre.
    pub fn new(listener: TcpListener, identity: Option<&Identity>) -> io::Result<Self> {
        let local = listener.local_addr()?;
        let acceptor = identity.map(|identity| TlsAcceptor::from(Arc::clone(&identity.config)));
        let (sender, ready) = mpsc::channel(BACKLOG);
        let _: tokio::task::JoinHandle<()> = tokio::spawn(async move {
            loop {
                let Ok((stream, address)) = listener.accept().await else {
                    // Plus de descripteurs, par exemple : on laisse passer.
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                };
                let acceptor = acceptor.clone();
                let sender = sender.clone();
                let _: tokio::task::JoinHandle<()> = tokio::spawn(async move {
                    if let Some(stream) = prepare(stream, acceptor).await {
                        let _: Result<(), _> = sender.send((stream, address)).await;
                    }
                });
            }
        });
        Ok(Self { local, ready })
    }
}

/// Regarde le premier octet et, s'il le faut, fait la poignée de main TLS.
async fn prepare(stream: TcpStream, acceptor: Option<TlsAcceptor>) -> Option<Stream> {
    let mut first = [0_u8; 1];
    let read = tokio::time::timeout(PATIENCE, stream.peek(&mut first))
        .await
        .ok()?
        .ok()?;
    if read == 0 {
        return None;
    }
    if first != [HANDSHAKE_BYTE] {
        return Some(Stream::Plain(stream));
    }
    // Du TLS sans certificat : on ne sait pas répondre.
    let secure = tokio::time::timeout(PATIENCE, acceptor?.accept(stream))
        .await
        .ok()?
        .ok()?;
    Some(Stream::Secure(Box::new(secure)))
}

impl axum::serve::Listener for Listener {
    type Io = Stream;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            if let Some(connection) = self.ready.recv().await {
                return connection;
            }
            // La tâche d'écoute ne s'arrête qu'avec le programme.
            std::future::pending::<()>().await;
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        Ok(self.local)
    }
}
