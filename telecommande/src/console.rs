//! Le côté ordinateur : le programme de tous les jeux.
//!
//! ```ignore
//! fn main() -> std::process::ExitCode {
//!     telecommande::console::run::<machine::Jeu>(env!("CARGO_MANIFEST_DIR"))
//! }
//! ```
//!
//! [`run`] prépare les pages, les sert, et fait vivre la machine : le temps,
//! ce que les pages envoient, ce qu'elles doivent montrer, le journal et
//! l'état gardé. Tout ce qui se décide est dans [`Session`], sans réseau ni
//! fichier : `run` ne fait que la brancher sur la salle, l'horloge et le
//! disque.

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher as _, RandomState};
use std::io::{self, BufWriter, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use cartouche::journal::Line;
use cartouche::{Input, Machine, Page, fingerprint, take_notes};

use crate::host::{Event, Id, Room};
use crate::{Welcome, Wire};

/// Le plus court écart entre deux vues envoyées à une page : au plus soixante
/// par seconde (arrondi au-dessus, pour ne jamais en envoyer soixante et une).
const VIEW_GAP: Duration = Duration::from_nanos(16_666_667);
/// Le plus court écart entre deux écritures de l'état gardé.
const SAVE_GAP: Duration = Duration::from_secs(1);
/// L'attente quand rien n'est prévu : le jeu attend les pages.
const IDLE: Duration = Duration::from_secs(1);
/// Sans rythme, un repère toutes les tant d'entrées.
const MARK_EVERY: u64 = 50;
/// Au-delà, les plus vieux journaux s'effacent au lancement.
const JOURNAL_LIMIT: u64 = 200_000_000;
const NANOS_PER_SECOND: u128 = 1_000_000_000;

/// Fait tourner le jeu dont la machine est `M`. Ne rend la main que si le jeu
/// ne peut pas démarrer (les pages ne compilent pas, le port est pris).
///
/// `manifest_dir` est le dossier du crate qui appelle :
/// `env!("CARGO_MANIFEST_DIR")`. Le crate des pages s'appelle `pages` ; la
/// télécommande est servie à `/t`, l'écran à `/`.
pub fn run<M: Machine>(manifest_dir: &str) -> ExitCode {
    match serve::<M>(Path::new(manifest_dir)) {
        Ok(never) => match never {},
        Err(error) => {
            eprintln!("Le jeu ne peut pas démarrer : {error}");
            ExitCode::FAILURE
        }
    }
}

fn serve<M: Machine>(manifest_dir: &Path) -> io::Result<Infallible> {
    let pages = crate::pages::build(manifest_dir, "pages")?;
    let workspace = workspace_root(manifest_dir);
    let folder = workspace.join(".soiree");
    let game = workspace.file_name().map_or_else(
        || "jeu".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let launched = SystemTime::now();
    let stamp = stamp(launched);
    let seed = seed(launched);
    let woken = if M::REMEMBERS {
        wake::<M>(&folder, seed, &stamp)
    } else {
        Woken {
            machine: M::new(seed),
            kept: None,
            keep: Keep::No,
        }
    };
    let first_tick = woken.kept.as_ref().map_or(0, |kept| kept.ticks);
    let room = Room::open(&pages, "/t")?;
    let mut journal = Journal::open(&folder.join("journal"), &stamp);
    journal.write(&[Line::start(
        &game,
        seed,
        M::TICKS_PER_SECOND,
        woken
            .kept
            .as_ref()
            .map(|kept| (kept.state.as_slice(), kept.ticks)),
    )]);
    journal.flush();
    let mut keeper = match woken.keep {
        Keep::Yes => Some(Keeper::new(
            folder.join("etat"),
            woken.kept.map(|kept| kept.file()),
        )),
        Keep::No => None,
    };
    let welcome = Welcome {
        join_url: room.remote_url().to_owned(),
        ticks_per_second: M::TICKS_PER_SECOND,
    };
    let mut session = Session::new(woken.machine, &welcome, Instant::now(), first_tick);
    loop {
        let wake = [
            session.next_wake(),
            keeper.as_ref().and_then(Keeper::next_wake),
        ]
        .into_iter()
        .flatten()
        .min();
        let wait = wake.map_or(IDLE, |wake| wake.saturating_duration_since(Instant::now()));
        let before = session.ticks();
        let event = room.next(wait);
        let now = Instant::now();
        let mut out = match &event {
            Some(Event::Joined(id, role)) => session.joined(
                Page {
                    id: id.0,
                    role: *role,
                },
                now,
            ),
            Some(Event::Text(id, text)) => session.message(id.0, text, now),
            Some(Event::Left(id)) => session.left(id.0, now),
            None => Out::default(),
        };
        out.extend(session.advance(now));
        deliver(&room, &mut journal, out);
        if let Some(keeper) = &mut keeper {
            if event.is_some() || session.ticks() != before {
                keeper.touch();
            }
            keeper.keep(now, || {
                Kept {
                    ticks: session.ticks(),
                    state: session.machine().save(),
                }
                .file()
            });
        }
        // Rien n'est prévu : le jeu attend. Ce qui est écrit doit être sur le
        // disque si on l'arrête maintenant.
        if event.is_none() && wake.is_none() {
            journal.write(session.settle().as_slice());
            journal.flush();
        }
    }
}

/// Porte ce que la session a décidé : aux pages, au journal, sur stderr.
fn deliver(room: &Room, journal: &mut Journal, out: Out) {
    for warning in &out.warnings {
        eprintln!("{warning}");
    }
    for (page, text) in out.sends {
        room.send(Id(page.id), text);
    }
    journal.write(&out.lines);
    if out
        .lines
        .iter()
        .any(|line| matches!(line, Line::Mark { .. }))
    {
        journal.flush();
    }
}

/// Ce qu'il faut faire après un événement.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Out {
    /// Les textes à envoyer, et à qui, dans l'ordre.
    pub(crate) sends: Vec<(Page, String)>,
    /// Les lignes à écrire au journal, dans l'ordre.
    pub(crate) lines: Vec<Line>,
    /// Ce qu'il faut dire sur stderr.
    pub(crate) warnings: Vec<String>,
}

impl Out {
    fn extend(&mut self, other: Out) {
        self.sends.extend(other.sends);
        self.lines.extend(other.lines);
        self.warnings.extend(other.warnings);
    }
}

/// Une page présente, et ce qu'on lui a envoyé.
struct Present {
    page: Page,
    /// La dernière vue envoyée, écrite.
    sent: Option<String>,
    /// Quand elle est partie.
    sent_at: Option<Instant>,
    /// La vue à envoyer dès que l'écart le permet.
    pending: Option<String>,
    /// Elle vient d'arriver : sa vue part tout de suite, même inchangée.
    fresh: bool,
}

impl Present {
    fn new(page: Page) -> Self {
        Self {
            page,
            sent: None,
            sent_at: None,
            pending: None,
            fresh: true,
        }
    }

    /// Une vue vient d'être calculée pour cette page.
    fn offer(&mut self, view: String) {
        if !self.fresh && self.sent.as_ref() == Some(&view) {
            self.pending = None;
        } else {
            self.pending = Some(view);
        }
    }

    /// Le moment où la vue en attente pourra partir, si elle doit attendre
    /// la fin de l'écart. `None` : rien n'attend, ou elle peut partir tout de
    /// suite.
    fn ready_at(&self) -> Option<Instant> {
        self.pending.as_ref()?;
        match self.sent_at {
            Some(at) if !self.fresh => at.checked_add(VIEW_GAP),
            Some(_) | None => None,
        }
    }
}

/// Le temps du jeu : à rythme fixe, sans dérive.
struct Clock {
    start: Instant,
    ticks_per_second: u32,
    /// Les pas que la machine avait déjà reçus avant ce lancement (un état
    /// gardé ne repart pas de zéro).
    base: u64,
    /// Les pas joués depuis ce lancement.
    played: u64,
    /// Les pas laissés tomber après un ralentissement.
    dropped: u64,
}

impl Clock {
    /// Les pas qui devraient être passés à `now`, depuis le début.
    fn due(&self, now: Instant) -> u64 {
        let elapsed = now.saturating_duration_since(self.start).as_nanos();
        let due = elapsed.saturating_mul(u128::from(self.ticks_per_second)) / NANOS_PER_SECOND;
        u64::try_from(due).unwrap_or(u64::MAX)
    }

    /// Le moment du pas `tick`, compté depuis le début : calculé depuis le
    /// début, pas ajouté au précédent, pour que les erreurs ne s'accumulent
    /// pas. Arrondi au-dessus : à ce moment, [`Clock::due`] le compte.
    fn at(&self, tick: u64) -> Option<Instant> {
        let rate = u128::from(self.ticks_per_second);
        let nanos = u128::from(tick)
            .saturating_mul(NANOS_PER_SECOND)
            .checked_add(rate.checked_sub(1)?)?
            .checked_div(rate)?;
        self.start
            .checked_add(Duration::from_nanos(u64::try_from(nanos).ok()?))
    }

    /// Le moment du prochain pas. `None` sans rythme.
    fn next_at(&self) -> Option<Instant> {
        self.at(self.played.saturating_add(self.dropped).saturating_add(1))
    }

    /// Le nombre de pas à jouer maintenant. Après un ralentissement, au plus
    /// un dixième de seconde : le reste est laissé tomber.
    fn catch_up(&mut self, now: Instant) -> u64 {
        let behind = self
            .due(now)
            .saturating_sub(self.played.saturating_add(self.dropped));
        let most = u64::from(self.ticks_per_second / 10).max(1);
        if behind > most {
            self.dropped = self.dropped.saturating_add(behind.saturating_sub(most));
            most
        } else {
            behind
        }
    }
}

/// Ce qui s'est passé depuis le dernier repère du journal.
#[derive(Default)]
struct SinceMark {
    inputs: u64,
    lines: u64,
}

/// La partie en cours, sans réseau ni fichier : ce qu'on fait de chaque
/// événement et du temps qui passe, ce qu'on envoie à qui, ce qu'on écrit au
/// journal.
pub(crate) struct Session<M: Machine> {
    machine: M,
    clock: Clock,
    pages: BTreeMap<u64, Present>,
    /// Le message d'accueil, écrit une fois pour toutes.
    welcome: String,
    since_mark: SinceMark,
    out: Out,
}

impl<M: Machine> Session<M> {
    /// Une session qui commence à `start`, sans page.
    pub(crate) fn new(machine: M, welcome: &Welcome, start: Instant, first_tick: u64) -> Self {
        let welcome =
            serde_json::to_string(&Wire::<(), ()>::Welcome(welcome.clone())).unwrap_or_default();
        Self {
            machine,
            clock: Clock {
                start,
                ticks_per_second: M::TICKS_PER_SECOND,
                base: first_tick,
                played: 0,
                dropped: 0,
            },
            pages: BTreeMap::new(),
            welcome,
            since_mark: SinceMark::default(),
            out: Out::default(),
        }
    }

    pub(crate) fn machine(&self) -> &M {
        &self.machine
    }

    /// Le numéro du pas : tous les pas que la machine a reçus depuis sa
    /// naissance, y compris avant un arrêt quand la console garde son état.
    pub(crate) fn ticks(&self) -> u64 {
        self.clock.base.saturating_add(self.clock.played)
    }

    /// Le jeu se tait : un repère, si quelque chose s'est écrit depuis le
    /// dernier. Un journal coupé là se vérifie jusqu'au bout.
    pub(crate) fn settle(&mut self) -> Option<Line> {
        (self.since_mark.lines > 0).then(|| {
            self.since_mark = SinceMark::default();
            Line::mark(self.ticks(), fingerprint(&self.machine))
        })
    }

    /// Une page arrive, ou revient. Elle reçoit l'accueil, puis sa vue.
    pub(crate) fn joined(&mut self, page: Page, now: Instant) -> Out {
        let _: Option<Present> = self.pages.insert(page.id, Present::new(page));
        self.out.sends.push((page, self.welcome.clone()));
        self.write(Line::Joined {
            tick: self.ticks(),
            page,
        });
        self.apply(Input::Joined(page));
        self.after_input(now)
    }

    /// La page `id` est partie. Une page inconnue ne change rien.
    pub(crate) fn left(&mut self, id: u64, now: Instant) -> Out {
        let Some(present) = self.pages.remove(&id) else {
            return std::mem::take(&mut self.out);
        };
        let page = present.page;
        self.write(Line::Left {
            tick: self.ticks(),
            page,
        });
        self.apply(Input::Left(page));
        self.after_input(now)
    }

    /// La page `id` a envoyé `text`. Une commande illisible n'entre ni dans
    /// la machine ni dans le journal : on le dit, et c'est tout.
    pub(crate) fn message(&mut self, id: u64, text: &str, now: Instant) -> Out {
        let Some(page) = self.pages.get(&id).map(|present| present.page) else {
            self.out
                .warnings
                .push(format!("message d'une page inconnue ({id}) : ignoré"));
            return std::mem::take(&mut self.out);
        };
        let read = serde_json::from_str::<serde_json::Value>(text).and_then(|command| {
            serde_json::from_value::<M::Command>(command.clone()).map(|parsed| (command, parsed))
        });
        let (command, parsed) = match read {
            Ok(read) => read,
            Err(error) => {
                self.out.warnings.push(format!(
                    "commande illisible de la page {id} ({}) : {error} ; reçu : {text}",
                    page.role.as_str()
                ));
                return std::mem::take(&mut self.out);
            }
        };
        self.write(Line::Message {
            tick: self.ticks(),
            page,
            command,
        });
        self.apply(Input::Message(page, parsed));
        self.after_input(now)
    }

    /// Le temps a passé jusqu'à `now` : joue les pas qui sont dus, et envoie
    /// les vues qui attendaient leur tour.
    pub(crate) fn advance(&mut self, now: Instant) -> Out {
        let due = self.clock.catch_up(now);
        let rate = u64::from(M::TICKS_PER_SECOND);
        for _ in 0..due {
            self.apply(Input::Tick);
            self.clock.played = self.clock.played.saturating_add(1);
            if rate > 0 && self.clock.played.is_multiple_of(rate) && self.since_mark.lines > 0 {
                self.mark();
            }
        }
        if due > 0 {
            self.refresh();
        }
        self.flush_views(now);
        std::mem::take(&mut self.out)
    }

    /// Le prochain moment où il y a quelque chose à faire : un pas, ou une
    /// vue qui attend. `None` : rien tant qu'une page ne parle pas.
    pub(crate) fn next_wake(&self) -> Option<Instant> {
        self.pages
            .values()
            .filter_map(Present::ready_at)
            .chain(self.clock.next_at())
            .min()
    }

    fn after_input(&mut self, now: Instant) -> Out {
        self.since_mark.inputs = self.since_mark.inputs.saturating_add(1);
        if M::TICKS_PER_SECOND == 0 && self.since_mark.inputs >= MARK_EVERY {
            self.mark();
        }
        self.refresh();
        self.flush_views(now);
        std::mem::take(&mut self.out)
    }

    /// Passe une entrée à la machine, porte ses effets et lit ses notes.
    fn apply(&mut self, input: Input<M::Command>) {
        for (to, effect) in self.machine.apply(input) {
            match serde_json::to_string(&Wire::<(), M::Effect>::Effect(effect)) {
                Ok(text) => {
                    for present in self.pages.values() {
                        if to.reaches(present.page) {
                            self.out.sends.push((present.page, text.clone()));
                        }
                    }
                }
                Err(error) => self
                    .out
                    .warnings
                    .push(format!("effet impossible à écrire : {error}")),
            }
        }
        if let Some(note) = take_notes() {
            self.out.warnings.push(format!(
                "oups : {} ({}:{}), {} fois",
                note.what, note.file, note.line, note.count
            ));
            self.write(Line::Note {
                tick: self.ticks(),
                what: note.what.to_owned(),
                file: note.file.to_owned(),
                line: note.line,
                count: note.count,
            });
        }
    }

    fn write(&mut self, line: Line) {
        self.out.lines.push(line);
        self.since_mark.lines = self.since_mark.lines.saturating_add(1);
    }

    fn mark(&mut self) {
        self.out
            .lines
            .push(Line::mark(self.ticks(), fingerprint(&self.machine)));
        self.since_mark = SinceMark::default();
    }

    /// Calcule la vue de chaque page présente.
    fn refresh(&mut self) {
        for present in self.pages.values_mut() {
            let Some(view) = self.machine.view(present.page) else {
                present.fresh = false;
                present.pending = None;
                continue;
            };
            match serde_json::to_string(&view) {
                Ok(view) => present.offer(view),
                Err(error) => self
                    .out
                    .warnings
                    .push(format!("vue impossible à écrire : {error}")),
            }
        }
    }

    /// Envoie les vues en attente dont l'écart est passé.
    fn flush_views(&mut self, now: Instant) {
        let tick = self.ticks();
        for present in self.pages.values_mut() {
            if present.ready_at().is_some_and(|at| now < at) {
                continue;
            }
            let Some(view) = present.pending.take() else {
                continue;
            };
            self.out.sends.push((present.page, view_text(tick, &view)));
            present.sent = Some(view);
            present.sent_at = Some(now);
            present.fresh = false;
        }
    }
}

/// Le message d'une vue déjà écrite : celui de [`Wire::View`], sans écrire la
/// vue une deuxième fois.
fn view_text(tick: u64, view: &str) -> String {
    format!("{{\"view\":{{\"tick\":{tick},\"view\":{view}}}}}")
}

/// Garder l'état ou non.
enum Keep {
    Yes,
    No,
}

/// La machine du lancement, et l'état gardé qu'elle a repris.
struct Woken<M> {
    machine: M,
    /// L'état repris, s'il y en avait un.
    kept: Option<Kept>,
    /// Non quand l'état gardé n'a pas pu être mis de côté : on ne l'écrase pas.
    keep: Keep,
}

/// Reprend l'état gardé dans `folder`, s'il y en a un. Un état qui ne se
/// relit pas est mis de côté, jamais écrasé ni effacé, et la partie part de
/// la graine.
fn wake<M: Machine>(folder: &Path, seed: u64, stamp: &str) -> Woken<M> {
    let path = folder.join("etat");
    let fresh = |keep| Woken {
        machine: M::new(seed),
        kept: None,
        keep,
    };
    let why = match fs::read(&path) {
        Ok(bytes) => {
            match Kept::read(&bytes).and_then(|kept| Some((M::load(&kept.state)?, kept))) {
                Some((machine, kept)) => {
                    println!("État repris : {}", path.display());
                    return Woken {
                        machine,
                        kept: Some(kept),
                        keep: Keep::Yes,
                    };
                }
                None => "il ne se relit pas avec cette version du jeu".to_owned(),
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => return fresh(Keep::Yes),
        Err(error) => format!("il ne se lit pas ({error})"),
    };
    match set_aside(&path, stamp) {
        Ok(aside) => {
            eprintln!(
                "L'état gardé ({}) est mis de côté : {why}. Il est dans {}. La partie repart \
                 de zéro.",
                path.display(),
                aside.display()
            );
            fresh(Keep::Yes)
        }
        Err(error) => {
            eprintln!(
                "L'état gardé ({}) ne peut pas être repris ({why}) ni mis de côté ({error}). La \
                 partie repart de zéro, et cet état ne sera pas écrasé : rien ne sera gardé.",
                path.display()
            );
            fresh(Keep::No)
        }
    }
}

/// Ce que la console garde d'un lancement à l'autre : l'état de la machine,
/// et le nombre de pas qu'elle a reçus (le numéro du pas ne repart pas de zéro).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Kept {
    ticks: u64,
    state: Vec<u8>,
}

impl Kept {
    /// Le fichier : le nombre de pas sur huit octets, puis l'état.
    fn file(&self) -> Vec<u8> {
        let mut bytes = self.ticks.to_le_bytes().to_vec();
        bytes.extend_from_slice(&self.state);
        bytes
    }

    fn read(bytes: &[u8]) -> Option<Self> {
        let (ticks, state) = bytes.split_first_chunk::<8>()?;
        Some(Self {
            ticks: u64::from_le_bytes(*ticks),
            state: state.to_vec(),
        })
    }
}

/// Renomme `path` en `<nom>.illisible-<date>`, sans jamais écraser un fichier.
fn set_aside(path: &Path, stamp: &str) -> io::Result<PathBuf> {
    let name = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    for rank in 1_u32..1000 {
        let aside = if rank == 1 {
            path.with_file_name(format!("{name}.illisible-{stamp}"))
        } else {
            path.with_file_name(format!("{name}.illisible-{stamp}-{rank}"))
        };
        if !aside.exists() {
            fs::rename(path, &aside)?;
            return Ok(aside);
        }
    }
    Err(io::Error::other("trop d'états déjà mis de côté"))
}

/// Écrit l'état gardé quand il change, au plus une fois par seconde, et une
/// dernière fois quand tout est calme.
struct Keeper {
    path: PathBuf,
    /// Les octets sur le disque.
    saved: Vec<u8>,
    /// La partie a peut-être changé depuis la dernière écriture.
    touched: bool,
    last: Option<Instant>,
    failed: bool,
}

impl Keeper {
    fn new(path: PathBuf, saved: Option<Vec<u8>>) -> Self {
        Self {
            path,
            saved: saved.unwrap_or_default(),
            touched: false,
            last: None,
            failed: false,
        }
    }

    fn touch(&mut self) {
        self.touched = true;
    }

    /// Le moment de la prochaine écriture possible, s'il y a lieu.
    fn next_wake(&self) -> Option<Instant> {
        if !self.touched {
            return None;
        }
        self.last.and_then(|last| last.checked_add(SAVE_GAP))
    }

    fn keep(&mut self, now: Instant, file: impl FnOnce() -> Vec<u8>) {
        if !self.touched {
            return;
        }
        if let Some(last) = self.last
            && last.checked_add(SAVE_GAP).is_none_or(|next| now < next)
        {
            return;
        }
        self.touched = false;
        self.last = Some(now);
        let bytes = file();
        if bytes == self.saved {
            return;
        }
        match write_whole(&self.path, &bytes) {
            Ok(()) => {
                self.saved = bytes;
                self.failed = false;
            }
            Err(error) => {
                if !self.failed {
                    eprintln!(
                        "L'état ne s'écrit pas dans {} : {error}. Le jeu continue.",
                        self.path.display()
                    );
                }
                self.failed = true;
            }
        }
    }
}

/// Écrit dans un fichier à côté, puis le renomme : un arrêt en plein milieu
/// ne laisse jamais un fichier à moitié écrit.
fn write_whole(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    let mut file = File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)
}

/// Le journal du lancement. Une erreur ne l'arrête jamais le jeu : on la dit
/// une fois, et le journal s'arrête là.
struct Journal {
    path: PathBuf,
    file: Option<BufWriter<File>>,
}

impl Journal {
    fn open(folder: &Path, stamp: &str) -> Self {
        let opened = fs::create_dir_all(folder).and_then(|()| {
            if let Err(error) = prune(folder, JOURNAL_LIMIT) {
                eprintln!(
                    "Les vieux journaux ne s'effacent pas ({}) : {error}",
                    folder.display()
                );
            }
            create_new(folder, stamp)
        });
        match opened {
            Ok((path, file)) => {
                println!("Journal     : {}", path.display());
                Self {
                    path,
                    file: Some(BufWriter::new(file)),
                }
            }
            Err(error) => {
                eprintln!(
                    "Le journal ne s'ouvre pas dans {} : {error}. Le jeu continue sans lui.",
                    folder.display()
                );
                Self {
                    path: folder.to_owned(),
                    file: None,
                }
            }
        }
    }

    fn write(&mut self, lines: &[Line]) {
        let Some(file) = &mut self.file else {
            return;
        };
        let written = lines
            .iter()
            .try_for_each(|line| writeln!(file, "{}", line.write()));
        if let Err(error) = written {
            self.fail(&error);
        }
    }

    fn flush(&mut self) {
        if let Some(file) = &mut self.file
            && let Err(error) = file.flush()
        {
            self.fail(&error);
        }
    }

    fn fail(&mut self, error: &io::Error) {
        eprintln!(
            "Le journal ne s'écrit plus ({}) : {error}. Le jeu continue sans lui.",
            self.path.display()
        );
        self.file = None;
    }
}

/// Crée `<date>.jsonl` dans `folder`, ou `<date>-2.jsonl`… s'il existe déjà.
fn create_new(folder: &Path, stamp: &str) -> io::Result<(PathBuf, File)> {
    for rank in 1_u32..1000 {
        let path = if rank == 1 {
            folder.join(format!("{stamp}.jsonl"))
        } else {
            folder.join(format!("{stamp}-{rank}.jsonl"))
        };
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other("trop de journaux à la même seconde"))
}

/// Efface les plus vieux fichiers de `folder` jusqu'à ce qu'ils tiennent dans
/// `limit` octets.
fn prune(folder: &Path, limit: u64) -> io::Result<()> {
    let mut files = Vec::new();
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_file() {
            files.push((metadata.modified()?, entry.path(), metadata.len()));
        }
    }
    files.sort();
    let mut total = files
        .iter()
        .fold(0_u64, |total, (_, _, size)| total.saturating_add(*size));
    for (_, path, size) in files {
        if total <= limit {
            break;
        }
        fs::remove_file(path)?;
        total = total.saturating_sub(size);
    }
    Ok(())
}

/// La racine du workspace du jeu : le premier dossier, en remontant, dont le
/// `Cargo.toml` déclare un `[workspace]`. À défaut, le dossier du crate.
fn workspace_root(manifest_dir: &Path) -> PathBuf {
    manifest_dir
        .ancestors()
        .find(|folder| {
            fs::read_to_string(folder.join("Cargo.toml")).is_ok_and(|manifest| {
                manifest
                    .lines()
                    .any(|line| line.trim_start().starts_with("[workspace"))
            })
        })
        .unwrap_or(manifest_dir)
        .to_owned()
}

/// Une graine tirée du hasard du système.
fn seed(now: SystemTime) -> u64 {
    let nanos = now
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    RandomState::new().hash_one(nanos)
}

/// La date en UTC, pour un nom de fichier : `AAAA-MM-JJ_hh-mm-ss`.
fn stamp(now: SystemTime) -> String {
    let seconds = now
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let (year, month, day) = civil(seconds / 86_400);
    let time = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}_{:02}-{:02}-{:02}",
        time / 3600,
        time % 3600 / 60,
        time % 60
    )
}

/// L'année, le mois et le jour d'un nombre de jours depuis le 1er janvier
/// 1970 (l'algorithme `civil_from_days` de Howard Hinnant, sur des entiers
/// positifs : l'année commence en mars, le jour bissextile tombe à la fin).
fn civil(days: u64) -> (u64, u64, u64) {
    let shifted = days.saturating_add(719_468);
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era = day_of_era
        .saturating_sub(day_of_era / 1460)
        .saturating_add(day_of_era / 36_524)
        .saturating_sub(day_of_era / 146_096)
        / 365;
    let day_of_year = day_of_era.saturating_sub(
        year_of_era
            .saturating_mul(365)
            .saturating_add(year_of_era / 4)
            .saturating_sub(year_of_era / 100),
    );
    let month_from_march = day_of_year.saturating_mul(5).saturating_add(2) / 153;
    let day = day_of_year
        .saturating_sub(month_from_march.saturating_mul(153).saturating_add(2) / 5)
        .saturating_add(1);
    let month = if month_from_march < 10 {
        month_from_march.saturating_add(3)
    } else {
        month_from_march.saturating_sub(9)
    };
    let year = year_of_era
        .saturating_add(era.saturating_mul(400))
        .saturating_add(u64::from(month <= 2));
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use cartouche::journal::{read, replay};
    use cartouche::{Role, State, To};
    use serde::{Deserialize, Serialize};

    use super::*;

    /// Une machine de rien : elle compte les pas et les tapes, et bipe les
    /// télécommandes quand on le lui demande.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Toy<const RATE: u32> {
        seed: u64,
        ticks: u64,
        taps: u64,
        here: Vec<Page>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    enum Press {
        Tap,
        Nothing,
        Beep,
    }

    #[derive(Debug, Serialize, Deserialize)]
    enum Sound {
        Beep,
    }

    impl<const RATE: u32> Machine for Toy<RATE> {
        type Command = Press;
        type View = u64;
        type Effect = Sound;
        const TICKS_PER_SECOND: u32 = RATE;

        fn new(seed: u64) -> Self {
            Self {
                seed,
                ticks: 0,
                taps: 0,
                here: Vec::new(),
            }
        }

        fn apply(&mut self, input: Input<Press>) -> Vec<(To, Sound)> {
            match input {
                Input::Joined(page) => {
                    if !self.here.contains(&page) {
                        self.here.push(page);
                    }
                }
                Input::Left(page) => self.here.retain(|one| *one != page),
                Input::Message(_, Press::Tap) => self.taps = self.taps.saturating_add(1),
                Input::Message(_, Press::Nothing) => {}
                Input::Message(_, Press::Beep) => return vec![(To::Remotes, Sound::Beep)],
                Input::Tick => self.ticks = self.ticks.saturating_add(1),
            }
            Vec::new()
        }

        fn view(&self, page: Page) -> Option<u64> {
            self.here.contains(&page).then_some(self.taps)
        }

        fn suggest(&self, _: Page) -> Vec<Press> {
            vec![Press::Tap]
        }
    }

    fn welcome() -> Welcome {
        Welcome {
            join_url: "http://192.0.2.7:8080/t".to_owned(),
            ticks_per_second: 0,
        }
    }

    fn after(start: Instant, millis: u64) -> Instant {
        start.checked_add(Duration::from_millis(millis)).unwrap()
    }

    fn welcome_text() -> String {
        serde_json::to_string(&Wire::<(), ()>::Welcome(welcome())).unwrap()
    }

    fn view(tick: u64, taps: u64) -> String {
        serde_json::to_string(&Wire::<u64, ()>::View { tick, view: taps }).unwrap()
    }

    fn effect() -> String {
        serde_json::to_string(&Wire::<(), Sound>::Effect(Sound::Beep)).unwrap()
    }

    fn to(out: &Out, page: Page) -> Vec<String> {
        out.sends
            .iter()
            .filter(|(to, _)| *to == page)
            .map(|(_, text)| text.clone())
            .collect()
    }

    /// Un dossier vide, propre à ce test.
    fn folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!(
            "telecommande-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn the_rhythm_is_fixed_and_does_not_drift() {
        let start = Instant::now();
        let mut session = Session::new(Toy::<60>::new(1), &welcome(), start, 0);
        for _ in 0..600 {
            let wake = session.next_wake().unwrap();
            let _: Out = session.advance(wake);
        }
        assert_eq!(session.ticks(), 600);
        assert_eq!(session.machine().ticks, 600);
        // Le 601e pas tombe à 601/60 s du début, à la nanoseconde près.
        let next = session.next_wake().unwrap().duration_since(start);
        assert_eq!(next, Duration::from_nanos(10_016_666_667));

        let mut session = Session::new(Toy::<10>::new(1), &welcome(), start, 0);
        let _: Out = session.advance(after(start, 99));
        assert_eq!(session.ticks(), 0);
        let _: Out = session.advance(after(start, 100));
        assert_eq!(session.ticks(), 1);
        let _: Out = session.advance(after(start, 250));
        assert_eq!(session.ticks(), 2);
        assert_eq!(session.next_wake(), Some(after(start, 300)));
    }

    #[test]
    fn after_a_slowdown_only_a_tenth_of_a_second_is_caught_up() {
        let start = Instant::now();
        let mut session = Session::new(Toy::<100>::new(1), &welcome(), start, 0);
        let _: Out = session.advance(after(start, 5000));
        assert_eq!(session.ticks(), 10);
        // Le reste est laissé tomber : le rythme reprend depuis là.
        assert_eq!(session.next_wake(), Some(after(start, 5010)));
        let _: Out = session.advance(after(start, 5010));
        assert_eq!(session.ticks(), 11);
    }

    #[test]
    fn without_a_rhythm_time_does_nothing() {
        let start = Instant::now();
        let mut session = Session::new(Toy::<0>::new(1), &welcome(), start, 0);
        let _: Out = session.advance(after(start, 60_000));
        assert_eq!(session.ticks(), 0);
        assert_eq!(session.next_wake(), None);
    }

    #[test]
    fn a_page_that_arrives_gets_the_welcome_then_its_view_even_unchanged() {
        let start = Instant::now();
        let mut session = Session::new(Toy::<0>::new(1), &welcome(), start, 0);
        let ada = Page::remote(1);
        let out = session.joined(ada, start);
        assert_eq!(to(&out, ada), [welcome_text(), view(0, 0)]);
        let out = session.joined(ada, after(start, 1));
        assert_eq!(to(&out, ada), [welcome_text(), view(0, 0)]);
    }

    #[test]
    fn a_view_is_sent_only_when_it_changes() {
        let start = Instant::now();
        let mut session = Session::new(Toy::<0>::new(1), &welcome(), start, 0);
        let ada = Page::remote(1);
        let _: Out = session.joined(ada, start);
        let out = session.message(1, "\"Nothing\"", after(start, 100));
        assert_eq!(out.sends.len(), 0);
        let out = session.message(1, "\"Tap\"", after(start, 200));
        assert_eq!(out.sends, [(ada, view(0, 1))]);
        // Le texte écrit à la main est bien celui de `Wire::View`.
        let wire: Wire<u64, ()> = serde_json::from_str(&view_text(3, "7")).unwrap();
        assert_eq!(wire, Wire::View { tick: 3, view: 7 });
    }

    #[test]
    fn at_most_sixty_views_a_second_and_the_last_one_wins() {
        let start = Instant::now();
        let mut session = Session::new(Toy::<0>::new(1), &welcome(), start, 0);
        let ada = Page::remote(1);
        let _: Out = session.joined(ada, start);
        let out = session.message(1, "\"Tap\"", after(start, 1));
        assert_eq!(out.sends.len(), 0);
        let out = session.message(1, "\"Tap\"", after(start, 2));
        assert_eq!(out.sends.len(), 0);
        assert_eq!(
            session.next_wake(),
            start.checked_add(Duration::from_nanos(16_666_667))
        );
        assert_eq!(session.advance(after(start, 16)).sends.len(), 0);
        let out = session.advance(after(start, 17));
        assert_eq!(out.sends, [(ada, view(0, 2))]);
        assert_eq!(session.advance(after(start, 40)).sends.len(), 0);
        assert_eq!(session.next_wake(), None);
    }

    #[test]
    fn an_effect_reaches_the_pages_it_names_once() {
        let start = Instant::now();
        let mut session = Session::new(Toy::<0>::new(1), &welcome(), start, 0);
        let (screen, ada, bob) = (Page::screen(10), Page::remote(1), Page::remote(2));
        for page in [screen, ada, bob] {
            let _: Out = session.joined(page, start);
        }
        let out = session.message(1, "\"Beep\"", after(start, 100));
        assert_eq!(to(&out, ada), [effect()]);
        assert_eq!(to(&out, bob), [effect()]);
        assert_eq!(to(&out, screen).len(), 0);
        // Une page partie ne reçoit plus rien.
        let _: Out = session.left(2, after(start, 200));
        let out = session.message(1, "\"Beep\"", after(start, 300));
        assert_eq!(to(&out, bob).len(), 0);
    }

    #[test]
    fn an_unreadable_command_is_ignored() {
        let start = Instant::now();
        let mut session = Session::new(Toy::<0>::new(1), &welcome(), start, 0);
        let _: Out = session.joined(Page::remote(1), start);
        for text in ["\"Jump\"", "pas du json"] {
            let out = session.message(1, text, after(start, 100));
            assert_eq!(out.sends.len(), 0);
            assert_eq!(out.lines.len(), 0);
            assert_eq!(out.warnings.len(), 1);
        }
        let out = session.message(99, "\"Tap\"", after(start, 100));
        assert_eq!(out.lines.len(), 0);
        assert_eq!(session.machine().taps, 0);
    }

    #[test]
    fn the_journal_replays_to_the_same_fingerprint() {
        let start = Instant::now();
        let mut session = Session::new(Toy::<10>::new(42), &welcome(), start, 0);
        let mut lines = vec![Line::start("essai", 42, 10, None)];
        let ada = Page::remote(1);
        lines.extend(session.joined(ada, start).lines);
        let mut millis: u64 = 0;
        while millis < 2000 {
            millis = millis.saturating_add(50);
            if millis == 350 {
                lines.extend(session.message(1, "\"Tap\"", after(start, millis)).lines);
            }
            if millis == 1200 {
                lines.extend(session.left(1, after(start, millis)).lines);
            }
            lines.extend(session.advance(after(start, millis)).lines);
        }
        assert_eq!(session.ticks(), 20);
        let marks: Vec<&Line> = lines
            .iter()
            .filter(|line| matches!(line, Line::Mark { .. }))
            .collect();
        assert_eq!(
            marks,
            [
                &Line::mark(
                    10,
                    fingerprint(&{
                        let mut toy = Toy::<10>::new(42);
                        toy.ticks = 10;
                        toy.taps = 1;
                        toy.here = vec![ada];
                        toy
                    })
                ),
                &Line::mark(20, fingerprint(session.machine())),
            ]
        );
        let text: String = lines.iter().map(|line| line.write() + "\n").collect();
        let (again, done) = replay::<Toy<10>>(&read(&text).unwrap()).unwrap();
        assert_eq!(&again, session.machine());
        assert_eq!((done.inputs, done.marks), (3, 2));
        assert_eq!(done.fingerprint, fingerprint(session.machine()));
    }

    #[test]
    fn without_a_rhythm_a_mark_comes_every_fifty_inputs() {
        let start = Instant::now();
        let mut session = Session::new(Toy::<0>::new(1), &welcome(), start, 0);
        let mut lines = session.joined(Page::remote(1), start).lines;
        for _ in 0..49 {
            lines.extend(session.message(1, "\"Tap\"", start).lines);
        }
        assert_eq!(lines.len(), 51);
        assert_eq!(
            lines.last(),
            Some(&Line::mark(0, fingerprint(session.machine())))
        );
    }

    #[test]
    fn a_kept_state_is_read_back() {
        let folder = folder("etat-relu");
        let woken = wake::<Toy<0>>(&folder, 3, "2026-11-17_20-00-00");
        assert_eq!(woken.machine, Toy::<0>::new(3));
        assert_eq!(woken.kept, None);

        let mut keeper = Keeper::new(folder.join("etat"), None);
        let mut toy = Toy::<0>::new(5);
        toy.taps = 4;
        // Ce qui est gardé : l'état, et le numéro du pas où il en est.
        let kept = |toy: &Toy<0>, ticks| Kept {
            ticks,
            state: toy.save(),
        };
        let start = Instant::now();
        keeper.touch();
        keeper.keep(start, || kept(&toy, 7).file());
        assert_eq!(fs::read(folder.join("etat")).unwrap(), kept(&toy, 7).file());
        // Pas plus d'une écriture par seconde : la suivante attend son tour.
        toy.taps = 5;
        keeper.touch();
        keeper.keep(after(start, 500), || kept(&toy, 9).file());
        assert_ne!(fs::read(folder.join("etat")).unwrap(), kept(&toy, 9).file());
        assert_eq!(keeper.next_wake(), Some(after(start, 1000)));
        keeper.keep(after(start, 1000), || kept(&toy, 9).file());
        assert_eq!(fs::read(folder.join("etat")).unwrap(), kept(&toy, 9).file());
        assert_eq!(keeper.next_wake(), None);
        assert!(!folder.join("etat.tmp").exists());

        // Relu, l'état reprend au pas où il en était.
        let woken = wake::<Toy<0>>(&folder, 3, "2026-11-17_20-00-00");
        assert_eq!(woken.machine, toy);
        assert_eq!(woken.kept, Some(kept(&toy, 9)));
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn an_unreadable_state_is_set_aside_never_overwritten() {
        let folder = folder("etat-illisible");
        for aside in [
            "etat.illisible-2026-11-17_20-00-00",
            "etat.illisible-2026-11-17_20-00-00-2",
        ] {
            fs::write(folder.join("etat"), [0xff]).unwrap();
            let woken = wake::<Toy<0>>(&folder, 3, "2026-11-17_20-00-00");
            assert_eq!(woken.machine, Toy::<0>::new(3));
            assert_eq!(woken.kept, None);
            assert!(!folder.join("etat").exists());
            assert_eq!(fs::read(folder.join(aside)).unwrap(), [0xff]);
        }
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn old_journals_are_erased_first() {
        let folder = folder("journaux");
        for name in ["a.jsonl", "b.jsonl", "c.jsonl"] {
            fs::write(folder.join(name), [0; 10]).unwrap();
            std::thread::sleep(Duration::from_millis(20));
        }
        prune(&folder, 25).unwrap();
        assert!(!folder.join("a.jsonl").exists());
        assert!(folder.join("b.jsonl").exists());
        let (path, _) = create_new(&folder, "b").unwrap();
        assert_eq!(path, folder.join("b-2.jsonl"));
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn dates_are_written_in_utc() {
        let at = |seconds| {
            stamp(
                UNIX_EPOCH
                    .checked_add(Duration::from_secs(seconds))
                    .unwrap(),
            )
        };
        assert_eq!(at(0), "1970-01-01_00-00-00");
        assert_eq!(at(951_782_400), "2000-02-29_00-00-00");
        assert_eq!(at(1_700_000_000), "2023-11-14_22-13-20");
        assert_eq!(at(4_107_542_399), "2100-02-28_23-59-59");
    }

    #[test]
    fn the_workspace_root_is_found_upwards() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(workspace_root(manifest), manifest.parent().unwrap());
        assert_eq!(Role::Remote.as_str(), "remote");
    }
}
