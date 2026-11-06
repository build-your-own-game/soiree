//! Un client WebSocket **non bloquant**, pour les bouts natifs de `soiree-native` (une
//! autorité, des téléphones) : `tungstenite` sur un `TcpStream` de `std`, la poignée de
//! main faite en bloquant, puis chaque lecture et chaque écriture sans attendre. Une
//! trame qui ne part pas tout de suite reste dans le tampon de `tungstenite`, et part au
//! prochain [`Ws::vider`].

use std::io;
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::time::Duration;

use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Bytes, Message, WebSocket};

/// Le lien est perdu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Perdu(pub String);

/// Un client WebSocket non bloquant.
pub struct Ws {
    ws: WebSocket<TcpStream>,
    local: Option<SocketAddr>,
}

fn bloque(e: &tungstenite::Error) -> bool {
    matches!(e, tungstenite::Error::Io(io) if io.kind() == io::ErrorKind::WouldBlock)
}

impl Ws {
    /// Ouvre `ws://<adresse>/ws` (sans `Origin` : un outil natif du banc), en acceptant
    /// des messages jusqu'à `max` octets.
    pub fn relier(adresse: SocketAddr, max: usize) -> Result<Ws, Perdu> {
        Ws::relier_par(adresse, adresse, max)
    }

    /// Ouvre `ws://<hote>/ws` par une connexion à `adresse` : un téléphone derrière le
    /// mandataire demande le relais (son `Host`), comme une page derrière un Wi-Fi.
    pub fn relier_par(adresse: SocketAddr, hote: SocketAddr, max: usize) -> Result<Ws, Perdu> {
        let flux = TcpStream::connect_timeout(&adresse, Duration::from_secs(2))
            .map_err(|e| Perdu(format!("joindre {adresse} : {e}")))?;
        let _: io::Result<()> = flux.set_nodelay(true);
        flux.set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| Perdu(format!("{e}")))?;
        let local = flux.local_addr().ok();
        let config = WebSocketConfig::default()
            .max_message_size(Some(max))
            .max_frame_size(Some(max));
        let (ws, _) =
            tungstenite::client::client_with_config(format!("ws://{hote}/ws"), flux, Some(config))
                .map_err(|e| Perdu(format!("la poignée de main WebSocket avec {adresse} : {e}")))?;
        ws.get_ref()
            .set_nonblocking(true)
            .map_err(|e| Perdu(format!("{e}")))?;
        Ok(Ws { ws, local })
    }

    /// L'adresse locale de la connexion : le mandataire la reconnaît.
    pub const fn locale(&self) -> Option<SocketAddr> {
        self.local
    }

    /// Envoie un message binaire ; s'il ne part pas tout de suite, il attend le prochain
    /// [`Ws::vider`].
    pub fn envoyer(&mut self, octets: Vec<u8>) -> Result<(), Perdu> {
        match self.ws.write(Message::Binary(Bytes::from(octets))) {
            Ok(()) => self.vider(),
            Err(e) if bloque(&e) => Ok(()),
            Err(e) => Err(Perdu(format!("écrire : {e}"))),
        }
    }

    /// Fait partir ce qui attend.
    pub fn vider(&mut self) -> Result<(), Perdu> {
        match self.ws.flush() {
            Ok(()) => Ok(()),
            Err(e) if bloque(&e) => Ok(()),
            Err(e) => Err(Perdu(format!("vider : {e}"))),
        }
    }

    /// Le prochain message binaire, s'il y en a un déjà là. Les battements du relais sont
    /// rendus par `tungstenite`, à la lecture.
    pub fn recevoir(&mut self) -> Result<Option<Vec<u8>>, Perdu> {
        loop {
            match self.ws.read() {
                Ok(Message::Binary(b)) => return Ok(Some(b.to_vec())),
                Ok(Message::Ping(_) | Message::Pong(_) | Message::Text(_) | Message::Frame(_)) => {}
                Ok(Message::Close(_)) => return Err(Perdu(String::from("fermé par le relais"))),
                Err(e) if bloque(&e) => return Ok(None),
                Err(e) => return Err(Perdu(format!("lire : {e}"))),
            }
        }
    }

    /// Coupe la connexion, sans au revoir : un onglet qui meurt, un processus tué.
    pub fn couper(&self) {
        let _: io::Result<()> = self.ws.get_ref().shutdown(Shutdown::Both);
    }
}
