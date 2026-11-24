//! Le banc du réseau : `soiree reseau`.
//!
//! Un jeu tourne. Le banc met un faux tuyau entre un téléphone d'essai et
//! l'adresse des téléphones, l'abîme comme un vrai réseau (du délai, des
//! à-coups, un débit bridé, une coupure), et mesure ce que ce téléphone en
//! voit : le délai d'un aller-retour jusqu'au jeu, le rythme et le poids de
//! ce que le jeu lui envoie, et si son lien tient.
//!
//! Le faux tuyau est Toxiproxy (de Shopify), un programme à part : le banc le
//! lance, le règle par son adresse de commande, et l'arrête en partant. Il
//! travaille au-dessus de TCP : il retarde, bride et coupe, mais ne perd pas
//! de paquets. Pour ça, il faut un vrai téléphone (sur iPhone : Réglages >
//! Développeur > Network Link Conditioner).
//!
//! Le téléphone d'essai est une page comme une autre pour le jeu : il se
//! présente avec le billet de la partie, et le jeu le voit arriver. Il ne
//! joue pas. Le banc se lance donc hors d'une vraie partie.

mod load;
mod wire;

use std::collections::VecDeque;
use std::collections::hash_map::RandomState;
use std::hash::BuildHasher as _;
use std::net::{Ipv4Addr, TcpListener, ToSocketAddrs as _};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use futures_util::future::join_all;
use futures_util::{SinkExt as _, StreamExt as _};
use serde_json::json;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

use self::load::Load;
use crate::internet::Stream;

const PROGRAM: &str = "toxiproxy-server";
/// Le nom du tuyau, pour Toxiproxy.
const PIPE: &str = "telephones";
/// Le téléphone d'essai demande un aller-retour à ce rythme.
const ASK_EVERY: Duration = Duration::from_millis(100);
/// Un lien coupé se rouvre après ce temps, comme celui d'une vraie page
/// (`RETRY_MS`, dans `telecommande/src/web.rs`).
const RETRY: Duration = Duration::from_secs(1);
/// Une mesure dure tant, sauf demande contraire.
const SECONDS: u64 = 10;
/// La coupure : rien ne passe pendant ce temps.
const STALL: Duration = Duration::from_secs(5);
/// La coupure longue : plus que ce que la console attend d'un téléphone muet
/// (`SILENCE`, dans `telecommande/src/host.rs`).
const LONG_STALL: Duration = Duration::from_secs(15);
/// La panne : plus que ce que l'étagère attend d'un relais muet (`SILENCE`,
/// dans `internet.rs`).
const OUTAGE: Duration = Duration::from_mins(1);
/// Au plus tant de téléphones d'essai à la fois.
const PHONES_MAX: u64 = 200;
/// La chute : le réseau revient après ce temps.
const DOWN: Duration = Duration::from_secs(2);
/// Après une coupure, la mesure continue ce temps-là : de quoi voir revenir.
const AFTER_CUT: Duration = Duration::from_secs(8);
/// Tenu, le banc dit où il en est à ce rythme, et coupe à celui-là.
const WINDOW: Duration = Duration::from_secs(5);
const CUT_EVERY: Duration = Duration::from_secs(20);
const PATIENCE: Duration = Duration::from_secs(10);
/// À partir de tant de messages par seconde, le jeu envoie un flux : l'écart
/// entre deux messages dit alors s'il arrive régulièrement.
const STEADY: u64 = 5;

const USAGE: &str = "usage : soiree reseau [tiens | lien] [scénario] [options]

Mesure ce qu'un téléphone voit du jeu en cours, à travers un réseau abîmé exprès.
Lance un jeu sur l'étagère (`soiree play`), puis, depuis le même dossier :

  soiree reseau                  joue tous les scénarios, l'un après l'autre
  soiree reseau 4g-mauvaise      n'en joue qu'un
  soiree reseau tiens coupure    le tient jusqu'à Ctrl-C, et donne l'adresse où jouer
                                 soi-même à travers ce réseau-là
  soiree reseau lien             abîme le lien entre l'étagère et le relais, celui
                                 dont dépendent tous les téléphones venus par internet
                                 (il dit ce qu'il lui faut)

Les scénarios : direct, 4g, 4g-mauvaise, coupure (5 secondes), coupure-longue
(15 secondes : le jeu voit partir le joueur), chute, et panne (une minute, sur
demande seulement).

  --ecran            le téléphone d'essai regarde comme l'écran : il reçoit tout ce
                     que le jeu dessine, le plus lourd de ce qui passe
  --telephones <n>   autant de téléphones d'essai à la fois (1 par défaut)
  --secondes <n>     la durée d'une mesure (10 par défaut)
  --adresse <a>      l'adresse des téléphones, celle sous le QR code, pour un jeu
                     lancé sans l'étagère
  --jeux <dossier>   le dossier des jeux de l'étagère (`games/` par défaut)

Le téléphone d'essai compte comme une page de plus pour le jeu : à lancer hors
d'une vraie partie.";

/// Dans quel sens un défaut s'applique.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Way {
    ToPhone,
    ToGame,
}

/// Qui entre dans le tuyau. Toxiproxy nomme ses deux sens d'après lui :
/// « upstream » part de celui qui entre, « downstream » lui revient.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Entering {
    /// Les téléphones : le tuyau est entre eux et le jeu.
    Phones,
    /// L'étagère : le tuyau est entre elle et le relais, et les téléphones
    /// sont de l'autre côté.
    Shelf,
}

impl Way {
    /// Le nom du sens pour Toxiproxy.
    fn stream(self, entering: Entering) -> &'static str {
        match (self, entering) {
            (Self::ToPhone, Entering::Phones) | (Self::ToGame, Entering::Shelf) => "downstream",
            (Self::ToGame, Entering::Phones) | (Self::ToPhone, Entering::Shelf) => "upstream",
        }
    }
}

/// Un défaut du tuyau, tel que Toxiproxy le nomme.
struct Toxic {
    /// Son nom dans le tuyau : deux défauts du même genre peuvent s'y suivre.
    name: &'static str,
    kind: &'static str,
    way: Way,
    attributes: &'static [(&'static str, u32)],
}

impl Toxic {
    fn name(&self, entering: Entering) -> String {
        format!("{}-{}", self.name, self.way.stream(entering))
    }
}

/// Ce qui casse en cours de mesure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cut {
    Never,
    /// Plus rien ne passe pendant ce temps, les connexions restent ouvertes,
    /// puis ça revient.
    Stall(Duration),
    /// Les connexions tombent, et le réseau revient un peu après.
    Fall,
}

/// Quand un scénario se joue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Played {
    /// Avec les autres, quand l'hôte n'en nomme aucun.
    WithTheOthers,
    /// Seulement quand l'hôte le nomme : il est long.
    WhenNamed,
}

struct Scenario {
    name: &'static str,
    tells: &'static str,
    toxics: &'static [Toxic],
    cut: Cut,
    played: Played,
}

const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "direct",
        tells: "rien d'abîmé : la référence",
        toxics: &[],
        cut: Cut::Never,
        played: Played::WithTheOthers,
    },
    Scenario {
        name: "4g",
        tells: "une 4G correcte : 40 ms de plus dans chaque sens, à 15 ms près",
        toxics: &[
            Toxic {
                name: "delai",
                kind: "latency",
                way: Way::ToPhone,
                attributes: &[("latency", 40), ("jitter", 15)],
            },
            Toxic {
                name: "delai",
                kind: "latency",
                way: Way::ToGame,
                attributes: &[("latency", 40), ("jitter", 15)],
            },
        ],
        cut: Cut::Never,
        played: Played::WithTheOthers,
    },
    Scenario {
        name: "4g-mauvaise",
        tells: "une 4G faible : 150 ms de plus dans chaque sens, à 100 ms près, et un débit \
                bridé (100 Ko par seconde vers le téléphone, 30 vers le jeu)",
        toxics: &[
            Toxic {
                name: "delai",
                kind: "latency",
                way: Way::ToPhone,
                attributes: &[("latency", 150), ("jitter", 100)],
            },
            Toxic {
                name: "delai",
                kind: "latency",
                way: Way::ToGame,
                attributes: &[("latency", 150), ("jitter", 100)],
            },
            Toxic {
                name: "debit",
                kind: "bandwidth",
                way: Way::ToPhone,
                attributes: &[("rate", 100)],
            },
            Toxic {
                name: "debit",
                kind: "bandwidth",
                way: Way::ToGame,
                attributes: &[("rate", 30)],
            },
        ],
        cut: Cut::Never,
        played: Played::WithTheOthers,
    },
    Scenario {
        name: "coupure",
        tells: "plus rien ne passe pendant 5 secondes (un tunnel, un ascenseur), puis ça revient",
        toxics: &[],
        cut: Cut::Stall(STALL),
        played: Played::WithTheOthers,
    },
    Scenario {
        name: "coupure-longue",
        tells: "plus rien ne passe pendant 15 secondes : la console voit le téléphone se \
                taire, ferme son lien, et le jeu voit partir le joueur",
        toxics: &[],
        cut: Cut::Stall(LONG_STALL),
        played: Played::WithTheOthers,
    },
    Scenario {
        name: "panne",
        tells: "plus rien ne passe pendant une minute (un ordinateur qui change de réseau, \
                une box qui redémarre)",
        toxics: &[],
        cut: Cut::Stall(OUTAGE),
        played: Played::WhenNamed,
    },
    Scenario {
        name: "chute",
        tells: "toutes les connexions tombent d'un coup (un relais qui redémarre, un Wi-Fi qui \
                saute), et le réseau revient 2 secondes après",
        toxics: &[],
        cut: Cut::Fall,
        played: Played::WithTheOthers,
    },
];

/// L'adresse des téléphones : où elle mène, et le billet qu'elle porte.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Target {
    secure: bool,
    host: String,
    port: u16,
    ticket: Option<String>,
}

impl Target {
    /// Lit `https://nom:port/t?c=billet`, l'adresse écrite sous le QR code.
    fn read(address: &str) -> Option<Self> {
        let (secure, rest) = match address.trim().split_once("://")? {
            ("https", rest) => (true, rest),
            ("http", rest) => (false, rest),
            _ => return None,
        };
        let (at, path) = rest.split_once('/').unwrap_or((rest, ""));
        let (host, port) = match at.rsplit_once(':') {
            Some((host, port)) => (host, port.parse().ok()?),
            None => (at, if secure { 443 } else { 80 }),
        };
        let ticket = path
            .split_once('?')
            .and_then(|(_, query)| query.split('&').find_map(|pair| pair.strip_prefix("c=")))
            .map(str::to_owned);
        (!host.is_empty()).then(|| Self {
            secure,
            host: host.to_owned(),
            port,
            ticket,
        })
    }

    /// Cette adresse mène-t-elle à cet ordinateur ? Alors un vrai téléphone
    /// du Wi-Fi peut passer par le tuyau, lui aussi.
    fn is_here(&self) -> bool {
        (self.host.as_str(), self.port)
            .to_socket_addrs()
            .is_ok_and(|mut found| {
                found.any(|address| TcpListener::bind((address.ip(), 0)).is_ok())
            })
    }

    /// Le nom à donner au serveur : sans le port quand c'est celui d'usage.
    fn authority(&self) -> String {
        if self.port == if self.secure { 443 } else { 80 } {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    /// La page qu'un téléphone ouvre, ou l'écran.
    fn page(&self, role: &str) -> String {
        match (role, &self.ticket) {
            ("screen", _) => "/".to_owned(),
            (_, Some(ticket)) => format!("/t?c={ticket}"),
            (_, None) => "/t".to_owned(),
        }
    }

    /// La même adresse, par un autre port : celui du tuyau.
    fn through(&self, port: u16) -> String {
        let scheme = if self.secure { "https" } else { "http" };
        let ticket = self
            .ticket
            .as_ref()
            .map_or_else(String::new, |ticket| format!("?c={ticket}"));
        format!("{scheme}://{}:{port}/t{ticket}", self.host)
    }
}

/// Qui peut entrer dans le tuyau.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Open {
    /// Le téléphone d'essai seulement.
    ToThisComputer,
    /// Les téléphones du Wi-Fi aussi.
    ToTheNetwork,
}

/// Ouvre une connexion vers l'adresse des téléphones, par le port donné sur
/// cet ordinateur (celui du tuyau) : chiffrée si l'adresse l'est, au nom
/// qu'elle porte.
async fn open(target: &Target, through: u16) -> Result<Stream, String> {
    let wrong = |error: &dyn std::fmt::Display| {
        format!("la connexion vers le jeu ne s'ouvre pas ({error})")
    };
    let opening = async {
        let tcp = TcpStream::connect((Ipv4Addr::LOCALHOST, through))
            .await
            .map_err(|error| wrong(&error))?;
        let _: std::io::Result<()> = tcp.set_nodelay(true);
        let stream: Stream = if target.secure {
            Box::new(
                crate::internet::secure(tcp, &target.host)
                    .await
                    .map_err(|error| wrong(&error))?,
            )
        } else {
            Box::new(tcp)
        };
        Ok(stream)
    };
    tokio::time::timeout(PATIENCE, opening)
        .await
        .unwrap_or_else(|_| {
            Err("la connexion vers le jeu ne s'ouvre pas (pas de réponse)".to_owned())
        })
}

/// Un port libre sur cet ordinateur, à l'instant.
fn free_port() -> Result<u16, String> {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .map_err(|error| format!("aucun port libre sur cet ordinateur ({error})"))
}

/// Le faux tuyau : Toxiproxy, lancé pour la durée du banc.
struct Pipe {
    child: Child,
    control: Control,
    /// Le port où entrer dans le tuyau.
    port: u16,
}

impl Drop for Pipe {
    fn drop(&mut self) {
        let _: std::io::Result<()> = self.child.kill();
        let _: std::io::Result<std::process::ExitStatus> = self.child.wait();
    }
}

impl Pipe {
    fn open(target: &Target, open: Open, entering: Entering) -> Result<Self, String> {
        let api = free_port()?;
        let port = free_port()?;
        let child = Command::new(PROGRAM)
            .args(["-host", "127.0.0.1", "-port", &api.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                format!(
                    "le banc du réseau a besoin de Toxiproxy, et `{PROGRAM}` ne se lance pas \
                     ({error}). Sur un Mac : `brew install toxiproxy`."
                )
            })?;
        let pipe = Self {
            child,
            control: Control {
                api: format!("http://127.0.0.1:{api}"),
                entering,
            },
            port,
        };
        let since = Instant::now();
        while pipe.control.call("GET", "/version", None).is_err() {
            if since.elapsed() > PATIENCE {
                return Err(format!("`{PROGRAM}` ne répond pas."));
            }
            thread::sleep(Duration::from_millis(50));
        }
        let listen = match open {
            Open::ToThisComputer => "127.0.0.1",
            Open::ToTheNetwork => "0.0.0.0",
        };
        pipe.control.call(
            "POST",
            "/proxies",
            Some(&json!({
                "name": PIPE,
                "listen": format!("{listen}:{port}"),
                "upstream": format!("{}:{}", target.host, target.port),
                "enabled": true,
            })),
        )?;
        Ok(pipe)
    }
}

/// De quoi régler le tuyau, d'où qu'on soit.
#[derive(Clone)]
struct Control {
    api: String,
    entering: Entering,
}

impl Control {
    fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<(), String> {
        let url = format!("{}{path}", self.api);
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(PATIENCE))
            .build()
            .into();
        let sent = match (method, body) {
            ("POST", Some(body)) => agent
                .post(&url)
                .header("content-type", "application/json")
                .send(body.to_string())
                .map(drop),
            ("DELETE", _) => agent.delete(&url).call().map(drop),
            (_, _) => agent.get(&url).call().map(drop),
        };
        sent.map_err(|error| format!("Toxiproxy refuse ({method} {path}) : {error}"))
    }

    fn spoil(&self, toxic: &Toxic) -> Result<(), String> {
        let attributes: serde_json::Map<String, serde_json::Value> = toxic
            .attributes
            .iter()
            .map(|(name, value)| ((*name).to_owned(), json!(value)))
            .collect();
        self.call(
            "POST",
            &format!("/proxies/{PIPE}/toxics"),
            Some(&json!({
                "name": toxic.name(self.entering),
                "type": toxic.kind,
                "stream": toxic.way.stream(self.entering),
                "toxicity": 1.0,
                "attributes": attributes,
            })),
        )
    }

    fn mend(&self, toxic: &Toxic) -> Result<(), String> {
        self.call(
            "DELETE",
            &format!("/proxies/{PIPE}/toxics/{}", toxic.name(self.entering)),
            None,
        )
    }

    /// Casse le tuyau comme le scénario le dit, attend, et le répare. Rend
    /// le temps que la panne a duré.
    fn cut(&self, cut: Cut) -> Result<Duration, String> {
        let since = Instant::now();
        match cut {
            Cut::Never => Ok(Duration::ZERO),
            Cut::Stall(length) => {
                // Un délai plus long que la coupure : tout est retenu, rien
                // n'est perdu, et ce qui attendait repart quand on l'enlève.
                // C'est ce que fait TCP sous un tunnel.
                let holes = [Way::ToPhone, Way::ToGame].map(|way| Toxic {
                    name: "coupure",
                    kind: "latency",
                    way,
                    attributes: &[("latency", 600_000)],
                });
                holes.iter().try_for_each(|hole| self.spoil(hole))?;
                thread::sleep(length);
                // Le réseau revient ici : Toxiproxy peut mettre du temps à
                // rendre la main, alors que les octets repassent déjà.
                let lasted = since.elapsed();
                holes.iter().try_for_each(|hole| self.mend(hole))?;
                Ok(lasted)
            }
            Cut::Fall => {
                let switch = |enabled: &str| {
                    let body: serde_json::Value =
                        serde_json::from_str(&format!("{{\"enabled\":{enabled}}}"))
                            .unwrap_or_default();
                    self.call("POST", &format!("/proxies/{PIPE}"), Some(&body))
                };
                switch("false")?;
                thread::sleep(DOWN);
                let lasted = since.elapsed();
                switch("true")?;
                Ok(lasted)
            }
        }
    }
}

/// Ce que le téléphone d'essai a vu.
#[derive(Default)]
struct Tally {
    /// Le temps de chaque aller-retour revenu.
    delays: Vec<Duration>,
    /// Les allers-retours demandés.
    asked: u64,
    /// L'écart entre deux messages du jeu qui se suivent.
    gaps: Vec<Duration>,
    messages: u64,
    bytes: u64,
    heaviest: usize,
    /// Chaque fois que le lien est tombé : le temps passé sans lui.
    away: Vec<Duration>,
    /// Le lien était tombé à la fin de la mesure.
    gone: bool,
    /// Ce que plusieurs téléphones ont vu, mis ensemble : combien ils
    /// étaient, et combien ont perdu leur lien au moins une fois.
    phones: u64,
    dropped: u64,
}

/// Un poids, lisible : « 654 Ko », « 2,9 Mo ».
fn weigh(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        let tenths = bytes.checked_div(100_000).unwrap_or(0);
        format!(
            "{},{} Mo",
            tenths.checked_div(10).unwrap_or(0),
            tenths.checked_rem(10).unwrap_or(0)
        )
    } else {
        format!("{} Ko", bytes.checked_div(1000).unwrap_or(0))
    }
}

/// Le chargement de la page, vu par chaque téléphone d'essai.
fn tell_loads(loads: &[Load]) -> Option<String> {
    let first = loads.first()?;
    let times: Vec<Duration> = loads.iter().map(|load| load.took).collect();
    let mut sorted = times.clone();
    sorted.sort_unstable();
    let took = match (rank(&sorted, 50), rank(&sorted, 100)) {
        (Some(median), Some(worst)) if loads.len() > 1 => {
            format!("{} (le plus lent : {})", say(median), say(worst))
        }
        (Some(median), _) => say(median),
        (None, _) => return None,
    };
    let heaviest = first.heaviest().map_or_else(String::new, |file| {
        let packed = file.encoding.as_ref().map_or_else(
            || "non compressé".to_owned(),
            |by| format!("compressé ({by})"),
        );
        format!(
            " · le plus lourd : {}, {}, {packed}",
            file.path.split('?').next().unwrap_or(&file.path),
            weigh(file.bytes)
        )
    });
    Some(format!(
        "{} en {took} · {} fichiers{heaviest}",
        weigh(first.bytes()),
        first.files.len()
    ))
}

/// Le rang d'un centième dans une liste triée : la médiane est le 50e.
fn rank(sorted: &[Duration], hundredth: usize) -> Option<Duration> {
    let last = sorted.len().checked_sub(1)?;
    let index = last.saturating_mul(hundredth).checked_div(100)?;
    sorted.get(index).copied()
}

/// Une durée, lisible : « 8,3 ms », « 142 ms », « 5,1 s ».
fn say(duration: Duration) -> String {
    let micros = duration.as_micros();
    let (whole, tenth, unit) = if micros >= 1_000_000 {
        (
            micros.checked_div(1_000_000),
            micros
                .checked_rem(1_000_000)
                .and_then(|rest| rest.checked_div(100_000)),
            "s",
        )
    } else {
        (
            micros.checked_div(1000),
            micros
                .checked_rem(1000)
                .and_then(|rest| rest.checked_div(100)),
            "ms",
        )
    };
    let (whole, tenth) = (whole.unwrap_or(0), tenth.unwrap_or(0));
    if whole >= 100 {
        format!("{whole} {unit}")
    } else {
        format!("{whole},{tenth} {unit}")
    }
}

/// Médiane, 95e centième et pire d'une série.
fn spread(series: &[Duration]) -> Option<String> {
    let mut sorted = series.to_vec();
    sorted.sort_unstable();
    let (median, most, worst) = (rank(&sorted, 50)?, rank(&sorted, 95)?, rank(&sorted, 100)?);
    Some(format!(
        "médiane {} · 95 % sous {} · pire {}",
        say(median),
        say(most),
        say(worst)
    ))
}

impl Tally {
    /// Ce que plusieurs téléphones ont vu, mis ensemble.
    fn together(all: Vec<Self>) -> Self {
        let mut whole = Self::default();
        for one in all {
            whole.phones = whole.phones.saturating_add(1);
            if one.gone || !one.away.is_empty() {
                whole.dropped = whole.dropped.saturating_add(1);
            }
            whole.delays.extend(one.delays);
            whole.asked = whole.asked.saturating_add(one.asked);
            whole.gaps.extend(one.gaps);
            whole.messages = whole.messages.saturating_add(one.messages);
            whole.bytes = whole.bytes.saturating_add(one.bytes);
            whole.heaviest = whole.heaviest.max(one.heaviest);
            whole.away.extend(one.away);
            whole.gone = whole.gone || one.gone;
        }
        whole
    }

    /// Ce qui est vu, en quelques lignes.
    fn report(&self, lasted: Duration) -> String {
        let unanswered = self
            .asked
            .saturating_sub(u64::try_from(self.delays.len()).unwrap_or(u64::MAX));
        let delay = spread(&self.delays).map_or_else(
            || "aucun aller-retour n'est revenu".to_owned(),
            |spread| {
                format!(
                    "{spread}  ({} mesures, {unanswered} sans réponse)",
                    self.delays.len()
                )
            },
        );
        // Le rythme et le poids se disent par téléphone.
        let seconds = lasted.as_secs().max(1).saturating_mul(self.phones.max(1));
        let stream = if self.messages == 0 {
            "aucun message : rien ne bouge pour ce téléphone (lance une partie pour mesurer \
             le rythme du jeu)"
                .to_owned()
        } else {
            let each = self.bytes.checked_div(self.messages).unwrap_or(0);
            let per_second = self.messages.checked_div(seconds).unwrap_or(0);
            let weight = self
                .bytes
                .checked_div(seconds)
                .and_then(|bytes| bytes.checked_div(1000))
                .unwrap_or(0);
            if per_second == 0 {
                format!(
                    "{} en tout, {each} octets chacun : le jeu envoie peu à ce téléphone",
                    self.messages
                )
            } else {
                format!(
                    "{per_second} par seconde · {each} octets chacun (le plus lourd : {}) · \
                     {weight} Ko par seconde",
                    self.heaviest
                )
            }
        };
        // Le rythme n'a de sens que pour un flux : quelques messages par seconde au moins.
        let steady = self.messages.checked_div(seconds).unwrap_or(0) >= STEADY;
        let gaps = spread(&self.gaps)
            .filter(|_| steady)
            .unwrap_or_else(|| "trop peu de messages pour parler de rythme".to_owned());
        let link = match (self.away.as_slice(), self.gone) {
            ([], false) => "tenu d'un bout à l'autre".to_owned(),
            (_, true) => "tombé, et pas revenu avant la fin de la mesure".to_owned(),
            (away, false) => {
                let longest = away.iter().max().copied().unwrap_or_default();
                format!("tombé {} fois, revenu après {}", away.len(), say(longest))
            }
        };
        let link = if self.phones > 1 {
            format!(
                "{link} ({} téléphones sur {} l'ont perdu)",
                self.dropped, self.phones
            )
        } else {
            link
        };
        format!(
            "   aller-retour jusqu'au jeu    {delay}\n   \
             messages du jeu              {stream}\n   \
             écart entre deux messages    {gaps}\n   \
             lien                         {link}"
        )
    }

    /// La même chose en une ligne, pour un banc tenu.
    fn line(&self) -> String {
        let mut delays = self.delays.clone();
        delays.sort_unstable();
        let mut gaps = self.gaps.clone();
        gaps.sort_unstable();
        let show = |sorted: &[Duration], hundredth| {
            rank(sorted, hundredth).map_or_else(|| "—".to_owned(), say)
        };
        format!(
            "   aller-retour {} (pire {}) · {} messages · écart {} (pire {})",
            show(&delays, 50),
            show(&delays, 100),
            self.messages,
            show(&gaps, 50),
            show(&gaps, 100),
        )
    }
}

type Socket = WebSocketStream<Stream>;

/// Comment une session du téléphone d'essai a fini.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ended {
    Over,
    Lost,
}

/// Le téléphone d'essai.
struct Probe<'run> {
    target: &'run Target,
    /// Le port du tuyau, sur cet ordinateur.
    through: u16,
    role: &'static str,
    id: u64,
    /// Tenu : il dit où il en est à ce rythme.
    tells_every: Option<Duration>,
}

impl Probe<'_> {
    /// Ouvre le lien d'une page avec le jeu, à travers le tuyau.
    async fn link(&self) -> Result<Socket, String> {
        let wrong =
            |error: &dyn std::fmt::Display| format!("le lien avec le jeu ne s'ouvre pas ({error})");
        let opening = async {
            let stream = open(self.target, self.through).await?;
            let ticket = self
                .target
                .ticket
                .as_ref()
                .map_or_else(String::new, |ticket| format!("&c={ticket}"));
            let url = format!(
                "ws://{}:{}/ws?id={}&role={}{ticket}",
                self.target.host, self.target.port, self.id, self.role
            );
            tokio_tungstenite::client_async(url, stream)
                .await
                .map(|(socket, _)| socket)
                .map_err(|error| wrong(&error))
        };
        tokio::time::timeout(PATIENCE, opening)
            .await
            .unwrap_or_else(|_| {
                Err("le lien avec le jeu ne s'ouvre pas (pas de réponse)".to_owned())
            })
    }

    /// Fait ce que fait un téléphone qui scanne le QR code : charge la page
    /// (si `page` en donne une), puis regarde le jeu pendant `length`.
    async fn visit(
        &self,
        page: Option<&str>,
        length: Duration,
    ) -> Result<(Option<Load>, Tally), String> {
        let loaded = match page {
            Some(page) => Some(load::load(self.target, self.through, page).await?),
            None => None,
        };
        Ok((loaded, self.watch(Some(length)).await?))
    }

    /// Regarde le jeu pendant `length` (ou jusqu'à Ctrl-C), en rouvrant le
    /// lien quand il tombe, comme une page.
    async fn watch(&self, length: Option<Duration>) -> Result<Tally, String> {
        let far = Duration::from_hours(8760);
        let start = tokio::time::Instant::now();
        let end = start.checked_add(length.unwrap_or(far)).unwrap_or(start);
        // Le premier lien doit s'ouvrir : sinon, il n'y a rien à mesurer.
        let mut socket = Some(self.link().await.map_err(|error| {
            format!(
                "{error}\nLe jeu tourne-t-il encore ? Le billet de l'adresse est-il celui de la \
                 partie en cours ?"
            )
        })?);
        let mut tally = Tally::default();
        let mut lost: Option<Instant> = None;
        while tokio::time::Instant::now() < end {
            let Some(open) = socket.take() else {
                tokio::time::sleep(RETRY).await;
                socket = self.link().await.ok();
                continue;
            };
            if let Some(since) = lost.take() {
                tally.away.push(since.elapsed());
                if self.tells_every.is_some() {
                    println!("   le lien est revenu après {}", say(since.elapsed()));
                }
            }
            if self.session(open, &mut tally, end).await == Ended::Lost {
                lost = Some(Instant::now());
                if self.tells_every.is_some() {
                    println!("   le lien est tombé");
                }
            }
        }
        tally.gone = lost.is_some();
        Ok(tally)
    }

    async fn session(&self, socket: Socket, tally: &mut Tally, end: tokio::time::Instant) -> Ended {
        let (mut sink, mut stream) = socket.split();
        let mut ask = tokio::time::interval(ASK_EVERY);
        let far = Duration::from_hours(8760);
        let mut tell = tokio::time::interval(self.tells_every.unwrap_or(far));
        // Le premier battement d'un intervalle est immédiat : rien à dire encore.
        let _: tokio::time::Instant = tell.tick().await;
        let mut pending: VecDeque<(u64, Instant)> = VecDeque::new();
        let mut number: u64 = 0;
        let mut last: Option<Instant> = None;
        loop {
            tokio::select! {
                () = tokio::time::sleep_until(end) => return Ended::Over,
                _ = tell.tick() => {
                    println!("{}", tally.line());
                    *tally = Tally::default();
                }
                _ = ask.tick() => {
                    number = number.wrapping_add(1);
                    pending.push_back((number, Instant::now()));
                    tally.asked = tally.asked.saturating_add(1);
                    let asked = Message::Ping(number.to_be_bytes().to_vec().into());
                    if sink.send(asked).await.is_err() {
                        return Ended::Lost;
                    }
                }
                incoming = stream.next() => match incoming {
                    Some(Ok(Message::Pong(payload))) => {
                        let answered = <[u8; 8]>::try_from(payload.as_ref())
                            .ok()
                            .map(u64::from_be_bytes);
                        while let Some((asked, at)) = pending.pop_front() {
                            if Some(asked) == answered {
                                tally.delays.push(at.elapsed());
                                break;
                            }
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        let now = Instant::now();
                        if let Some(before) = last.replace(now) {
                            tally.gaps.push(now.duration_since(before));
                        }
                        tally.messages = tally.messages.saturating_add(1);
                        tally.bytes = tally
                            .bytes
                            .saturating_add(u64::try_from(text.len()).unwrap_or(u64::MAX));
                        tally.heaviest = tally.heaviest.max(text.len());
                    }
                    Some(Ok(Message::Binary(_) | Message::Ping(_) | Message::Frame(_))) => {}
                    Some(Ok(Message::Close(_)) | Err(_)) | None => return Ended::Lost,
                },
            }
        }
    }
}

/// Ce que l'hôte demande au banc.
struct Asked {
    held: bool,
    /// C'est le lien de l'étagère avec le relais qu'on abîme (`wire.rs`).
    wire: bool,
    scenario: Option<String>,
    role: &'static str,
    seconds: u64,
    phones: u64,
    address: Option<String>,
    games: PathBuf,
    /// Pour le lien de l'étagère : le programme du relais, et le port du jeu.
    relay: Option<PathBuf>,
    game: Option<u16>,
}

impl Asked {
    fn read(args: &[String]) -> Result<Self, String> {
        let mut asked = Self {
            held: false,
            wire: false,
            scenario: None,
            role: "remote",
            seconds: SECONDS,
            phones: 1,
            address: None,
            games: PathBuf::from("games"),
            relay: None,
            game: None,
        };
        let mut words = args.iter().map(String::as_str);
        while let Some(word) = words.next() {
            match word {
                "tiens" => asked.held = true,
                "lien" => asked.wire = true,
                "--telephones" => {
                    asked.phones = words
                        .next()
                        .and_then(|count| count.parse().ok())
                        .filter(|count| (1..=PHONES_MAX).contains(count))
                        .ok_or(USAGE)?;
                }
                "--relais" => asked.relay = Some(PathBuf::from(words.next().ok_or(USAGE)?)),
                "--jeu" => {
                    asked.game = Some(
                        words
                            .next()
                            .and_then(|port| port.parse().ok())
                            .ok_or(USAGE)?,
                    );
                }
                "--ecran" => asked.role = "screen",
                "--secondes" => {
                    asked.seconds = words
                        .next()
                        .and_then(|count| count.parse().ok())
                        .filter(|count| *count > 0)
                        .ok_or(USAGE)?;
                }
                "--adresse" => asked.address = Some(words.next().ok_or(USAGE)?.to_owned()),
                "--jeux" => asked.games = PathBuf::from(words.next().ok_or(USAGE)?),
                name if !name.starts_with('-') && asked.scenario.is_none() => {
                    asked.scenario = Some(name.to_owned());
                }
                _ => return Err(USAGE.to_owned()),
            }
        }
        Ok(asked)
    }
}

fn scenario(name: &str) -> Result<&'static Scenario, String> {
    SCENARIOS
        .iter()
        .find(|scenario| scenario.name == name)
        .ok_or_else(|| {
            let known: Vec<&str> = SCENARIOS.iter().map(|scenario| scenario.name).collect();
            format!("scénario inconnu : {name}. Il y a : {}.", known.join(", "))
        })
}

/// Les scénarios à jouer : celui que l'hôte nomme, sinon tous ceux qui se
/// jouent ensemble.
fn chosen(asked: &Asked) -> Result<Vec<&'static Scenario>, String> {
    match &asked.scenario {
        Some(name) => Ok(vec![scenario(name)?]),
        None => Ok(SCENARIOS
            .iter()
            .filter(|scenario| scenario.played == Played::WithTheOthers)
            .collect()),
    }
}

/// L'identité du téléphone d'essai : neuve à chaque banc.
fn identity() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    // Une page tire la sienne en JavaScript : un entier qu'il sait écrire.
    RandomState::new().hash_one(nanos) >> 12_u32
}

/// `soiree reseau` : mesure ce qu'un téléphone voit du jeu en cours, à travers
/// un réseau abîmé exprès.
pub fn run(args: &[String]) -> Result<(), String> {
    if args.iter().any(|word| word == "--help" || word == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let asked = Asked::read(args)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    if asked.wire {
        return wire::run(&runtime, &asked);
    }
    let address = match &asked.address {
        Some(address) => address.clone(),
        None => crate::shelf::phones_address(&asked.games)?,
    };
    let target = Target::read(&address).ok_or_else(|| {
        format!("ce n'est pas l'adresse des téléphones (`https://nom:port/t?c=billet`) : {address}")
    })?;
    println!("soiree {} : le banc du réseau", crate::VERSION);
    println!("Les téléphones vont à {address}\n");
    if asked.held {
        let name = asked.scenario.as_deref().unwrap_or("4g");
        return hold(&runtime, &target, scenario(name)?, asked.role);
    }
    let scenarios = chosen(&asked)?;
    let pipe = Pipe::open(&target, Open::ToThisComputer, Entering::Phones)?;
    let setup = Setup {
        pipe: &pipe,
        target: &target,
        through: pipe.port,
        wire: None,
    };
    for scenario in scenarios {
        measure(&runtime, &setup, scenario, &asked)?;
    }
    Ok(())
}

/// Où passent les téléphones d'essai, et ce que le banc suit en plus.
struct Setup<'run> {
    /// Le tuyau qu'on abîme.
    pipe: &'run Pipe,
    /// L'adresse des téléphones.
    target: &'run Target,
    /// Le port, sur cet ordinateur, par où les téléphones d'essai y vont.
    through: u16,
    /// Le lien de l'étagère avec le relais, quand c'est lui qu'on abîme.
    wire: Option<Arc<AtomicBool>>,
}

/// Suit le lien de l'étagère jusqu'à `done` : chaque fois qu'il change, à
/// quel moment, et ce qu'il devient.
fn follow(wire: &AtomicBool, done: &AtomicBool, since: Instant) -> Vec<(Duration, bool)> {
    let mut seen = Vec::new();
    let mut linked = true;
    while !done.load(Ordering::Relaxed) {
        let now = wire.load(Ordering::Relaxed);
        if now != linked {
            linked = now;
            seen.push((since.elapsed(), now));
        }
        thread::sleep(Duration::from_millis(20));
    }
    seen
}

/// Ce que le lien de l'étagère est devenu pendant le scénario.
fn tell_wire(seen: &[(Duration, bool)], cut: Option<(Duration, Duration)>) -> String {
    let outage = cut.map_or_else(String::new, |(from, to)| {
        format!("panne de {} à {} · ", say(from), say(to))
    });
    if seen.is_empty() {
        return format!("{outage}tenu d'un bout à l'autre");
    }
    let turns: Vec<String> = seen
        .iter()
        .map(|(at, linked)| {
            let what = if *linked { "relié" } else { "perdu" };
            format!("{what} à {}", say(*at))
        })
        .collect();
    format!("{outage}{}", turns.join(" · "))
}

/// Joue un scénario et dit ce que les téléphones d'essai ont vu.
fn measure(
    runtime: &tokio::runtime::Runtime,
    setup: &Setup<'_>,
    scenario: &Scenario,
    asked: &Asked,
) -> Result<(), String> {
    println!("── {} : {}", scenario.name, scenario.tells);
    let control = &setup.pipe.control;
    scenario
        .toxics
        .iter()
        .try_for_each(|toxic| control.spoil(toxic))?;
    let before_cut = Duration::from_secs(asked.seconds.checked_div(2).unwrap_or(1).max(1));
    let length = match scenario.cut {
        Cut::Never => Duration::from_secs(asked.seconds),
        Cut::Stall(stall) => before_cut.saturating_add(stall).saturating_add(AFTER_CUT),
        Cut::Fall => before_cut.saturating_add(DOWN).saturating_add(AFTER_CUT),
    };
    // La page se charge avant de jouer : mesuré quand rien ne casse en route.
    let page = (scenario.cut == Cut::Never).then(|| setup.target.page(asked.role));
    let probes: Vec<Probe<'_>> = (0..asked.phones)
        .map(|_| Probe {
            target: setup.target,
            through: setup.through,
            role: asked.role,
            id: identity(),
            tells_every: None,
        })
        .collect();
    let since = Instant::now();
    let done = AtomicBool::new(false);
    let cut = scenario.cut;
    let (visits, cut_done, wire) = thread::scope(|scope| {
        let cutter = scope.spawn(|| {
            if cut == Cut::Never {
                return Ok(None);
            }
            thread::sleep(before_cut);
            let from = since.elapsed();
            let lasted = control.cut(cut)?;
            Ok(Some((from, from.saturating_add(lasted))))
        });
        let follower = setup
            .wire
            .as_ref()
            .map(|wire| scope.spawn(|| follow(wire, &done, since)));
        let visits = runtime.block_on(join_all(
            probes
                .iter()
                .map(|probe| probe.visit(page.as_deref(), length)),
        ));
        done.store(true, Ordering::Relaxed);
        let cut_done: Result<Option<(Duration, Duration)>, String> = cutter
            .join()
            .unwrap_or_else(|_| Err("la coupure a échoué".to_owned()));
        let wire = follower.map(|follower| follower.join().unwrap_or_default());
        (visits, cut_done, wire)
    });
    scenario
        .toxics
        .iter()
        .try_for_each(|toxic| control.mend(toxic))?;
    let cut_done = cut_done?;
    let mut loads = Vec::new();
    let mut tallies = Vec::new();
    for visit in visits {
        let (loaded, tally) = visit?;
        loads.extend(loaded);
        tallies.push(tally);
    }
    if let Some(told) = tell_loads(&loads) {
        println!("   chargement de la page        {told}");
    }
    println!("{}", Tally::together(tallies).report(length));
    if let Some(seen) = wire {
        println!(
            "   lien de l'étagère            {}",
            tell_wire(&seen, cut_done)
        );
    }
    println!();
    Ok(())
}

/// Tient un scénario jusqu'à Ctrl-C : l'hôte joue lui-même à travers le tuyau.
fn hold(
    runtime: &tokio::runtime::Runtime,
    target: &Target,
    scenario: &'static Scenario,
    role: &'static str,
) -> Result<(), String> {
    let here = target.is_here();
    let open = if here {
        Open::ToTheNetwork
    } else {
        Open::ToThisComputer
    };
    let pipe = Pipe::open(target, open, Entering::Phones)?;
    scenario
        .toxics
        .iter()
        .try_for_each(|toxic| pipe.control.spoil(toxic))?;
    println!("── {} : {}", scenario.name, scenario.tells);
    if here {
        println!(
            "Pour jouer à travers ce réseau-là, depuis un téléphone du Wi-Fi ou un onglet de \
             cet ordinateur :\n   {}",
            target.through(pipe.port)
        );
    } else {
        println!(
            "L'adresse des téléphones mène à un serveur, pas à cet ordinateur : un vrai \
             téléphone ne peut pas passer par ce tuyau. Le téléphone d'essai, lui, y passe.\n\
             Pour abîmer le réseau d'un vrai iPhone : Réglages > Développeur > Network Link \
             Conditioner."
        );
    }
    if scenario.cut != Cut::Never {
        println!(
            "La coupure revient toutes les {} secondes.",
            CUT_EVERY.as_secs()
        );
    }
    println!("Ctrl-C arrête.\n");
    let control = pipe.control.clone();
    let cut = scenario.cut;
    // Ce fil s'arrête avec le programme.
    let _: thread::JoinHandle<()> = thread::spawn(move || {
        if cut == Cut::Never {
            return;
        }
        loop {
            thread::sleep(CUT_EVERY);
            println!("   coupure");
            if control.cut(cut).is_err() {
                break;
            }
            println!("   le réseau est revenu");
        }
    });
    let probe = Probe {
        target,
        through: pipe.port,
        role,
        id: identity(),
        tells_every: Some(WINDOW),
    };
    runtime.block_on(probe.watch(None)).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_phones_address_is_read_with_its_ticket() {
        assert_eq!(
            Target::read("https://jeu.exemple.com:8089/t?c=k7f2"),
            Some(Target {
                secure: true,
                host: "jeu.exemple.com".to_owned(),
                port: 8089,
                ticket: Some("k7f2".to_owned()),
            })
        );
        assert_eq!(
            Target::read("https://relais.exemple.com/t?c=a6x7\n"),
            Some(Target {
                secure: true,
                host: "relais.exemple.com".to_owned(),
                port: 443,
                ticket: Some("a6x7".to_owned()),
            })
        );
        assert_eq!(
            Target::read("http://192.168.1.20:8080/t"),
            Some(Target {
                secure: false,
                host: "192.168.1.20".to_owned(),
                port: 8080,
                ticket: None,
            })
        );
        assert_eq!(Target::read("jeu.exemple.com"), None);
    }

    #[test]
    fn the_same_address_goes_through_the_pipe() {
        let target = Target::read("https://jeu.exemple.com:8089/t?c=k7f2").unwrap();
        assert_eq!(
            target.through(9000),
            "https://jeu.exemple.com:9000/t?c=k7f2"
        );
    }

    #[test]
    fn a_series_is_told_by_its_median_and_its_worst() {
        let series: Vec<Duration> = (1..=100).map(Duration::from_millis).collect();
        assert_eq!(
            spread(&series).unwrap(),
            "médiane 50,0 ms · 95 % sous 95,0 ms · pire 100 ms"
        );
        assert_eq!(spread(&[]), None);
        assert_eq!(say(Duration::from_micros(8300)), "8,3 ms");
        assert_eq!(say(Duration::from_millis(5100)), "5,1 s");
    }

    #[test]
    fn every_scenario_has_a_name_to_ask_it_by() {
        for known in SCENARIOS {
            assert_eq!(scenario(known.name).unwrap().name, known.name);
        }
        assert!(scenario("fibre").is_err());
    }
}
