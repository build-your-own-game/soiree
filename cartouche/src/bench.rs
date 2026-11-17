//! Le banc : la console joue des parties avec la machine d'un jeu, sans une
//! ligne écrite par le jeu.
//!
//! Des écrans et des téléphones arrivent et partent, envoient des commandes
//! suggérées par la machine ou tirées au hasard, le temps passe par rafales.
//! En route, le banc photographie la partie et la reprend depuis la photo,
//! calcule les vues de toutes les pages et mesure le temps de chaque pas.
//!
//! L'exemple `banc` de la machine n'est que `cartouche::bench::main::<Jeu>()`.
//! `soiree check` le lance :
//!
//! - sans argument, en natif deux fois puis en WebAssembly : les lignes
//!   d'empreinte doivent être les mêmes partout ;
//! - avec `--long`, en natif : plus de parties, la reprise depuis la photo
//!   comparée à la partie jouée d'une traite, et le temps de chaque pas.
//!
//! `--graine <n>` rejoue une partie en racontant chaque entrée ;
//! `--rejouer <fichier>` rejoue le journal d'une partie de soirée.

use std::env;
use std::format;
use std::process::ExitCode;
use std::string::{String, ToString};
use std::sync::atomic::{AtomicU64, Ordering};
use std::vec::Vec;
use std::{eprintln, println};

use serde::Serialize;

use crate::draw::{self, Cause};
use crate::journal::{self, Strayed};
use crate::rng::Rng;
use crate::{Input, Machine, Page, Role, fingerprint};

/// Combien de parties, et jusqu'où va chacune.
#[derive(Clone, Copy)]
struct Plan {
    parties: u64,
    /// Les entrées des pages, par partie.
    inputs: u64,
    /// Le temps de jeu, au plus, par partie.
    seconds: u64,
}

/// Ce que `soiree check` compare entre l'ordinateur et WebAssembly. En
/// WebAssembly, la machine tourne dans un interpréteur : on reste court.
const SHORT: Plan = Plan {
    parties: 6,
    inputs: 240,
    seconds: 300,
};

/// Le banc d'essai complet, en natif.
const LONG: Plan = Plan {
    parties: 48,
    inputs: 600,
    seconds: 900,
};

/// Une ligne d'empreinte toutes les tant d'entrées.
const STRIDE: u64 = 40;
/// Au plus tant d'écrans et de télécommandes à la fois.
const SCREENS_MAX: usize = 2;
const REMOTES_MAX: usize = 8;
/// Un jeu sans télécommande se joue à plusieurs écrans : au plus tant.
const SCREENS_ALONE_MAX: usize = 4;
/// Un moment d'attention dure au plus tant de secondes, et tant de commandes.
const ATTENTION_SECONDS: u64 = 10;
const ATTENTION_COMMANDS: u8 = 12;
/// Un jeu sans rythme a ce temps-là pour répondre à une entrée, en microsecondes.
const UNPACED_BUDGET: u64 = 50_000;

/// Reprendre la partie depuis sa photo en cours de route, ou la jouer d'une traite.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Resume {
    Yes,
    No,
}

/// Raconter chaque entrée, ou seulement les repères.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Talk {
    Marks,
    Everything,
}

/// Ce qui refuse le jeu, prêt à lire.
struct Failure(String);

/// Où en est la partie en cours : ce qu'on dit quand la machine s'arrête.
#[derive(Default)]
struct Progress {
    seed: AtomicU64,
    input: AtomicU64,
    tick: AtomicU64,
}

/// Le temps que prennent les pas.
#[derive(Default)]
struct Timing {
    steps: u64,
    slow: u64,
    worst: u64,
}

impl Timing {
    /// Ce qu'un pas a le droit de prendre, en microsecondes : la moitié de
    /// son temps.
    fn budget<M: Machine>() -> u64 {
        500_000_u64
            .checked_div(u64::from(M::TICKS_PER_SECOND))
            .unwrap_or(UNPACED_BUDGET)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn record(&mut self, micros: u64, budget: u64) {
        self.steps = self.steps.saturating_add(1);
        self.worst = self.worst.max(micros);
        if micros > budget {
            self.slow = self.slow.saturating_add(1);
        }
    }
}

/// Mesure un travail. En WebAssembly, l'horloge n'existe pas : on ne mesure rien.
#[cfg(not(target_arch = "wasm32"))]
fn timed<T>(timing: &mut Timing, budget: u64, work: impl FnOnce() -> T) -> T {
    let start = std::time::Instant::now();
    let done = work();
    let micros = u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX);
    timing.record(micros, budget);
    done
}

#[cfg(target_arch = "wasm32")]
fn timed<T>(_: &mut Timing, _: u64, work: impl FnOnce() -> T) -> T {
    work()
}

/// Une empreinte qui se nourrit au fil de l'eau (FNV-1a sur 64 bits).
struct Print(u64);

impl Print {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn feed(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
        // Une borne entre deux textes : « ab » puis « c » n'est pas « a » puis « bc ».
        self.0 = (self.0 ^ 0xff).wrapping_mul(0x0000_0100_0000_01b3);
    }
}

/// Ce qu'une partie jouée laisse.
struct Outcome {
    lines: Vec<String>,
    inputs: u64,
    ticks: u64,
    /// Les commandes que la machine a suggérées et que le banc a envoyées.
    suggested: u64,
}

/// Une partie en cours.
struct Party<'bench, M: Machine> {
    seed: u64,
    machine: M,
    rng: Rng,
    plan: Plan,
    resume: Resume,
    talk: Talk,
    /// Les pages présentes, et toutes celles qu'on a vues.
    present: Vec<Page>,
    known: Vec<Page>,
    inputs: u64,
    ticks: u64,
    suggested: u64,
    /// L'empreinte de tout ce que les pages ont reçu : vues et effets.
    shown: Print,
    lines: Vec<String>,
    progress: &'bench Progress,
    timing: &'bench mut Timing,
}

/// Le programme de l'exemple `banc` d'une machine.
pub fn main<M: Machine>() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let outcome = match arguments.as_slice() {
        [] => short::<M>(),
        ["--long"] => long::<M>(),
        ["--graine", seed] => match seed.parse() {
            Ok(seed) => one::<M>(seed),
            Err(_) => Err(Failure(format!(
                "`{seed}` n'est pas une graine : un nombre."
            ))),
        },
        ["--rejouer", file] => replay::<M>(file),
        _ => Err(Failure(
            "Le banc se lance sans argument, avec `--long`, avec `--graine <n>` ou avec \
             `--rejouer <fichier>`."
                .to_string(),
        )),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure(why)) => {
            eprintln!("\n{why}");
            ExitCode::FAILURE
        }
    }
}

/// Les parties que `soiree check` compare d'une machine à l'autre.
fn short<M: Machine>() -> Result<(), Failure> {
    let progress = Progress::default();
    let mut timing = Timing::default();
    for seed in 0..SHORT.parties {
        let played = guarded(&progress, || {
            play::<M>(
                seed,
                SHORT,
                Resume::Yes,
                Talk::Marks,
                &progress,
                &mut timing,
            )
        })?;
        for line in played.lines {
            println!("{line}");
        }
    }
    Ok(())
}

/// Le banc d'essai complet.
fn long<M: Machine>() -> Result<(), Failure> {
    let progress = Progress::default();
    let mut timing = Timing::default();
    let (mut inputs, mut ticks, mut suggested) = (0_u64, 0_u64, 0_u64);
    for seed in 0..LONG.parties {
        let resumed = guarded(&progress, || {
            play::<M>(seed, LONG, Resume::Yes, Talk::Marks, &progress, &mut timing)
        })?;
        let straight = guarded(&progress, || {
            play::<M>(seed, LONG, Resume::No, Talk::Marks, &progress, &mut timing)
        })?;
        if let Some((straight, resumed)) = straight
            .lines
            .iter()
            .zip(&resumed.lines)
            .find(|(straight, resumed)| straight != resumed)
        {
            return Err(Failure(format!(
                "Reprise depuis sa photo, la partie {seed} ne continue pas à l'identique.\n  \
                 d'une traite : {straight}\n  \
                 reprise      : {resumed}\n\
                 L'état de la machine ne tient pas tout entier dans sa photo : un champ que \
                 `serde` saute (`#[serde(skip)]`), un état gardé hors de la machine (une \
                 variable globale), ou un `save`/`load` écrit à la main qui oublie quelque chose.\n\
                 Pour revoir la partie : {}",
                again(seed)
            )));
        }
        inputs = inputs.saturating_add(resumed.inputs);
        ticks = ticks.saturating_add(resumed.ticks);
        suggested = suggested.saturating_add(resumed.suggested);
    }
    println!(
        "{} parties, {inputs} entrées, {ticks} pas ; {suggested} commandes suggérées par la machine.",
        LONG.parties
    );
    if suggested == 0 {
        return Err(Failure(
            "La machine ne suggère jamais rien : `suggest` rend toujours une liste vide.\n\
             Sans suggestions, les parties du banc n'avancent pas : il ne tire que des commandes \
             au hasard, presque toutes refusées. `suggest(page)` rend ce que les boutons de cette \
             page enverraient maintenant (lancer la partie, répondre, voter, frapper…)."
                .to_string(),
        ));
    }
    let budget = Timing::budget::<M>();
    println!(
        "Le pas le plus lent : {} (un pas a droit à {}).",
        millis(timing.worst),
        millis(budget)
    );
    if timing.slow.saturating_mul(100) > timing.steps {
        return Err(Failure(format!(
            "La machine est trop lente : {} mesures sur {} dépassent {}, la moitié du temps d'un \
             pas (la plus lente : {}).\n\
             Un pas qui déborde, et tout le jeu ralentit pour tout le monde. Cherche ce qui \
             coûte : un calcul refait à chaque pas, une vue trop grosse à écrire, une boucle qui \
             grandit avec la partie.",
            timing.slow,
            timing.steps,
            millis(budget),
            millis(timing.worst)
        )));
    }
    Ok(())
}

/// Une partie, racontée entrée par entrée.
fn one<M: Machine>(seed: u64) -> Result<(), Failure> {
    let progress = Progress::default();
    let mut timing = Timing::default();
    let played = guarded(&progress, || {
        play::<M>(
            seed,
            LONG,
            Resume::Yes,
            Talk::Everything,
            &progress,
            &mut timing,
        )
    })?;
    for line in played.lines {
        println!("{line}");
    }
    Ok(())
}

/// Le journal d'une partie de soirée, rejoué.
fn replay<M: Machine>(file: &str) -> Result<(), Failure> {
    let text =
        std::fs::read_to_string(file).map_err(|error| Failure(format!("{file} : {error}")))?;
    let lines = journal::read(&text).map_err(|why| Failure(format!("{file} : {why}")))?;
    match journal::replay::<M>(&lines) {
        Ok((_, done)) => {
            println!(
                "{} pas, {} entrées, {} repères retrouvés sur {}. Empreinte finale : {:016x}.",
                done.ticks, done.inputs, done.marks, done.marks, done.fingerprint
            );
            if done.notes > 0 {
                println!(
                    "Ce journal porte {} note(s) `oups!` : cherche les lignes `note` du fichier.",
                    done.notes
                );
            }
            Ok(())
        }
        Err(Strayed::NoStart(why)) => Err(Failure(format!("{file} : {why}."))),
        Err(Strayed::Unreadable { tick, why }) => Err(Failure(format!(
            "Au pas {tick}, ce journal porte une commande que le jeu ne sait plus lire ({why}).\n\
             Un journal ne se rejoue qu'avec la version du jeu qui l'a écrit."
        ))),
        Err(Strayed::Differs {
            tick,
            expected,
            found,
        }) => Err(Failure(format!(
            "Au pas {tick}, la partie rejouée n'est plus celle du journal.\n  \
             dans le journal : {expected}\n  \
             rejouée         : {found}\n\
             Si le jeu a changé depuis, c'est attendu : un journal ne se rejoue qu'avec la \
             version qui l'a écrit. Sinon, la machine ne joue pas deux fois la même partie."
        ))),
    }
}

/// La commande qui rejoue une partie du banc.
fn again(seed: u64) -> String {
    let package = env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "<la machine>".to_string());
    format!("cargo run --release -p {package} --example banc -- --graine {seed}")
}

/// Des microsecondes, en millisecondes lisibles.
fn millis(micros: u64) -> String {
    format!("{},{:03} ms", micros / 1000, micros % 1000)
}

/// Joue une partie en rattrapant un arrêt de la machine : on dit où elle en
/// était, et comment la revoir.
#[cfg(not(target_arch = "wasm32"))]
fn guarded(
    progress: &Progress,
    work: impl FnOnce() -> Result<Outcome, Failure>,
) -> Result<Outcome, Failure> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).unwrap_or_else(|_| {
        let seed = progress.seed.load(Ordering::Relaxed);
        Err(Failure(format!(
            "La machine s'est arrêtée pendant la partie {seed}, à l'entrée {} (après {} pas). \
             La raison est écrite juste au-dessus.\n\
             À la soirée, cet arrêt aurait arrêté le jeu pour tout le monde.\n\
             Pour revoir la partie, entrée par entrée : {}",
            progress.input.load(Ordering::Relaxed),
            progress.tick.load(Ordering::Relaxed),
            again(seed)
        )))
    })
}

/// En WebAssembly, un arrêt arrête tout : le natif, lancé d'abord, l'a déjà dit.
#[cfg(target_arch = "wasm32")]
fn guarded(
    _: &Progress,
    work: impl FnOnce() -> Result<Outcome, Failure>,
) -> Result<Outcome, Failure> {
    work()
}

/// Joue une partie de bout en bout.
fn play<M: Machine>(
    seed: u64,
    plan: Plan,
    resume: Resume,
    talk: Talk,
    progress: &Progress,
    timing: &mut Timing,
) -> Result<Outcome, Failure> {
    progress.seed.store(seed, Ordering::Relaxed);
    progress.input.store(0, Ordering::Relaxed);
    progress.tick.store(0, Ordering::Relaxed);
    let mut party = Party {
        seed,
        machine: M::new(seed),
        // Le hasard du banc n'est pas celui du jeu : une autre graine.
        rng: Rng::new(seed ^ 0x5eed_ba5e_ba11_d0c5),
        plan,
        resume,
        talk,
        present: Vec::new(),
        known: Vec::new(),
        inputs: 0,
        ticks: 0,
        suggested: 0,
        shown: Print::new(),
        lines: Vec::new(),
        progress,
        timing,
    };
    party.mark("neuve");
    // Le plus souvent, l'écran est là avant tout le monde.
    if party.rng.chance(4, 5) {
        let screen = Page::screen(party.rng.next_u64());
        party.arrive(screen)?;
        party.after("l'écran arrive")?;
    }
    while party.inputs < plan.inputs {
        party.step()?;
    }
    party.mark("finie");
    Ok(Outcome {
        lines: party.lines,
        inputs: party.inputs,
        ticks: party.ticks,
        suggested: party.suggested,
    })
}

impl<M: Machine> Party<'_, M> {
    /// Une chose de plus arrive à la partie.
    fn step(&mut self) -> Result<(), Failure> {
        match self.rng.below(100) {
            0..=29 => self.pass(),
            30..=51 => self.suggested_command(1),
            52..=63 => self.attend(),
            64..=68 => self.suggested_command(2),
            69..=80 => self.random_command(),
            81..=84 => self.stranger(),
            85..=91 => self.join(),
            92..=96 => self.leave(),
            _ => self.reload(),
        }
    }

    /// Le temps passe : le plus souvent un instant, parfois de longues minutes.
    fn pass(&mut self) -> Result<(), Failure> {
        let rate = u64::from(M::TICKS_PER_SECOND);
        let left = self
            .plan
            .seconds
            .saturating_mul(rate)
            .saturating_sub(self.ticks);
        if left == 0 {
            // Un jeu sans rythme, ou une partie qui a épuisé son temps.
            return self.suggested_command(1);
        }
        let most = match self.rng.below(100) {
            0..=54 => rate / 4,
            55..=84 => rate.saturating_mul(3),
            85..=96 => rate.saturating_mul(20),
            _ => rate.saturating_mul(120),
        };
        let length = self.rng.below(most).saturating_add(1).min(left);
        for elapsed in 0..length {
            self.tick()?;
            // Chaque pas d'abord, puis une fois par seconde : écrire toutes
            // les vues à chaque pas d'une longue attente ne montrerait rien de plus.
            if elapsed < rate / 4 || elapsed.is_multiple_of(rate) {
                self.look()?;
            }
        }
        self.look()?;
        if self.talk == Talk::Everything {
            let what = format!("{length} pas de temps");
            self.mark(&what);
        }
        Ok(())
    }

    /// Les joueurs sont attentifs : le temps passe pas à pas, et chaque page
    /// regarde souvent ce que la machine lui suggère. Une suggestion qui ne
    /// vaut qu'un instant (frapper quand la balle est là) est saisie.
    fn attend(&mut self) -> Result<(), Failure> {
        let rate = u64::from(M::TICKS_PER_SECOND);
        let left = self
            .plan
            .seconds
            .saturating_mul(rate)
            .saturating_sub(self.ticks);
        if left == 0 || self.present.is_empty() {
            return self.suggested_command(1);
        }
        let most = rate.saturating_mul(ATTENTION_SECONDS);
        let length = self.rng.below(most).saturating_add(1).min(left);
        let mut sent = 0_u8;
        for elapsed in 0..length {
            self.tick()?;
            if elapsed < rate / 4 || elapsed.is_multiple_of(rate) {
                self.look()?;
            }
            for page in self.present.clone() {
                if sent >= ATTENTION_COMMANDS || !self.rng.chance(1, 8) {
                    continue;
                }
                let suggestions = self.machine.suggest(page);
                let rank = self.rng.index(suggestions.len());
                let Some(command) = suggestions.into_iter().nth(rank) else {
                    continue;
                };
                self.suggested = self.suggested.saturating_add(1);
                self.apply(Input::Message(page, command))?;
                sent = sent.saturating_add(1);
                self.after(&format!("{} joue au bon moment", name(page)))?;
            }
            if sent >= ATTENTION_COMMANDS {
                break;
            }
        }
        if sent == 0 {
            // Personne n'avait rien à faire : le temps a passé, c'est tout.
            self.look()?;
            if self.talk == Talk::Everything {
                self.mark("un moment d'attention, sans rien à faire");
            }
        }
        Ok(())
    }

    fn tick(&mut self) -> Result<(), Failure> {
        self.ticks = self.ticks.saturating_add(1);
        self.progress.tick.store(self.ticks, Ordering::Relaxed);
        self.apply(Input::Tick)
    }

    /// Une page présente envoie ce que la machine suggère, une fois ou deux
    /// de suite (un double appui).
    fn suggested_command(&mut self, times: u8) -> Result<(), Failure> {
        let Some(page) = self.rng.pick(&self.present).copied() else {
            return self.join();
        };
        let rank = self.rng.next_u64();
        let mut sent = 0_u8;
        for _ in 0..times {
            let suggestions = self.machine.suggest(page);
            if suggestions.is_empty() {
                break;
            }
            let length = u64::try_from(suggestions.len()).unwrap_or(1);
            let rank = usize::try_from(rank.checked_rem(length).unwrap_or(0)).unwrap_or(0);
            let Some(command) = suggestions.into_iter().nth(rank) else {
                break;
            };
            self.suggested = self.suggested.saturating_add(1);
            self.apply(Input::Message(page, command))?;
            sent = sent.saturating_add(1);
        }
        if sent == 0 {
            return self.random_command();
        }
        self.after(&format!(
            "{} envoie une commande suggérée ({sent} fois)",
            name(page)
        ))
    }

    /// Une page présente envoie n'importe quoi.
    fn random_command(&mut self) -> Result<(), Failure> {
        let Some(page) = self.rng.pick(&self.present).copied() else {
            return self.join();
        };
        self.send_random(page)
    }

    /// Une page que la machine n'a jamais vue arriver envoie n'importe quoi.
    fn stranger(&mut self) -> Result<(), Failure> {
        let page = self.newcomer(1, 2);
        self.send_random(page)
    }

    /// Une page neuve : une télécommande `times` fois sur `out_of`, sinon un
    /// écran. Toujours un écran pour un jeu sans télécommande : la console
    /// n'en laisse entrer aucune.
    fn newcomer(&mut self, times: u64, out_of: u64) -> Page {
        if self.rng.chance(times, out_of) && M::REMOTES {
            Page::remote(self.rng.next_u64())
        } else {
            Page::screen(self.rng.next_u64())
        }
    }

    fn send_random(&mut self, page: Page) -> Result<(), Failure> {
        match draw::draw::<M::Command>(&mut self.rng) {
            Ok(command) => {
                self.apply(Input::Message(page, command))?;
                self.after(&format!("{} envoie une commande au hasard", name(page)))
            }
            Err(refusal) if refusal.cause == Cause::HiddenShape => Err(Failure(format!(
                "Le banc ne sait pas tirer une commande de ce jeu : {}",
                refusal.why
            ))),
            // Le type a refusé la valeur : la ligne le dit, et un écart entre
            // l'ordinateur et WebAssembly montre la valeur en cause (un
            // `usize`, qui n'a pas la même taille des deux côtés).
            Err(refusal) => {
                self.lines.push(format!(
                    "partie {}, entrée {} : tirage refusé ({})",
                    self.seed, self.inputs, refusal.why
                ));
                self.after("une commande au hasard, refusée par son type")
            }
        }
    }

    /// Une page arrive : une nouvelle, ou une qui revient.
    fn join(&mut self) -> Result<(), Failure> {
        let away: Vec<Page> = self
            .known
            .iter()
            .copied()
            .filter(|page| !self.present.contains(page))
            .collect();
        let back = if self.rng.chance(1, 2) {
            self.rng.pick(&away).copied()
        } else {
            None
        };
        let page = if let Some(page) = back {
            page
        } else {
            let screens = self.count(Role::Screen);
            let remotes = self.count(Role::Remote);
            let screens_max = if M::REMOTES {
                SCREENS_MAX
            } else {
                SCREENS_ALONE_MAX
            };
            if M::REMOTES
                && remotes < REMOTES_MAX
                && (screens >= screens_max || self.rng.chance(5, 6))
            {
                Page::remote(self.rng.next_u64())
            } else if screens < screens_max {
                Page::screen(self.rng.next_u64())
            } else {
                // Tout le monde est là : quelqu'un s'en va à la place.
                return self.leave();
            }
        };
        self.arrive(page)?;
        self.after(&format!("{} arrive", name(page)))
    }

    fn count(&self, role: Role) -> usize {
        self.present.iter().filter(|page| page.role == role).count()
    }

    fn arrive(&mut self, page: Page) -> Result<(), Failure> {
        if !self.known.contains(&page) {
            self.known.push(page);
        }
        if !self.present.contains(&page) {
            self.present.push(page);
        }
        self.apply(Input::Joined(page))
    }

    /// Une page s'en va.
    fn leave(&mut self) -> Result<(), Failure> {
        let Some(page) = self.rng.pick(&self.present).copied() else {
            return self.join();
        };
        self.present.retain(|one| *one != page);
        self.apply(Input::Left(page))?;
        self.after(&format!("{} part", name(page)))
    }

    /// Une page présente se recharge : elle arrive sans être partie.
    fn reload(&mut self) -> Result<(), Failure> {
        let Some(page) = self.rng.pick(&self.present).copied() else {
            return self.join();
        };
        self.apply(Input::Joined(page))?;
        self.after(&format!("{} se recharge", name(page)))
    }

    /// Fait entrer quelque chose dans la machine, et écoute ses effets.
    fn apply(&mut self, input: Input<M::Command>) -> Result<(), Failure> {
        let budget = Timing::budget::<M>();
        let machine = &mut self.machine;
        let effects = timed(self.timing, budget, || machine.apply(input));
        for (to, effect) in &effects {
            let to = json(to, "le destinataire d'un effet")?;
            let effect = json(effect, "un effet")?;
            self.shown.feed(to.as_bytes());
            self.shown.feed(effect.as_bytes());
        }
        Ok(())
    }

    /// Calcule et écrit la vue de chaque page présente, comme la console.
    fn look(&mut self) -> Result<(), Failure> {
        let budget = Timing::budget::<M>();
        let (machine, present) = (&self.machine, &self.present);
        let views = timed(self.timing, budget, || {
            present
                .iter()
                .filter_map(|page| machine.view(*page))
                .map(|view| json(&view, "une vue"))
                .collect::<Result<Vec<String>, Failure>>()
        })?;
        for view in views {
            self.shown.feed(view.as_bytes());
        }
        Ok(())
    }

    /// Après chaque entrée d'une page : les vues, et parfois un repère.
    fn after(&mut self, what: &str) -> Result<(), Failure> {
        self.inputs = self.inputs.saturating_add(1);
        self.progress.input.store(self.inputs, Ordering::Relaxed);
        self.look()?;
        if self.inputs.is_multiple_of(STRIDE) {
            self.checkpoint()?;
            self.mark(what);
        } else if self.talk == Talk::Everything {
            self.mark(what);
        }
        Ok(())
    }

    /// Un repère : la photo, la reprise, et ce qu'une page inconnue obtient.
    fn checkpoint(&mut self) -> Result<(), Failure> {
        // Une page que la machine n'a jamais vue ne doit rien casser.
        let id = self.rng.next_u64();
        let nobody = if M::REMOTES {
            Page::remote(id)
        } else {
            Page::screen(id)
        };
        if let Some(view) = self.machine.view(nobody) {
            let view = json(&view, "une vue")?;
            self.shown.feed(view.as_bytes());
        }
        let _: Vec<M::Command> = self.machine.suggest(nobody);
        if self.resume == Resume::No {
            return Ok(());
        }
        let photo = self.machine.save();
        let Some(resumed) = M::load(&photo) else {
            return Err(Failure(format!(
                "La photo de la partie {} ne se relit pas (entrée {}).\n\
                 `load(save(état))` doit rendre l'état. Avec `serde` : un type qui ne se relit \
                 pas comme il s'écrit (un `Deserialize` écrit à la main, un `#[serde(skip)]` sans \
                 valeur par défaut, un format qui demande à deviner le type).\n\
                 Pour revoir la partie : {}",
                self.seed,
                self.inputs,
                again(self.seed)
            )));
        };
        if resumed.save() != photo {
            return Err(Failure(format!(
                "Relue, la photo de la partie {} ne redonne pas les mêmes octets (entrée {}).\n\
                 Deux états égaux doivent s'écrire pareil : cherche un ordre qui change (une \
                 table de hachage), ou une valeur recalculée à la relecture.\n\
                 Pour revoir la partie : {}",
                self.seed,
                self.inputs,
                again(self.seed)
            )));
        }
        self.machine = resumed;
        Ok(())
    }

    /// Une ligne d'empreinte : l'état de la machine, et ce que les pages ont reçu.
    fn mark(&mut self, what: &str) {
        let line = format!(
            "partie {}, entrée {}, pas {} : {:016x}, vues {:016x}",
            self.seed,
            self.inputs,
            self.ticks,
            fingerprint(&self.machine),
            self.shown.0
        );
        self.lines.push(match self.talk {
            Talk::Marks => line,
            Talk::Everything => format!("{line}   ← {what}"),
        });
    }
}

/// Le nom d'une page dans le récit d'une partie.
fn name(page: Page) -> String {
    let role = match page.role {
        Role::Screen => "l'écran",
        Role::Remote => "le téléphone",
    };
    format!("{role} {:04x}", page.id & 0xffff)
}

/// Écrit une valeur en JSON, comme la console l'enverra à une page.
fn json<T: Serialize>(value: &T, what: &str) -> Result<String, Failure> {
    serde_json::to_string(value).map_err(|error| {
        Failure(format!(
            "La console n'arrive pas à écrire {what} en JSON : {error}.\n\
             Une page ne la recevrait jamais. Le plus souvent : une table dont la clé n'est ni \
             un texte ni un nombre (un tuple, une structure). Change la clé, ou envoie une liste."
        ))
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::vec;

    use serde::Deserialize;

    use crate::{NoEffect, To};

    use super::*;

    /// Un petit jeu : on s'assoit, on lance, on tape pendant dix secondes.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Taps {
        seats: BTreeMap<u64, u32>,
        /// Les pas qui restent à la manche ; 0 au salon.
        left: u32,
        rounds: u32,
        #[serde(skip)]
        forgotten: u32,
    }

    #[derive(Debug, Serialize, Deserialize)]
    enum Command {
        Start,
        Tap,
        Name(String),
    }

    #[derive(Debug, Serialize, Deserialize)]
    enum Ding {
        RoundOver,
    }

    impl Machine for Taps {
        type Command = Command;
        type View = (u32, u32);
        type Effect = Ding;
        const TICKS_PER_SECOND: u32 = 20;

        fn new(_: u64) -> Self {
            Self {
                seats: BTreeMap::new(),
                left: 0,
                rounds: 0,
                forgotten: 0,
            }
        }

        fn apply(&mut self, input: Input<Command>) -> Vec<(To, Ding)> {
            match input {
                Input::Joined(page) if page.role == Role::Remote => {
                    let _: &mut u32 = self.seats.entry(page.id).or_insert(0);
                }
                Input::Joined(_) | Input::Left(_) | Input::Message(_, Command::Name(_)) => {}
                Input::Message(page, Command::Start) => {
                    if page.role == Role::Screen && self.left == 0 {
                        self.left = 200;
                    }
                }
                Input::Message(page, Command::Tap) => {
                    if self.left > 0
                        && let Some(taps) = self.seats.get_mut(&page.id)
                    {
                        *taps = taps.saturating_add(1);
                    }
                }
                Input::Tick => {
                    if self.left == 1 {
                        self.rounds = self.rounds.saturating_add(1);
                        self.left = 0;
                        return vec![(To::Everyone, Ding::RoundOver)];
                    }
                    self.left = self.left.saturating_sub(1);
                }
            }
            Vec::new()
        }

        fn view(&self, page: Page) -> Option<(u32, u32)> {
            match page.role {
                Role::Screen => Some((self.left, self.rounds)),
                Role::Remote => self.seats.get(&page.id).map(|taps| (*taps, self.rounds)),
            }
        }

        fn suggest(&self, page: Page) -> Vec<Command> {
            match page.role {
                Role::Screen if self.left == 0 => vec![Command::Start],
                Role::Screen => Vec::new(),
                Role::Remote => vec![Command::Tap],
            }
        }
    }

    fn lines<M: Machine>(seed: u64, resume: Resume) -> Outcome {
        let progress = Progress::default();
        let mut timing = Timing::default();
        play::<M>(seed, SHORT, resume, Talk::Marks, &progress, &mut timing)
            .unwrap_or_else(|Failure(why)| panic!("{why}"))
    }

    #[test]
    fn the_same_seed_plays_the_same_party() {
        let first = lines::<Taps>(3, Resume::Yes);
        let second = lines::<Taps>(3, Resume::Yes);
        assert_eq!(first.lines, second.lines);
        assert_ne!(first.lines, lines::<Taps>(4, Resume::Yes).lines);
    }

    #[test]
    fn suggestions_make_the_party_go_somewhere() {
        // Avec les suggestions, l'écran lance des manches et elles se finissent.
        let played = lines::<Taps>(1, Resume::Yes);
        assert!(played.suggested > 50);
        assert_eq!(played.inputs, SHORT.inputs);
        assert!(played.ticks > 200);
    }

    #[test]
    fn a_whole_state_resumes_as_if_nothing_happened() {
        for seed in 0..4 {
            assert_eq!(
                lines::<Taps>(seed, Resume::Yes).lines,
                lines::<Taps>(seed, Resume::No).lines
            );
        }
    }

    /// Le même jeu, mais qui compte ses manches dans un champ que la photo oublie.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Forgetful(Taps);

    impl Machine for Forgetful {
        type Command = Command;
        type View = (u32, u32);
        type Effect = Ding;
        const TICKS_PER_SECOND: u32 = 20;

        fn new(seed: u64) -> Self {
            Self(Taps::new(seed))
        }

        fn apply(&mut self, input: Input<Command>) -> Vec<(To, Ding)> {
            let effects = self.0.apply(input);
            if !effects.is_empty() {
                self.0.forgotten = self.0.forgotten.saturating_add(1);
            }
            effects
        }

        fn view(&self, page: Page) -> Option<(u32, u32)> {
            self.0
                .view(page)
                .map(|(first, _)| (first, self.0.forgotten))
        }

        fn suggest(&self, page: Page) -> Vec<Command> {
            self.0.suggest(page)
        }
    }

    #[test]
    fn a_state_the_photo_forgets_shows_when_the_party_resumes() {
        let differs = (0..6).any(|seed| {
            lines::<Forgetful>(seed, Resume::Yes).lines
                != lines::<Forgetful>(seed, Resume::No).lines
        });
        assert!(differs);
    }

    #[test]
    fn a_machine_that_stops_is_caught_with_the_way_to_see_it_again() {
        #[derive(Serialize, Deserialize)]
        struct Fragile;

        impl Machine for Fragile {
            type Command = Command;
            type View = ();
            type Effect = NoEffect;
            const TICKS_PER_SECOND: u32 = 0;

            fn new(_: u64) -> Self {
                Self
            }

            fn apply(&mut self, input: Input<Command>) -> Vec<(To, NoEffect)> {
                assert!(
                    !matches!(input, Input::Message(_, Command::Name(_))),
                    "un prénom casse tout"
                );
                Vec::new()
            }

            fn view(&self, _: Page) -> Option<()> {
                None
            }

            fn suggest(&self, _: Page) -> Vec<Command> {
                vec![Command::Tap]
            }
        }

        let progress = Progress::default();
        let mut timing = Timing::default();
        let caught = guarded(&progress, || {
            play::<Fragile>(0, SHORT, Resume::Yes, Talk::Marks, &progress, &mut timing)
        });
        let Err(Failure(why)) = caught else {
            panic!("la machine aurait dû s'arrêter");
        };
        assert!(why.contains("--graine 0"));
    }

    #[test]
    fn a_game_without_rhythm_never_gets_a_tick() {
        #[derive(Serialize, Deserialize)]
        struct Still(u64);

        impl Machine for Still {
            type Command = Command;
            type View = u64;
            type Effect = NoEffect;
            const TICKS_PER_SECOND: u32 = 0;

            fn new(_: u64) -> Self {
                Self(0)
            }

            fn apply(&mut self, input: Input<Command>) -> Vec<(To, NoEffect)> {
                assert!(
                    !matches!(input, Input::Tick),
                    "un pas dans un jeu sans rythme"
                );
                self.0 = self.0.saturating_add(1);
                Vec::new()
            }

            fn view(&self, _: Page) -> Option<u64> {
                Some(self.0)
            }

            fn suggest(&self, _: Page) -> Vec<Command> {
                vec![Command::Tap]
            }
        }

        assert_eq!(lines::<Still>(2, Resume::Yes).ticks, 0);
    }

    #[test]
    fn a_game_without_remotes_never_sees_one() {
        #[derive(Serialize, Deserialize)]
        struct Board(u64);

        fn only_screens(page: Page) {
            assert_eq!(page.role, Role::Screen, "une télécommande est entrée");
        }

        impl Machine for Board {
            type Command = Command;
            type View = u64;
            type Effect = NoEffect;
            const TICKS_PER_SECOND: u32 = 0;
            const REMOTES: bool = false;

            fn new(_: u64) -> Self {
                Self(0)
            }

            fn apply(&mut self, input: Input<Command>) -> Vec<(To, NoEffect)> {
                match input {
                    Input::Joined(page) | Input::Left(page) | Input::Message(page, _) => {
                        only_screens(page);
                    }
                    Input::Tick => {}
                }
                self.0 = self.0.saturating_add(1);
                Vec::new()
            }

            fn view(&self, page: Page) -> Option<u64> {
                only_screens(page);
                Some(self.0)
            }

            fn suggest(&self, page: Page) -> Vec<Command> {
                only_screens(page);
                vec![Command::Tap]
            }
        }

        for seed in 0..6 {
            assert!(lines::<Board>(seed, Resume::Yes).inputs > 0);
        }
    }

    #[test]
    fn microseconds_read_as_milliseconds() {
        assert_eq!(millis(4_166), "4,166 ms");
        assert_eq!(millis(50), "0,050 ms");
    }
}
