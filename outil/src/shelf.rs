//! `soiree play` : l'étagère. Pour l'hôte, le soir de la soirée.
//!
//! Une page, sur l'ordinateur de l'hôte, montre la jaquette de chaque jeu.
//! Un clic lance le jeu ; son menu (Échap maintenu) y ramène.
//!
//! - **Au démarrage**, l'étagère récupère les jeux de l'organisation, vérifie
//!   chacun (`soiree check`) et range à part ce qu'il faut pour le lancer :
//!   son programme et ses pages, déjà compilés. Un jeu déjà rangé et qui n'a
//!   pas changé n'est pas revérifié.
//! - **Pendant la soirée**, elle ne touche plus à rien : ce qu'un ami pousse
//!   sur GitHub ne change pas le jeu rangé. Un lancement ne compile rien.
//! - **Mettre un jeu à jour** se demande, sur l'étagère ou par `soiree maj
//!   <jeu>` : la nouvelle version est vérifiée pendant que l'ancienne reste
//!   jouable, et ne la remplace que si elle passe.
//!
//! Les téléphones n'ouvrent jamais le port d'un jeu : ils arrivent à
//! l'étagère, qui les amène au jeu en cours. Leur adresse ne change donc pas
//! d'un jeu à l'autre (un téléphone garde sa page ouverte, et le jeu suivant
//! y arrive), et le pare-feu de l'ordinateur n'a qu'un programme à laisser
//! passer.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::io::{BufReader, IsTerminal as _, Read as _, Write as _};
use std::net::{IpAddr, Ipv4Addr, Shutdown, TcpListener, TcpStream, ToSocketAddrs as _, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::cover::{self, COVER, Card};
use crate::internet::{Link, Passage};
use crate::{games, plug, rules, sh};

/// Le port de l'étagère, sauf si `SOIREE_ETAGERE_PORT` en donne un autre. Les
/// téléphones arrivent sur le port suivant.
const PORT: u16 = 8088;
const PORT_VARIABLE: &str = "SOIREE_ETAGERE_PORT";
/// Au plus tant de jeux se préparent à la fois.
const WORKERS_MAX: usize = 3;
/// Là où l'étagère range les jeux prêts, dans le dossier des jeux.
const FOLDER: &str = ".etagere";
const CARD_FILE: &str = "fiche.json";
const CHECK_LOG: &str = "verification.log";
const GAME_LOG: &str = "partie.log";
/// Ce fichier existe, sous l'étagère, tant que l'hôte a laissé « internet »
/// allumé : l'étagère s'en souvient d'un lancement à l'autre.
const INTERNET_FILE: &str = "internet";
/// Un jeu rangé sert sa première page en moins de temps que ça.
const LAUNCH_PATIENCE: Duration = Duration::from_secs(20);
const BETWEEN_TRIES: Duration = Duration::from_millis(100);
/// Allumé mais pas encore relié, le relais a tant de temps pour répondre
/// avant qu'un jeu se lance sans lui.
const LINK_PATIENCE: Duration = Duration::from_secs(5);
const ANSWER_WITHIN: Duration = Duration::from_secs(5);
/// L'en-tête qu'une page d'un autre site ne peut pas envoyer : seule la page
/// de l'étagère, ou `soiree maj`, lance ou met à jour un jeu.
const TRUSTED: &str = "x-soiree";
/// L'en-tête d'une demande à l'étagère ne dépasse pas cette taille : celui
/// d'une question au guichet porte les en-têtes de la question.
const HEAD_MAX: usize = 32 * 1024;
/// Les en-têtes qu'une question au guichet porte, et que l'étagère fait
/// suivre au relais tels quels.
const COUNTER_HEADERS: &str = "x-guichet-";

const PAGE: &str = include_str!("../etagere/etagere.html");
const FONTS: [(&str, &[u8]); 2] = [
    (
        "silkscreen.woff2",
        include_bytes!("../etagere/silkscreen-700.woff2"),
    ),
    ("vt323.woff2", include_bytes!("../etagere/vt323.woff2")),
];

/// Où en est un jeu sur l'étagère.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    /// Son tour n'est pas encore venu.
    Waiting,
    /// L'étagère le récupère, le vérifie ou le range.
    Preparing,
    /// Sa dernière préparation a réussi.
    Ready,
    /// Sa dernière préparation a échoué.
    Refused,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "attente",
            Self::Preparing => "preparation",
            Self::Ready => "pret",
            Self::Refused => "refuse",
        }
    }
}

/// Ce que l'étagère écrit à côté d'un jeu rangé, pour le retrouver au
/// prochain démarrage.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Filed {
    /// Le numéro du rangement : chaque mise à jour a son dossier.
    version: u32,
    /// Ce qui dit que le jeu n'a pas changé depuis (voir [`key`]).
    key: String,
    /// Le nom du fichier du programme.
    program: String,
    title: String,
    players: (u32, u32),
}

impl Filed {
    fn card(&self) -> Card {
        Card {
            title: self.title.clone(),
            players: self.players,
        }
    }
}

/// Un jeu sur l'étagère.
struct Entry {
    status: Status,
    /// Ce que l'étagère dit de ce jeu : l'étape en cours, ou pourquoi il
    /// n'est pas à jour.
    note: String,
    /// La version rangée, celle qu'on joue.
    filed: Option<Filed>,
    /// La fiche lue dans le dossier du jeu, tant que rien n'est rangé.
    card: Option<Card>,
}

struct Running {
    name: String,
    child: Child,
    /// Son QR code mène au relais : ses téléphones passent par internet.
    internet: bool,
}

struct Shelf {
    /// Le dossier des jeux : un sous-dossier par jeu.
    games: PathBuf,
    port: u16,
    entries: BTreeMap<String, Entry>,
    running: Option<Running>,
    /// Un jeu est en train de se lancer.
    launching: bool,
    /// Ce que l'étagère a à dire à l'hôte : un jeu qui s'est arrêté seul.
    notice: String,
    jobs: mpsc::Sender<Job>,
    /// Les jeux en cours de préparation : un jeu ne se prépare pas deux fois
    /// en même temps.
    busy: BTreeSet<String>,
    /// Les jeux redemandés pendant leur préparation : ils repassent ensuite.
    again: BTreeMap<String, Why>,
    /// Le passage par internet, si l'hôte l'a réglé, et le lien tenu avec le
    /// relais tant qu'il est allumé.
    passage: Option<Passage>,
    link: Option<Link>,
}

/// Pourquoi préparer un jeu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Why {
    /// L'étagère démarre : un jeu rangé qui n'a pas changé reste tel quel.
    Startup,
    /// L'hôte le demande : le jeu est revérifié, quoi qu'il en soit.
    Update,
}

struct Job {
    name: String,
    why: Why,
}

type Shared = Arc<Mutex<Shelf>>;

fn lock(shared: &Shared) -> MutexGuard<'_, Shelf> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

fn port() -> Result<u16, String> {
    match std::env::var(PORT_VARIABLE) {
        Ok(text) => text
            .parse()
            .map_err(|_| format!("{PORT_VARIABLE} n'est pas un port : {text}")),
        Err(_) => Ok(PORT),
    }
}

fn shelf_url(port: u16) -> String {
    format!("http://localhost:{port}/")
}

/// Le port que les téléphones ouvrent : c'est l'étagère qui y répond, et
/// qui les amène au jeu en cours. Il ne change pas d'un jeu à l'autre, et le
/// pare-feu de l'ordinateur n'a qu'un programme à laisser passer : l'étagère.
fn public_port(port: u16) -> u16 {
    port.saturating_add(1)
}

/// Le port de l'écran du jeu lancé, sur cet ordinateur seulement.
fn game_port(port: u16) -> u16 {
    port.saturating_add(2)
}

/// Le port, sur cet ordinateur seulement, où l'étagère amène les téléphones
/// au jeu lancé.
fn door_port(port: u16) -> u16 {
    port.saturating_add(3)
}

/// Amène au jeu en cours tout ce qui arrive du Wi-Fi, octet pour octet : le
/// jeu répond lui-même, en clair ou en HTTPS. Sans jeu, la connexion est
/// refermée, et le téléphone réessaie.
fn relay(public: &TcpListener, door: u16) {
    for phone in public.incoming().flatten() {
        let _: thread::JoinHandle<()> = thread::spawn(move || carry(phone, door));
    }
}

fn carry(phone: TcpStream, door: u16) {
    let Ok(game) = TcpStream::connect((Ipv4Addr::LOCALHOST, door)) else {
        return;
    };
    // Un geste de raquette n'attend pas qu'un paquet se remplisse.
    let _: std::io::Result<()> = phone.set_nodelay(true);
    let _: std::io::Result<()> = game.set_nodelay(true);
    let (Ok(mut from_phone), Ok(mut to_game)) = (phone.try_clone(), game.try_clone()) else {
        return;
    };
    let (mut to_phone, mut from_game) = (phone, game);
    let up = thread::spawn(move || {
        let _: std::io::Result<u64> = std::io::copy(&mut from_phone, &mut to_game);
        let _: std::io::Result<()> = to_game.shutdown(Shutdown::Both);
        let _: std::io::Result<()> = from_phone.shutdown(Shutdown::Both);
    });
    let _: std::io::Result<u64> = std::io::copy(&mut from_game, &mut to_phone);
    let _: std::io::Result<()> = to_phone.shutdown(Shutdown::Both);
    let _: std::io::Result<()> = from_game.shutdown(Shutdown::Both);
    let _: thread::Result<()> = up.join();
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
        .unwrap_or(IpAddr::from(Ipv4Addr::LOCALHOST))
}

/// Dit par où les téléphones arrivent. En HTTPS, ils passent par un nom, qui
/// doit mener à cet ordinateur sur le Wi-Fi : s'il mène ailleurs, l'étagère
/// le fait suivre dès maintenant (la commande de `SOIREE_TLS_SUIVRE`, comme
/// la brique « télécommande »), bien avant le premier QR code scanné.
fn announce_phones(public: u16) {
    let here = lan_address();
    let read = |name: &str| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
    };
    let Some(name) = read("SOIREE_TLS_NOM") else {
        println!("Téléphones  : http://{here}:{public}/t, sur le même Wi-Fi que cet ordinateur");
        return;
    };
    println!("Téléphones  : https://{name}:{public}/t");
    println!("              Ce nom n'existe que pour le HTTPS : il mène à {here}, cet ordinateur");
    println!("              sur le Wi-Fi. Rien ne passe par internet.");
    let found: Vec<IpAddr> = (name.as_str(), public)
        .to_socket_addrs()
        .map(|addresses| addresses.map(|address| address.ip()).collect())
        .unwrap_or_default();
    if found.contains(&here) {
        return;
    }
    let Some(command) = read("SOIREE_TLS_SUIVRE") else {
        println!("Attention : {name} ne mène pas à {here}. Les téléphones n'arriveront pas.");
        return;
    };
    // La commande porte les identifiants de l'hôte : on ne l'affiche jamais.
    let command = command.replace("{adresse}", &here.to_string());
    let _: thread::JoinHandle<()> = thread::spawn(move || {
        let done = Command::new("sh")
            .args(["-c", &command])
            .stdin(Stdio::null())
            .output()
            .is_ok_and(|output| output.status.success());
        if done {
            println!(
                "── {name} mène maintenant à {here}. Les téléphones le voient d'ici une minute."
            );
        } else {
            println!(
                "Attention : {name} n'a pas pu suivre vers {here}. Les téléphones n'arriveront pas."
            );
        }
    });
}

pub fn run(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| format!("{} : {error}", dir.display()))?;
    let games = dir
        .canonicalize()
        .map_err(|error| format!("{} : {error}", dir.display()))?;
    let port = port()?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).map_err(|error| {
        format!(
            "le port {port} est pris ({error}) : l'étagère tourne peut-être déjà. Sinon, donne-lui \
             un autre port avec la variable {PORT_VARIABLE}."
        )
    })?;
    let public = public_port(port);
    let phones = TcpListener::bind(("0.0.0.0", public))
        .map_err(|error| format!("le port {public}, celui des téléphones, est pris ({error})."))?;
    let door = door_port(port);
    let _: thread::JoinHandle<()> = thread::spawn(move || relay(&phones, door));
    println!("soiree {} : l'étagère\n", crate::VERSION);
    let names = gather(&games);
    if names.is_empty() {
        return Err(format!(
            "aucun jeu dans {} : `soiree games` les récupère, `soiree new` en crée un.",
            games.display()
        ));
    }
    let passage = Passage::from_env()?;
    // Allumé la dernière fois, allumé cette fois : l'hôte n'a pas à y repenser.
    let link = passage
        .as_ref()
        .filter(|_| remembers_internet(&games))
        .map(|passage| passage.open(public));
    let on = link.is_some();
    let (jobs, queue) = mpsc::channel();
    let entries: BTreeMap<String, Entry> = names
        .iter()
        .map(|name| (name.clone(), entry(&games, name)))
        .collect();
    // Les jeux déjà rangés d'abord : inchangés, ils sont prêts tout de suite,
    // et un jeu neuf, long à vérifier, ne les fait pas attendre.
    let mut order = names;
    order.sort_by_key(|name| entries.get(name).is_none_or(|entry| entry.filed.is_none()));
    let shared: Shared = Arc::new(Mutex::new(Shelf {
        games,
        port,
        entries,
        running: None,
        launching: false,
        notice: String::new(),
        jobs: jobs.clone(),
        busy: BTreeSet::new(),
        again: BTreeMap::new(),
        passage: passage.clone(),
        link,
    }));
    for name in order {
        let _: Result<(), mpsc::SendError<Job>> = jobs.send(Job {
            name,
            why: Why::Startup,
        });
    }
    // Plusieurs jeux se préparent à la fois : chacun prend le suivant de la file.
    let queue = Arc::new(Mutex::new(queue));
    for _ in 0..workers() {
        let (shared, queue) = (Arc::clone(&shared), Arc::clone(&queue));
        let _: thread::JoinHandle<()> = thread::spawn(move || {
            loop {
                let job = queue.lock().unwrap_or_else(PoisonError::into_inner).recv();
                let Ok(job) = job else {
                    break;
                };
                work(&shared, job);
            }
        });
    }
    let url = shelf_url(port);
    println!("\nÉtagère     : {url}");
    announce_phones(public);
    if let Some(passage) = &passage {
        if on {
            println!(
                "Internet    : allumé, comme la dernière fois. Les téléphones passent par {}\n              \
                 La touche I, sur l'étagère, l'éteint.",
                passage.phones()
            );
        } else {
            println!(
                "Internet    : éteint. La touche I, sur l'étagère, fait passer les téléphones par {}",
                passage.phones()
            );
        }
    }
    println!("Chaque jeu s'allume quand il est prêt. Ctrl-C ferme l'étagère et le jeu en cours.\n");
    // Lancée par un autre programme, l'étagère n'ouvre pas de fenêtre.
    if std::io::stdout().is_terminal() {
        open_browser(&url);
    }
    for stream in listener.incoming().flatten() {
        let shared = Arc::clone(&shared);
        let _: thread::JoinHandle<()> = thread::spawn(move || answer(stream, &shared));
    }
    Ok(())
}

/// Les jeux à montrer : ceux de l'organisation, récupérés s'ils manquent, et
/// ceux qui sont déjà dans le dossier (un jeu pas encore publié).
fn gather(games: &Path) -> Vec<String> {
    match games::listed(games) {
        Ok(listed) => {
            for name in &listed {
                if !games.join(name).is_dir() {
                    println!("── {name} : récupéré depuis GitHub");
                    if !games::clone(games, name) {
                        println!("  impossible de le récupérer.");
                    }
                }
            }
        }
        Err(why) => println!(
            "GitHub ne répond pas : l'étagère montre les jeux déjà là, sans chercher les nouveaux.\n  ({why})\n"
        ),
    }
    let mut names: Vec<String> = fs::read_dir(games)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| is_game(&entry.path()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort();
    names
}

/// Un jeu emprunte la brique « cartouche ». L'organisation a d'autres dépôts
/// (le site d'invitation) : ils n'ont pas leur place sur l'étagère.
fn is_game(folder: &Path) -> bool {
    fs::read_to_string(folder.join("Cargo.toml"))
        .is_ok_and(|manifest| manifest.contains("cartouche"))
}

/// Un jeu tel que l'étagère le trouve en démarrant : peut-être déjà rangé.
fn entry(games: &Path, name: &str) -> Entry {
    let filed = fs::read(games.join(FOLDER).join(name).join(CARD_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Filed>(&bytes).ok())
        // Rangé par une autre version de la console : à refaire.
        .filter(|filed| filed.key.starts_with(&version_prefix()))
        .filter(|filed| program(games, name, filed).is_file());
    Entry {
        status: Status::Waiting,
        note: String::new(),
        filed,
        card: cover::card(&games.join(name)).ok(),
    }
}

fn version_prefix() -> String {
    format!("soiree {} ", crate::VERSION)
}

/// Le dossier d'une version rangée d'un jeu.
fn filed_dir(games: &Path, name: &str, version: u32) -> PathBuf {
    games.join(FOLDER).join(name).join(version.to_string())
}

fn program(games: &Path, name: &str, filed: &Filed) -> PathBuf {
    filed_dir(games, name, filed.version).join(&filed.program)
}

/// Ce qui change quand le jeu change : la version de la console, le commit,
/// et ce qui n'est pas encore commité. `None` hors d'un dépôt git : le jeu
/// est alors revérifié à chaque fois.
fn key(game: &Path) -> Option<String> {
    let head = sh::output(game, "git", &["rev-parse", "HEAD"])?;
    let changes = sh::output(game, "git", &["status", "--porcelain"])?;
    let prefix = version_prefix();
    if changes.is_empty() {
        return Some(format!("{prefix}{head}"));
    }
    let diff = sh::output(game, "git", &["diff", "HEAD"]).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    changes.hash(&mut hasher);
    diff.hash(&mut hasher);
    Some(format!("{prefix}{head} +{:016x}", hasher.finish()))
}

fn note(shared: &Shared, name: &str, status: Status, note: &str) {
    println!("── {name} : {note}");
    if let Some(entry) = lock(shared).entries.get_mut(name) {
        entry.status = status;
        note.clone_into(&mut entry.note);
    }
}

/// Combien de jeux l'étagère prépare à la fois. La vérification d'un jeu
/// occupe déjà plusieurs cœurs (la compilation, puis les parties du banc), et
/// elle chronomètre la machine : on en mène peu de front, un pour quatre cœurs.
fn workers() -> usize {
    let cores = thread::available_parallelism().map_or(1, core::num::NonZero::get);
    cores.checked_div(4).unwrap_or(1).clamp(1, WORKERS_MAX)
}

/// Prépare un jeu, à moins qu'il ne soit déjà en préparation : il repasse
/// alors dans la file quand elle est finie.
fn work(shared: &Shared, job: Job) {
    {
        let mut shelf = lock(shared);
        if !shelf.busy.insert(job.name.clone()) {
            // Une mise à jour demandée par l'hôte l'emporte sur un démarrage.
            let why = match shelf.again.get(&job.name) {
                Some(Why::Update) => Why::Update,
                Some(Why::Startup) | None => job.why,
            };
            let _: Option<Why> = shelf.again.insert(job.name, why);
            return;
        }
    }
    prepare(shared, &job);
    let mut shelf = lock(shared);
    let _: bool = shelf.busy.remove(&job.name);
    if let Some(why) = shelf.again.remove(&job.name) {
        let _: Result<(), mpsc::SendError<Job>> = shelf.jobs.send(Job {
            name: job.name,
            why,
        });
    }
}

/// Récupère, vérifie et range un jeu. La version déjà rangée reste jouable
/// tant que la nouvelle n'a pas passé la vérification.
fn prepare(shared: &Shared, job: &Job) {
    let name = job.name.as_str();
    let (games, known) = {
        let shelf = lock(shared);
        let known = shelf
            .entries
            .get(name)
            .and_then(|entry| entry.filed.clone());
        (shelf.games.clone(), known)
    };
    let game = games.join(name);
    note(shared, name, Status::Preparing, "mise à jour depuis GitHub");
    let clean = sh::output(&game, "git", &["status", "--porcelain"])
        .is_some_and(|changes| changes.is_empty());
    // Un dossier avec des changements locaux est pris tel qu'il est.
    if clean {
        let _: bool = sh::quiet(&game, "git", &["pull", "--ff-only"]);
    }
    let key = key(&game);
    if job.why == Why::Startup
        && let Some(known) = &known
        && key.as_ref() == Some(&known.key)
    {
        return note(
            shared,
            name,
            Status::Ready,
            "prêt, inchangé depuis la dernière fois",
        );
    }
    note(
        shared,
        name,
        Status::Preparing,
        "vérification : la compilation, puis la console joue des parties (plusieurs minutes pour un jeu neuf)",
    );
    let outcome = if check(&games, name) {
        let version = known
            .as_ref()
            .map_or(1, |known| known.version.saturating_add(1));
        file(&games, name, version, key.unwrap_or_default())
    } else {
        Err(format!(
            "ne passe pas la vérification : `soiree check` dans {} dit pourquoi",
            game.display()
        ))
    };
    match outcome {
        Ok(filed) => {
            let mut shelf = lock(shared);
            let in_use = shelf
                .running
                .as_ref()
                .is_some_and(|running| running.name == name);
            if let Some(entry) = shelf.entries.get_mut(name) {
                entry.card = Some(filed.card());
                entry.filed = Some(filed.clone());
            }
            drop(shelf);
            // L'ancienne version sert encore au jeu qui tourne.
            if !in_use {
                forget_older(&games, name, filed.version);
            }
            note(shared, name, Status::Ready, "prêt");
        }
        Err(why) if known.is_some() => note(
            shared,
            name,
            Status::Refused,
            &format!("la nouvelle version {why}. L'ancienne reste sur l'étagère."),
        ),
        Err(why) => note(shared, name, Status::Refused, &why),
    }
}

/// Lance `soiree check` sur le jeu, et garde ce qu'il dit dans un fichier.
fn check(games: &Path, name: &str) -> bool {
    let folder = games.join(FOLDER).join(name);
    let log = fs::create_dir_all(&folder).and_then(|()| File::create(folder.join(CHECK_LOG)));
    let (Ok(log), Ok(tool)) = (log, std::env::current_exe()) else {
        return false;
    };
    let Ok(errors) = log.try_clone() else {
        return false;
    };
    Command::new(tool)
        .arg("check")
        .current_dir(games.join(name))
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(errors)
        .status()
        .is_ok_and(|status| status.success())
}

/// Range ce qu'il faut pour lancer le jeu sans rien compiler : son programme,
/// ses pages et sa jaquette, tels que la vérification vient de les fabriquer.
fn file(games: &Path, name: &str, version: u32, key: String) -> Result<Filed, String> {
    let game = games.join(name);
    let metadata = rules::metadata(&game)?;
    let binaries = metadata.binaries();
    let [(_, target)] = binaries.as_slice() else {
        return Err("n'a pas un seul programme".to_owned());
    };
    let program = format!("{}{}", target.name, std::env::consts::EXE_SUFFIX);
    let card = cover::card(&game).map_err(|wrong| wrong.join(", "))?;
    let to = filed_dir(games, name, version);
    let copied = (|| {
        if to.exists() {
            fs::remove_dir_all(&to)?;
        }
        fs::create_dir_all(&to)?;
        let built = metadata.target_directory.join("release").join(&program);
        let _: u64 = fs::copy(built, to.join(&program))?;
        let _: u64 = fs::copy(game.join(COVER), to.join(COVER))?;
        copy_dir(&metadata.target_directory.join("pages"), &to.join("pages"))
    })();
    copied.map_err(|error: std::io::Error| {
        format!("ne se range pas dans {} : {error}", to.display())
    })?;
    let filed = Filed {
        version,
        key,
        program,
        title: card.title,
        players: card.players,
    };
    let text = serde_json::to_vec_pretty(&filed).map_err(|error| error.to_string())?;
    fs::write(games.join(FOLDER).join(name).join(CARD_FILE), text)
        .map_err(|error| format!("la fiche ne s'écrit pas : {error}"))?;
    Ok(filed)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            let _: u64 = fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Efface les versions rangées plus vieilles que `keep`.
fn forget_older(games: &Path, name: &str, keep: u32) {
    let folder = games.join(FOLDER).join(name);
    for entry in fs::read_dir(folder).into_iter().flatten().flatten() {
        let older = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
            .is_some_and(|version| version < keep);
        if older {
            let _: std::io::Result<()> = fs::remove_dir_all(entry.path());
        }
    }
}

/// Arrête le jeu en cours, s'il y en a un.
fn stop(shared: &Shared) {
    let running = lock(shared).running.take();
    if let Some(mut running) = running {
        let _: std::io::Result<()> = running.child.kill();
        let _: std::io::Result<std::process::ExitStatus> = running.child.wait();
    }
}

/// Le jeu en cours s'est-il arrêté ? Par son menu, c'est normal ; sinon
/// l'étagère le dit.
fn reap(shelf: &mut Shelf) {
    let Some(running) = &mut shelf.running else {
        return;
    };
    let Ok(Some(status)) = running.child.try_wait() else {
        return;
    };
    if !status.success() {
        shelf.notice = format!(
            "{} s'est arrêté tout seul. Ce qu'il a dit : {}",
            running.name,
            shelf
                .games
                .join(FOLDER)
                .join(&running.name)
                .join(GAME_LOG)
                .display()
        );
    }
    shelf.running = None;
}

/// Lance le jeu rangé et rend l'adresse de son écran, une fois sa page servie.
fn launch(shared: &Shared, name: &str) -> Result<String, String> {
    let (games, port, filed) = {
        let mut shelf = lock(shared);
        if shelf.launching {
            return Err("un jeu se lance déjà".to_owned());
        }
        let filed = shelf
            .entries
            .get(name)
            .and_then(|entry| entry.filed.clone())
            .ok_or("ce jeu n'est pas prêt")?;
        shelf.launching = true;
        shelf.notice.clear();
        (shelf.games.clone(), shelf.port, filed)
    };
    let internet = internet(shared);
    stop(shared);
    // Plus aucun jeu ne tourne : les versions remplacées ne servent plus.
    forget_older(&games, name, filed.version);
    let started = start(&games, name, &filed, port, internet.as_deref());
    let mut shelf = lock(shared);
    shelf.launching = false;
    let child = started?;
    shelf.running = Some(Running {
        name: name.to_owned(),
        child,
        internet: internet.is_some(),
    });
    Ok(shelf_url(game_port(port)))
}

/// L'adresse du relais, si les téléphones du jeu qui se lance passent par
/// internet : le jeu la leur montre. Allumé mais pas encore relié (l'étagère
/// vient de démarrer, l'hôte vient d'allumer), le relais a quelques secondes
/// pour répondre ; sans réponse, les téléphones restent sur le Wi-Fi, et
/// l'étagère le dit.
fn internet(shared: &Shared) -> Option<String> {
    let since = Instant::now();
    loop {
        let mut shelf = lock(shared);
        let linked = shelf.link.as_ref()?.linked();
        if linked {
            return shelf
                .passage
                .as_ref()
                .map(|passage| passage.phones().to_owned());
        }
        if since.elapsed() >= LINK_PATIENCE {
            "Le relais ne répond pas : pour ce jeu, les téléphones passent par le Wi-Fi."
                .clone_into(&mut shelf.notice);
            return None;
        }
        drop(shelf);
        thread::sleep(BETWEEN_TRIES);
    }
}

/// L'hôte avait-il laissé « internet » allumé ?
fn remembers_internet(games: &Path) -> bool {
    games.join(FOLDER).join(INTERNET_FILE).exists()
}

/// Retient qu'« internet » est allumé, pour le prochain `soiree play`. Si ça
/// ne s'écrit pas, l'interrupteur marche quand même : il sera éteint la
/// prochaine fois.
fn remember_internet(games: &Path) {
    let folder = games.join(FOLDER);
    let _: std::io::Result<()> = fs::create_dir_all(&folder)
        .and_then(|()| fs::write(folder.join(INTERNET_FILE), "allumé\n"));
}

/// Oublie qu'« internet » était allumé.
fn forget_internet(games: &Path) {
    let _: std::io::Result<()> = fs::remove_file(games.join(FOLDER).join(INTERNET_FILE));
}

fn start(
    games: &Path,
    name: &str,
    filed: &Filed,
    port: u16,
    internet: Option<&str>,
) -> Result<Child, String> {
    let game = games.join(name);
    let folder = filed_dir(games, name, filed.version);
    let log_path = games.join(FOLDER).join(name).join(GAME_LOG);
    let log =
        File::create(&log_path).map_err(|error| format!("{} : {error}", log_path.display()))?;
    let errors = log.try_clone().map_err(|error| error.to_string())?;
    let program = folder.join(&filed.program);
    let game_port = game_port(port);
    let mut command = Command::new(&program);
    let _: &mut Command = command
        .current_dir(&game)
        .envs(secrets(&game))
        .env("PORT", game_port.to_string())
        .env(
            "SOIREE_RELAIS",
            format!("{}:{}", public_port(port), door_port(port)),
        )
        .env("SOIREE_PAGES", folder.join("pages"))
        .env("SOIREE_ETAGERE", shelf_url(port))
        // La clé du relais est à l'étagère : le jeu n'en a pas besoin.
        .env_remove(crate::internet::KEY_VARIABLE)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(errors);
    // L'adresse que le jeu montre aux téléphones (son QR code) : celle du
    // relais, quand ils passent par internet.
    if let Some(address) = internet {
        let _: &mut Command = command.env("SOIREE_ADRESSE", address);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("{} ne se lance pas : {error}", program.display()))?;
    let since = Instant::now();
    loop {
        if plug::serves_a_page(game_port) {
            return Ok(child);
        }
        let exited = !matches!(child.try_wait(), Ok(None));
        if exited || since.elapsed() > LAUNCH_PATIENCE {
            let _: std::io::Result<()> = child.kill();
            let _: std::io::Result<std::process::ExitStatus> = child.wait();
            let said = fs::read_to_string(&log_path).unwrap_or_default();
            let last = said.lines().rev().find(|line| !line.trim().is_empty());
            return Err(format!(
                "le jeu ne s'est pas ouvert : {}",
                last.unwrap_or("il n'a rien dit")
            ));
        }
        thread::sleep(BETWEEN_TRIES);
    }
}

/// Les secrets d'un jeu qui en a (`fnox.toml` dans son dossier) : l'hôte les
/// lui confie par fnox, et l'étagère les lui passe au lancement. Ils ne sont
/// jamais affichés.
fn secrets(game: &Path) -> Vec<(String, String)> {
    if cfg!(windows) || !game.join("fnox.toml").is_file() {
        return Vec::new();
    }
    let listed = Command::new("fnox")
        .current_dir(game)
        .args(["exec", "--", "env", "-0"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    let Ok(listed) = listed else {
        return Vec::new();
    };
    String::from_utf8_lossy(&listed.stdout)
        .split('\0')
        .filter_map(|variable| variable.split_once('='))
        .filter(|(name, _)| name.starts_with("SOIREE_"))
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect()
}

/// Ce que la page de l'étagère montre.
fn state(shelf: &mut Shelf) -> serde_json::Value {
    reap(shelf);
    let games: Vec<serde_json::Value> = shelf
        .entries
        .iter()
        .map(|(name, entry)| {
            let card = entry
                .filed
                .as_ref()
                .map(Filed::card)
                .or_else(|| entry.card.clone());
            json!({
                "nom": name,
                "titre": card.as_ref().map_or(name.as_str(), |card| card.title.as_str()),
                "joueurs": card.as_ref().map(|card| [card.players.0, card.players.1]),
                "etat": entry.status.as_str(),
                "jouable": entry.filed.is_some(),
                "note": entry.note,
                "version": entry.filed.as_ref().map_or(0, |filed| filed.version),
            })
        })
        .collect();
    json!({
        "console": crate::VERSION,
        "jeux": games,
        "en_cours": shelf.running.as_ref().map(|running| json!({
            "nom": running.name,
            "adresse": shelf_url(game_port(shelf.port)),
            // Son QR code mène-t-il au relais ? Fixé à son lancement.
            "internet": running.internet,
        })),
        "avis": shelf.notice,
        // Le passage par internet : réglé par l'hôte, allumé sur l'étagère,
        // et relié quand le relais répond.
        "internet": shelf.passage.as_ref().map(|passage| json!({
            "adresse": passage.phones(),
            "allume": shelf.link.is_some(),
            "relie": shelf.link.as_ref().is_some_and(Link::linked),
        })),
    })
}

struct Request {
    method: String,
    path: String,
    /// La demande porte l'en-tête de l'étagère.
    trusted: bool,
    /// Ses en-têtes, le nom en minuscules, dans l'ordre.
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

struct Reply {
    status: &'static str,
    kind: &'static str,
    body: Vec<u8>,
    /// Le statut du serveur appelé, pour une réponse du guichet.
    counted: Option<u16>,
}

impl Reply {
    fn new(status: &'static str, kind: &'static str, body: Vec<u8>) -> Self {
        Self {
            status,
            kind,
            body,
            counted: None,
        }
    }

    fn json(value: &serde_json::Value) -> Self {
        Self::new(
            "200 OK",
            "application/json; charset=utf-8",
            value.to_string().into_bytes(),
        )
    }

    fn refused(status: &'static str, why: &str) -> Self {
        Self::new(
            status,
            "application/json; charset=utf-8",
            json!({ "erreur": why }).to_string().into_bytes(),
        )
    }

    fn file(kind: &'static str, path: &Path) -> Self {
        match fs::read(path) {
            Ok(body) => Self::new("200 OK", kind, body),
            Err(_) => Self::refused("404 Not Found", "introuvable"),
        }
    }
}

fn read_request(stream: &TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream);
    let mut head: Vec<u8> = Vec::new();
    while !head.ends_with(b"\r\n\r\n") && !head.ends_with(b"\n\n") {
        if head.len() >= HEAD_MAX {
            return None;
        }
        let mut byte = [0_u8; 1];
        reader.read_exact(&mut byte).ok()?;
        head.extend_from_slice(&byte);
    }
    let head = String::from_utf8(head).ok()?;
    let mut lines = head.lines();
    let first = lines.next()?;
    let mut parts = first.split(' ');
    let method = parts.next()?.to_owned();
    let path = parts.next()?.split('?').next()?.to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let trusted = headers.iter().any(|(name, _)| name == TRUSTED);
    let length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .map_or(Some(0), |(_, length)| length.parse::<usize>().ok())?;
    if length > crate::internet::COUNTER_MAX {
        return None;
    }
    let mut body = vec![0_u8; length];
    reader.read_exact(&mut body).ok()?;
    Some(Request {
        method,
        path,
        trusted,
        headers,
        body,
    })
}

fn answer(mut stream: TcpStream, shared: &Shared) {
    let _: std::io::Result<()> = stream.set_read_timeout(Some(ANSWER_WITHIN));
    let Some(request) = read_request(&stream) else {
        return;
    };
    let reply = route(&request, shared);
    let counted = reply.counted.map_or_else(String::new, |status| {
        format!("{}: {status}\r\n", crate::internet::COUNTER_STATUS)
    });
    let head = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\n\
         {counted}Connection: close\r\n\r\n",
        reply.status,
        reply.kind,
        reply.body.len()
    );
    let _: std::io::Result<()> = stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(&reply.body));
}

fn route(request: &Request, shared: &Shared) -> Reply {
    let segments: Vec<&str> = request.path.trim_matches('/').split('/').collect();
    let known = |name: &str| lock(shared).entries.contains_key(name);
    match (request.method.as_str(), segments.as_slice()) {
        ("GET", [""]) => Reply::new(
            "200 OK",
            "text/html; charset=utf-8",
            PAGE.as_bytes().to_vec(),
        ),
        ("GET", ["etat"]) => Reply::json(&state(&mut lock(shared))),
        ("GET", ["police", name]) => FONTS.iter().find(|(font, _)| font == name).map_or_else(
            || Reply::refused("404 Not Found", "introuvable"),
            |(_, bytes)| Reply::new("200 OK", "font/woff2", bytes.to_vec()),
        ),
        ("GET", ["jaquette", name]) if known(name) => {
            let shelf = lock(shared);
            let filed = shelf
                .entries
                .get(*name)
                .and_then(|entry| entry.filed.as_ref());
            let path = filed.map_or_else(
                || shelf.games.join(name).join(COVER),
                |filed| filed_dir(&shelf.games, name, filed.version).join(COVER),
            );
            drop(shelf);
            Reply::file("image/png", &path)
        }
        ("GET", ["journal", name]) if known(name) => {
            let path = lock(shared).games.join(FOLDER).join(name).join(CHECK_LOG);
            Reply::file("text/plain; charset=utf-8", &path)
        }
        ("POST", _) if !request.trusted => Reply::refused("403 Forbidden", "refusé"),
        ("POST", ["lancer", name]) if known(name) => match launch(shared, name) {
            Ok(url) => Reply::json(&json!({ "adresse": url })),
            Err(why) => Reply::refused("409 Conflict", &why),
        },
        ("POST", ["maj", name]) if known(name) => {
            let shelf = lock(shared);
            let _: Result<(), mpsc::SendError<Job>> = shelf.jobs.send(Job {
                name: (*name).to_owned(),
                why: Why::Update,
            });
            drop(shelf);
            note(shared, name, Status::Preparing, "mise à jour demandée");
            Reply::json(&json!({ "fait": true }))
        }
        ("POST", ["internet"]) => {
            let mut shelf = lock(shared);
            let Some(passage) = shelf.passage.clone() else {
                return Reply::refused(
                    "409 Conflict",
                    "le passage par internet n'est pas réglé (SOIREE_INTERNET)",
                );
            };
            // Lâcher le lien le ferme : les téléphones venus par internet repartent.
            shelf.link = match shelf.link.take() {
                Some(_) => None,
                None => Some(passage.open(public_port(shelf.port))),
            };
            if shelf.link.is_some() {
                remember_internet(&shelf.games);
            } else {
                forget_internet(&shelf.games);
            }
            Reply::json(&json!({ "allume": shelf.link.is_some() }))
        }
        ("POST", ["guichet"]) => {
            let passage = lock(shared).passage.clone();
            counter(passage.as_ref(), request)
        }
        ("POST", ["arreter"]) => {
            stop(shared);
            Reply::json(&json!({ "fait": true }))
        }
        _ => Reply::refused("404 Not Found", "introuvable"),
    }
}

/// Le guichet : une question d'un jeu dont la console n'a pas le secret part
/// au relais de l'hôte, qui y met le secret qu'il garde, que l'interrupteur
/// « internet » soit allumé ou non. La clé du relais reste ici.
fn counter(passage: Option<&Passage>, request: &Request) -> Reply {
    let Some(passage) = passage else {
        return Reply::refused(
            "409 Conflict",
            "cette étagère n'a pas de guichet : le passage par internet n'est pas réglé \
             (SOIREE_INTERNET et SOIREE_INTERNET_CLE)",
        );
    };
    let headers: Vec<(&str, &str)> = request
        .headers
        .iter()
        .filter(|(name, _)| name.starts_with(COUNTER_HEADERS))
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    match passage.counter(&headers, &request.body) {
        Ok((status, body)) => Reply {
            counted: Some(status),
            ..Reply::new("200 OK", "application/octet-stream", body)
        },
        Err(why) => Reply::refused("502 Bad Gateway", &why),
    }
}

fn open_browser(url: &str) {
    let (program, args): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("open", &[])
    } else if cfg!(windows) {
        ("cmd", &["/C", "start"])
    } else {
        ("xdg-open", &[])
    };
    let _: std::io::Result<Child> = Command::new(program)
        .args(args)
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Une demande à l'étagère qui tourne sur cet ordinateur. Rend le corps de
/// sa réponse.
fn ask(port: u16, method: &str, path: &str) -> Option<String> {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).ok()?;
    stream.set_read_timeout(Some(ANSWER_WITHIN)).ok()?;
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\n{TRUSTED}: 1\r\nContent-Length: 0\r\n\
         Connection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).ok()?;
    let mut answer = String::new();
    let _: usize = stream.read_to_string(&mut answer).ok()?;
    let (_, body) = answer.split_once("\r\n\r\n")?;
    Some(body.to_owned())
}

/// `soiree maj <jeu>` : demande à l'étagère qui tourne de mettre un jeu à
/// jour, et attend de savoir s'il est passé.
pub fn update(args: &[String]) -> Result<(), String> {
    let [name] = args else {
        return Err("usage : soiree maj <jeu>".to_owned());
    };
    let port = port()?;
    let absent = || {
        format!(
            "l'étagère ne répond pas sur le port {port} : lance-la d'abord, avec `soiree play`."
        )
    };
    let asked = ask(port, "POST", &format!("/maj/{name}")).ok_or_else(absent)?;
    if !asked.contains("fait") {
        return Err(format!("l'étagère ne connaît pas le jeu « {name} »."));
    }
    println!("{name} : mise à jour demandée. L'ancienne version reste jouable en attendant.");
    let mut said = String::new();
    loop {
        thread::sleep(Duration::from_secs(1));
        let state: serde_json::Value = ask(port, "GET", "/etat")
            .and_then(|body| serde_json::from_str(&body).ok())
            .ok_or_else(absent)?;
        let game = state
            .get("jeux")
            .and_then(serde_json::Value::as_array)
            .and_then(|games| {
                games
                    .iter()
                    .find(|game| game.get("nom").and_then(serde_json::Value::as_str) == Some(name))
            });
        let read = |key: &str| {
            game.and_then(|game| game.get(key)?.as_str())
                .unwrap_or_default()
                .to_owned()
        };
        let note = read("note");
        if note != said {
            println!("  {note}");
            said = note;
        }
        match read("etat").as_str() {
            "pret" => return Ok(()),
            "refuse" => return Err(format!("{name} n'a pas été mis à jour.")),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_answers_on_the_port_after_the_shelf() {
        assert_eq!(public_port(8088), 8089);
        assert_eq!(game_port(8088), 8090);
        assert_eq!(door_port(8088), 8091);
        assert_eq!(shelf_url(8088), "http://localhost:8088/");
    }

    /// Une demande au guichet, avec l'en-tête de l'étagère ou sans.
    fn post(trusted: &str) -> Request {
        Request {
            method: "POST".to_owned(),
            path: "/guichet".to_owned(),
            trusted: trusted == TRUSTED,
            headers: vec![(
                "x-guichet-adresse".to_owned(),
                "https://juge.test/".to_owned(),
            )],
            body: Vec::new(),
        }
    }

    #[test]
    fn only_this_computer_may_ask_the_counter() {
        let shared: Shared = Arc::new(Mutex::new(Shelf {
            games: std::env::temp_dir(),
            port: PORT,
            entries: BTreeMap::new(),
            running: None,
            launching: false,
            notice: String::new(),
            jobs: mpsc::channel().0,
            busy: BTreeSet::new(),
            again: BTreeMap::new(),
            passage: None,
            link: None,
        }));
        // Une page d'un autre site ne peut pas envoyer l'en-tête de l'étagère.
        assert_eq!(route(&post(""), &shared).status, "403 Forbidden");
        assert_eq!(route(&post(TRUSTED), &shared).status, "409 Conflict");
    }

    #[test]
    fn the_internet_switch_is_remembered() {
        let games =
            std::env::temp_dir().join(format!("soiree-interrupteur-{}", std::process::id()));
        assert!(!remembers_internet(&games));
        remember_internet(&games);
        assert!(remembers_internet(&games));
        forget_internet(&games);
        assert!(!remembers_internet(&games));
        fs::remove_dir_all(&games).unwrap();
    }

    #[test]
    fn without_a_passage_there_is_no_counter() {
        let reply = counter(None, &post(TRUSTED));
        assert_eq!(reply.status, "409 Conflict");
        assert_eq!(reply.counted, None);
        let said = String::from_utf8(reply.body).unwrap();
        assert!(said.contains("SOIREE_INTERNET_CLE"), "{said}");
    }

    #[test]
    fn a_request_is_read_with_its_body() {
        let server = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = server.local_addr().unwrap().port();
        let sent = thread::spawn(move || {
            let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
            stream
                .write_all(
                    b"POST /guichet?x HTTP/1.1\r\nX-Soiree: 1\r\nX-Guichet-En-Tete: a: {B}\r\n\
                      Content-Length: 4\r\n\r\nping",
                )
                .unwrap();
            stream
        });
        let (stream, _) = server.accept().unwrap();
        let request = read_request(&stream).unwrap();
        assert_eq!(
            (request.method.as_str(), request.path.as_str()),
            ("POST", "/guichet")
        );
        assert!(request.trusted);
        assert!(
            request
                .headers
                .contains(&("x-guichet-en-tete".to_owned(), "a: {B}".to_owned()))
        );
        assert_eq!(request.body, b"ping");
        drop(sent.join().unwrap());
    }

    #[test]
    fn older_filed_versions_are_forgotten() {
        let games = std::env::temp_dir().join(format!("soiree-etagere-{}", std::process::id()));
        let _: std::io::Result<()> = fs::remove_dir_all(&games);
        for version in [1, 2, 3] {
            fs::create_dir_all(filed_dir(&games, "canards", version)).unwrap();
        }
        forget_older(&games, "canards", 3);
        assert!(!filed_dir(&games, "canards", 1).exists());
        assert!(!filed_dir(&games, "canards", 2).exists());
        assert!(filed_dir(&games, "canards", 3).exists());
    }
}
