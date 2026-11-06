//! Le mandataire de pannes (PHASE-3, § 8.1, `banc::mandataire`) : un relais TCP entre les
//! téléphones natifs et le relais de la soirée, qui fait au vrai trajet chiffré ce que le
//! Wi-Fi lui fait (SAL-86). Il **retarde** chaque morceau (une latence par sens, l'ordre
//! gardé, comme TCP), **groupe par rafales**, **retient** un sens un moment, **coupe**,
//! **ferme lentement** (la fermeture de la connexion part en retard, après les données :
//! la trame de fermeture de WebSocket, elle, est une donnée et passe devant, SAL-60) et
//! garde toujours une fermeture derrière les données de son lien (SAL-61).
//!
//! Ses valeurs sont **aujourd'hui celles du monde simulé de l'épreuve**, faute de mieux
//! (`epreuve/src/telephone.rs` : une latence de 16 à 66 ms par sens, une rafale de
//! 100 ms, une fermeture vue en 0,5 s) ; la mesure du vrai Wi-Fi (M5, lot 14) les
//! remplacera, ici comme là.

use std::collections::{BTreeMap, VecDeque};
use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use salle::simulation::Graine;

use crate::horloge::maintenant;

/// La latence d'un sens, en microsecondes : de 16 à 66 ms (le monde simulé de
/// l'épreuve, et `salle::simulation::Pannes::SOIREE`).
pub const LATENCE_US: (u64, u64) = (16_000, 66_000);

/// La rafale du Wi-Fi : les morceaux partent groupés toutes les 100 ms
/// (`epreuve::telephone::RAFALE`, 6 pas).
pub const RAFALE_US: u64 = 100_000;

/// Une fermeture lente : la fin de la connexion part 0,5 s après les données
/// (`epreuve::telephone::FERMETURE`, 30 pas).
pub const FERMETURE_LENTE_US: u64 = 500_000;

/// Un sens du mandataire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sens {
    /// Du téléphone vers le relais.
    VersRelais,
    /// Du relais vers le téléphone.
    VersTelephone,
}

impl Sens {
    const fn rang(self) -> usize {
        match self {
            Sens::VersRelais => 0,
            Sens::VersTelephone => 1,
        }
    }
}

#[derive(Debug, Default)]
struct Etat {
    retenu_jusqu_a: [u64; 2],
    rafales_jusqu_a: u64,
    fermeture_lente: bool,
    coupee: bool,
}

#[derive(Debug)]
struct Connexion {
    telephone: TcpStream,
    relais: TcpStream,
    etat: Mutex<Etat>,
}

impl Connexion {
    fn couper(&self) {
        if let Ok(mut e) = self.etat.lock() {
            e.coupee = true;
        }
        let _: io::Result<()> = self.telephone.shutdown(Shutdown::Both);
        let _: io::Result<()> = self.relais.shutdown(Shutdown::Both);
    }
}

type Connexions = Arc<Mutex<BTreeMap<u16, Arc<Connexion>>>>;

/// Le mandataire, lancé.
pub struct Mandataire {
    adresse: SocketAddr,
    connexions: Connexions,
    arret: Arc<AtomicBool>,
    fil: Option<JoinHandle<()>>,
    depart: std::time::Instant,
}

fn us_depuis(depart: std::time::Instant) -> u64 {
    u64::try_from(maintenant().saturating_duration_since(depart).as_micros()).unwrap_or(u64::MAX)
}

impl Mandataire {
    /// Écoute sur la boucle locale ; chaque connexion part vers `relais`.
    pub fn lancer(relais: SocketAddr, graine: u64) -> io::Result<Mandataire> {
        let ecoute = TcpListener::bind("127.0.0.1:0")?;
        let adresse = ecoute.local_addr()?;
        let connexions: Connexions = Arc::new(Mutex::new(BTreeMap::new()));
        let arret = Arc::new(AtomicBool::new(false));
        let depart = maintenant();
        let fil = {
            let c = Arc::clone(&connexions);
            let a = Arc::clone(&arret);
            thread::Builder::new()
                .name(String::from("mandataire"))
                .spawn(move || accepter(&ecoute, relais, &c, &a, graine, depart))?
        };
        Ok(Mandataire {
            adresse,
            connexions,
            arret,
            fil: Some(fil),
            depart,
        })
    }

    /// Où les téléphones se relient.
    pub const fn adresse(&self) -> SocketAddr {
        self.adresse
    }

    fn connexion(&self, telephone: SocketAddr) -> Option<Arc<Connexion>> {
        self.connexions
            .lock()
            .ok()
            .and_then(|c| c.get(&telephone.port()).cloned())
    }

    /// Coupe la connexion de ce téléphone, net : ce qui était en route est perdu.
    pub fn couper(&self, telephone: SocketAddr) {
        if let Some(c) = self.connexion(telephone) {
            c.couper();
        }
    }

    /// Retient un sens de la connexion de ce téléphone pendant `duree_us`.
    pub fn retenir(&self, telephone: SocketAddr, sens: Sens, duree_us: u64) {
        let t = us_depuis(self.depart).saturating_add(duree_us);
        if let Some(c) = self.connexion(telephone)
            && let Ok(mut e) = c.etat.lock()
            && let Some(r) = e.retenu_jusqu_a.get_mut(sens.rang())
        {
            *r = (*r).max(t);
        }
    }

    /// Groupe par rafales de [`RAFALE_US`] les deux sens de ce téléphone, `duree_us`.
    pub fn rafales(&self, telephone: SocketAddr, duree_us: u64) {
        let t = us_depuis(self.depart).saturating_add(duree_us);
        if let Some(c) = self.connexion(telephone)
            && let Ok(mut e) = c.etat.lock()
        {
            e.rafales_jusqu_a = e.rafales_jusqu_a.max(t);
        }
    }

    /// La prochaine fermeture de ce téléphone part lentement (SAL-60).
    pub fn fermer_lentement(&self, telephone: SocketAddr) {
        if let Some(c) = self.connexion(telephone)
            && let Ok(mut e) = c.etat.lock()
        {
            e.fermeture_lente = true;
        }
    }

    /// Les connexions ouvertes.
    pub fn ouvertes(&self) -> usize {
        self.connexions.lock().map_or(0, |c| c.len())
    }

    /// Arrête le mandataire et coupe chaque connexion.
    pub fn arreter(mut self) {
        self.arret.store(true, Ordering::SeqCst);
        let _: io::Result<TcpStream> =
            TcpStream::connect_timeout(&self.adresse, Duration::from_secs(1));
        if let Ok(c) = self.connexions.lock() {
            for x in c.values() {
                x.couper();
            }
        }
        if let Some(f) = self.fil.take() {
            let _: thread::Result<()> = f.join();
        }
    }
}

fn accepter(
    ecoute: &TcpListener,
    relais: SocketAddr,
    connexions: &Connexions,
    arret: &AtomicBool,
    graine: u64,
    depart: std::time::Instant,
) {
    let mut rang = 0u64;
    for flux in ecoute.incoming() {
        if arret.load(Ordering::SeqCst) {
            return;
        }
        let Ok(telephone) = flux else {
            continue;
        };
        let Ok(pair) = telephone.peer_addr() else {
            continue;
        };
        let Ok(vers) = TcpStream::connect_timeout(&relais, Duration::from_secs(2)) else {
            let _: io::Result<()> = telephone.shutdown(Shutdown::Both);
            continue;
        };
        let _: io::Result<()> = telephone.set_nodelay(true);
        let _: io::Result<()> = vers.set_nodelay(true);
        let (Ok(t2), Ok(r2), Ok(t3), Ok(r3)) = (
            telephone.try_clone(),
            vers.try_clone(),
            telephone.try_clone(),
            vers.try_clone(),
        ) else {
            continue;
        };
        let c = Arc::new(Connexion {
            telephone,
            relais: vers,
            etat: Mutex::new(Etat::default()),
        });
        if let Ok(mut m) = connexions.lock() {
            m.insert(pair.port(), Arc::clone(&c));
        }
        rang = rang.wrapping_add(1);
        for (de, vers, sens) in [(t2, r2, Sens::VersRelais), (r3, t3, Sens::VersTelephone)] {
            let c = Arc::clone(&c);
            let tous = Arc::clone(connexions);
            let h = Graine::new(
                "mandataire",
                graine
                    .wrapping_mul(7919)
                    .wrapping_add(rang.wrapping_mul(2))
                    .wrapping_add(match sens {
                        Sens::VersRelais => 0,
                        Sens::VersTelephone => 1,
                    }),
            );
            let lance = thread::Builder::new()
                .name(String::from("mandataire-sens"))
                .stack_size(256 * 1024)
                .spawn(move || {
                    pomper(de, vers, sens, &c, h, depart);
                    if sens == Sens::VersRelais
                        && let Ok(mut m) = tous.lock()
                    {
                        m.remove(&pair.port());
                    }
                });
            let _: io::Result<JoinHandle<()>> = lance;
        }
    }
}

/// Un sens : lire, retarder (l'ordre gardé), grouper, retenir, écrire ; à la fin de la
/// source, la fin part derrière les données, lentement si c'est dit.
fn pomper(
    mut de: TcpStream,
    mut vers: TcpStream,
    sens: Sens,
    c: &Connexion,
    mut h: Graine,
    depart: std::time::Instant,
) {
    let _: io::Result<()> = de.set_read_timeout(Some(Duration::from_millis(5)));
    let mut file: VecDeque<(u64, Vec<u8>)> = VecDeque::new();
    let mut dernier = 0u64;
    let mut fin = false;
    let mut tampon = vec![0u8; 64 * 1024];
    loop {
        let (retenu, rafales, lente, coupee) = c.etat.lock().map_or((0, 0, false, true), |e| {
            (
                e.retenu_jusqu_a.get(sens.rang()).copied().unwrap_or(0),
                e.rafales_jusqu_a,
                e.fermeture_lente,
                e.coupee,
            )
        });
        if coupee {
            return;
        }
        if fin {
            thread::sleep(Duration::from_millis(5));
        } else {
            match de.read(&mut tampon) {
                Ok(0) => fin = true,
                Ok(n) => {
                    let t = us_depuis(depart);
                    let mut due = t.saturating_add(h.entre(LATENCE_US.0, LATENCE_US.1));
                    if t < rafales {
                        due = due.div_ceil(RAFALE_US).saturating_mul(RAFALE_US);
                    }
                    due = due.max(dernier);
                    dernier = due;
                    file.push_back((due, tampon.get(..n).map(<[u8]>::to_vec).unwrap_or_default()));
                }
                Err(e)
                    if e.kind() == io::ErrorKind::WouldBlock
                        || e.kind() == io::ErrorKind::TimedOut => {}
                Err(_) => fin = true,
            }
        }
        let t = us_depuis(depart);
        while let Some((due, _)) = file.front() {
            if *due > t || t < retenu {
                break;
            }
            let Some((_, o)) = file.pop_front() else {
                break;
            };
            if vers.write_all(&o).is_err() {
                c.couper();
                return;
            }
        }
        if fin && file.is_empty() {
            if lente {
                thread::sleep(Duration::from_micros(FERMETURE_LENTE_US));
            }
            let _: io::Result<()> = vers.shutdown(Shutdown::Write);
            return;
        }
    }
}
