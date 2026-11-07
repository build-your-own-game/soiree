//! Ce que partagent les tests du relais sur de vraies connexions : un relais lancé dans
//! un dossier de garde neuf, et un client WebSocket écrit à la main, qui fait ce qu'on
//! lui dit (répondre ou non aux battements, lire ou non), pour éprouver la coque.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use liaison::{CodeSalle, DuRelais, PROTOCOLE_RELAIS, Role, VersRelais};
use relais::adresses::Interface;
use relais::coque::{self, Reglages, Relais};
use relais::http::Banc;

/// Un dossier neuf sous `target/`, propre à ce test.
pub fn dossier(nom: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("relais-{nom}-{}", std::process::id()));
    let _: std::io::Result<()> = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Une machine sans réseau local : la boucle seulement.
pub fn sans_reseau() -> Vec<Interface> {
    vec![Interface {
        nom: String::from("lo0"),
        ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
    }]
}

/// Les réglages d'un relais de test : un port libre, sans réseau, le banc ou non.
pub fn reglages(nom: &str, banc: Banc) -> Reglages {
    Reglages {
        port: 0,
        adresse: None,
        garde: dossier(nom),
        banc,
        enregistrer: None,
        interfaces: Some(sans_reseau()),
        page: None,
        paquet: None,
        jeux: relais::paquet::Jeux::default(),
    }
}

/// Un relais du banc, sans réseau.
pub fn relais(nom: &str) -> Relais {
    coque::lancer(&reglages(nom, Banc::Oui)).unwrap()
}

/// Une requête HTTP entière, et sa réponse entière : (statut, en-têtes et corps).
pub fn http(port: u16, requete: &str) -> (u16, String) {
    let mut f = TcpStream::connect(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)).unwrap();
    f.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    f.write_all(requete.as_bytes()).unwrap();
    let mut o = Vec::new();
    let _: std::io::Result<usize> = f.read_to_end(&mut o);
    let texte = String::from_utf8_lossy(&o).into_owned();
    let statut = texte
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (statut, texte)
}

/// Ce qu'un client reçoit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recu {
    /// Une trame binaire.
    Binaire(Vec<u8>),
    /// Un battement.
    Ping,
    /// La trame de fermeture.
    Fermeture,
    /// La connexion est fermée.
    Fin,
    /// Rien dans le temps imparti.
    Rien,
}

/// Un client WebSocket écrit à la main.
pub struct Client {
    pub flux: TcpStream,
    tampon: Vec<u8>,
    /// Répond-il aux battements ?
    pub repond: bool,
}

/// Le statut de la réponse à la demande de WebSocket, et le client s'il est accepté.
pub fn ouvrir(adresse: SocketAddr, entetes: &[(&str, &str)]) -> Result<Client, u16> {
    let mut f = TcpStream::connect(adresse).unwrap();
    f.set_nodelay(true).unwrap();
    let mut r = String::from("GET /ws HTTP/1.1\r\n");
    for (n, v) in entetes {
        r.push_str(n);
        r.push_str(": ");
        r.push_str(v);
        r.push_str("\r\n");
    }
    r.push_str(
        "Upgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
    );
    f.write_all(r.as_bytes()).unwrap();
    f.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut reponse = Vec::new();
    let mut b = [0u8; 1];
    while !reponse.ends_with(b"\r\n\r\n") {
        match f.read(&mut b) {
            Ok(1) => reponse.push(b[0]),
            _ => break,
        }
    }
    let texte = String::from_utf8_lossy(&reponse).into_owned();
    let statut: u16 = texte
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if statut != 101 {
        return Err(statut);
    }
    assert!(
        texte.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="),
        "{texte}"
    );
    Ok(Client {
        flux: f,
        tampon: Vec::new(),
        repond: true,
    })
}

/// Un client de la boucle locale, sans `Origin` (le banc).
pub fn client(port: u16) -> Client {
    let host = format!("127.0.0.1:{port}");
    ouvrir(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
        &[("Host", &host)],
    )
    .unwrap()
}

impl Client {
    fn trame(&mut self, opcode: u8, charge: &[u8]) {
        let masque = [0x11u8, 0x22, 0x33, 0x44];
        let mut o = vec![0b1000_0000 | opcode];
        let n = charge.len();
        if n < 126 {
            o.push(0b1000_0000 | u8::try_from(n).unwrap());
        } else if n <= 0xffff {
            o.push(0b1000_0000 | 0x7e);
            o.extend_from_slice(&u16::try_from(n).unwrap().to_be_bytes());
        } else {
            o.push(0b1000_0000 | 0x7f);
            o.extend_from_slice(&(n as u64).to_be_bytes());
        }
        o.extend_from_slice(&masque);
        o.extend(charge.iter().enumerate().map(|(i, b)| b ^ masque[i % 4]));
        let _: std::io::Result<()> = self.flux.write_all(&o);
    }

    /// Envoie une trame binaire.
    pub fn envoyer(&mut self, octets: &[u8]) {
        self.trame(0x2, octets);
    }

    /// Envoie un message au relais.
    pub fn message(&mut self, m: &VersRelais<'_>) {
        let o = m.encoder().unwrap();
        self.envoyer(&o);
    }

    /// Dit bonjour.
    pub fn bonjour(&mut self, salle: CodeSalle, role: Role) {
        self.message(&VersRelais::Bonjour {
            protocole: PROTOCOLE_RELAIS,
            salle,
            role,
        });
    }

    /// Rend un battement.
    pub fn pong(&mut self) {
        self.trame(0xa, &[]);
    }

    fn une_trame(&mut self) -> Option<(u8, Vec<u8>)> {
        let t = self.tampon.as_slice();
        if t.len() < 2 {
            return None;
        }
        let opcode = t[0] & 0x0f;
        let (n, debut) = match t[1] & 0x7f {
            126 => {
                if t.len() < 4 {
                    return None;
                }
                (usize::from(u16::from_be_bytes([t[2], t[3]])), 4)
            }
            127 => {
                if t.len() < 10 {
                    return None;
                }
                (
                    usize::try_from(u64::from_be_bytes(t[2..10].try_into().unwrap())).unwrap(),
                    10,
                )
            }
            n => (usize::from(n), 2),
        };
        if t.len() < debut + n {
            return None;
        }
        let charge = t[debut..debut + n].to_vec();
        self.tampon.drain(..debut + n);
        Some((opcode, charge))
    }

    /// La prochaine trame, dans le temps imparti. Un battement reçu est rendu si le
    /// client répond.
    pub fn recevoir(&mut self, delai: Duration) -> Recu {
        let fin = Instant::now() + delai;
        loop {
            if let Some((op, charge)) = self.une_trame() {
                return match op {
                    0x2 => Recu::Binaire(charge),
                    0x9 => {
                        if self.repond {
                            self.pong();
                        }
                        Recu::Ping
                    }
                    0x8 => Recu::Fermeture,
                    _ => continue,
                };
            }
            let reste = fin.saturating_duration_since(Instant::now());
            if reste.is_zero() {
                return Recu::Rien;
            }
            // Sous macOS, régler une connexion que le pair a coupée rend `EINVAL`.
            if self.flux.set_read_timeout(Some(reste)).is_err() {
                return Recu::Fin;
            }
            let mut b = vec![0u8; 65536];
            match self.flux.read(&mut b) {
                Ok(0) => return Recu::Fin,
                Ok(n) => self.tampon.extend_from_slice(&b[..n]),
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    return Recu::Rien;
                }
                Err(_) => return Recu::Fin,
            }
        }
    }

    /// Le prochain message du relais, les battements passés (et rendus si le client
    /// répond) ; `None` si rien n'arrive dans le temps imparti ou si le lien finit.
    pub fn du_relais(&mut self, delai: Duration) -> Option<DuRelais<'static>> {
        let fin = Instant::now() + delai;
        loop {
            let reste = fin.saturating_duration_since(Instant::now());
            match self.recevoir(reste) {
                Recu::Binaire(o) => {
                    let o: &'static [u8] = Vec::leak(o);
                    return Some(DuRelais::decoder(o).unwrap());
                }
                Recu::Ping => {}
                Recu::Fermeture | Recu::Fin | Recu::Rien => return None,
            }
        }
    }

    /// Attend la fin du lien : la trame de fermeture, puis la connexion fermée.
    pub fn attendre_la_fin(&mut self, delai: Duration) -> bool {
        let fin = Instant::now() + delai;
        loop {
            let reste = fin.saturating_duration_since(Instant::now());
            match self.recevoir(reste) {
                Recu::Fin => return true,
                Recu::Rien => return false,
                Recu::Binaire(_) | Recu::Ping | Recu::Fermeture => {}
            }
        }
    }
}
