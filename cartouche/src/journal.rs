//! Le journal d'une partie : la graine et chaque entrée, avec le numéro de son
//! pas. Il suffit à refaire la partie à l'identique.
//!
//! Ce que le monde extérieur a répondu à la machine y est aussi : rejouée, la
//! partie n'interroge ni internet ni le disque. Seuls les octets d'un contenu
//! lourd n'y sont pas : le journal porte son ticket, et les octets se relisent
//! dans le dossier des contenus.
//!
//! Une ligne de JSON par événement, dans l'ordre où la machine les a reçus. La
//! console l'écrit pendant que le jeu tourne ; [`replay`] le rejoue.
//!
//! La graine et les empreintes sont écrites comme des textes : ce sont des
//! entiers de 64 bits, qu'un outil qui lit le JSON en flottants abîmerait.

use std::format;
use std::string::{String, ToString};
use std::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::ask::Got;
use crate::{Answer, Ask, Content, Date, Input, Machine, Page, Want, fingerprint};

/// Une ligne du journal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Line {
    /// Le début : toujours la première ligne.
    Start {
        /// Le nom du jeu, pour s'y retrouver.
        game: String,
        /// La graine de la partie, en chiffres.
        seed: String,
        ticks_per_second: u32,
        /// L'état de départ, en hexadécimal, quand la partie reprend un état
        /// gardé au lieu de naître de la graine.
        state: Option<String>,
        /// Le numéro du premier pas : les pas que cet état avait déjà reçus.
        #[serde(default)]
        tick: u64,
    },
    /// Une page est arrivée, après `tick` pas.
    Joined { tick: u64, page: Page },
    /// Une page est partie.
    Left { tick: u64, page: Page },
    /// Une page a envoyé une commande, telle qu'elle l'a écrite.
    Message {
        tick: u64,
        page: Page,
        command: serde_json::Value,
    },
    /// La console a répondu à la demande numéro `id` de la machine.
    Answer { tick: u64, id: u32, answer: Told },
    /// L'empreinte de l'état à cet endroit du journal, en hexadécimal.
    Mark { tick: u64, fingerprint: String },
    /// Un cas qui n'aurait pas dû arriver (`oups!`).
    Note {
        tick: u64,
        what: String,
        file: String,
        line: u32,
        count: u32,
    },
}

impl Line {
    /// La première ligne d'un journal.
    ///
    /// `kept` est l'état repris et le nombre de pas qu'il avait déjà reçus,
    /// quand la partie ne naît pas de la graine.
    pub fn start(game: &str, seed: u64, ticks_per_second: u32, kept: Option<(&[u8], u64)>) -> Self {
        Line::Start {
            game: game.to_string(),
            seed: seed.to_string(),
            ticks_per_second,
            state: kept.map(|(state, _)| hex(state)),
            tick: kept.map_or(0, |(_, tick)| tick),
        }
    }

    /// La réponse de la console à une demande, après `tick` pas.
    pub fn answer(tick: u64, ask: &Ask, answer: &Answer) -> Self {
        Line::Answer {
            tick,
            id: ask.id,
            answer: Told::of(ask, answer),
        }
    }

    /// Un repère : l'empreinte de l'état après `tick` pas.
    pub fn mark(tick: u64, print: u64) -> Self {
        Line::Mark {
            tick,
            fingerprint: format!("{print:016x}"),
        }
    }

    /// La ligne, écrite : du JSON sans retour à la ligne.
    pub fn write(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Une réponse de la console, telle que le journal l'écrit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Told {
    /// Pas de réponse.
    Nothing,
    /// C'est fait.
    Done,
    Date(Date),
    /// Ce qu'internet a répondu.
    Web {
        status: u16,
        body: Bytes,
    },
    /// Les octets d'une sauvegarde.
    Bytes(Bytes),
    /// Les noms des sauvegardes.
    Names(Vec<String>),
    /// Les octets de ce contenu : ils sont dans le dossier des contenus, sous
    /// le nom de ce ticket, pas dans le journal.
    Content(Content),
}

impl Told {
    fn of(ask: &Ask, answer: &Answer) -> Self {
        match (&answer.0, &ask.want) {
            (Got::Nothing, _) => Told::Nothing,
            (Got::Done, _) => Told::Done,
            (Got::Date(date), _) => Told::Date(*date),
            (Got::Web { status, body }, _) => Told::Web {
                status: *status,
                body: Bytes::of(body),
            },
            (Got::Bytes(_), Want::Content(content)) => Told::Content(*content),
            (
                Got::Bytes(bytes),
                Want::Date
                | Want::Web(_)
                | Want::Save { .. }
                | Want::Load(_)
                | Want::Erase(_)
                | Want::Saves,
            ) => Told::Bytes(Bytes::of(bytes)),
            (Got::Names(names), _) => Told::Names(names.clone()),
        }
    }

    /// La réponse à remettre à la machine. `contents` rend les octets d'un
    /// contenu ; `Err` nomme le contenu qui n'est plus là.
    fn answer(&self, contents: impl Fn(Content) -> Option<Vec<u8>>) -> Result<Answer, String> {
        Ok(match self {
            Told::Nothing => Answer::nothing(),
            Told::Done => Answer::done(),
            Told::Date(date) => Answer::date(*date),
            Told::Web { status, body } => Answer::web(*status, body.read()?),
            Told::Bytes(bytes) => Answer::bytes(bytes.read()?),
            Told::Names(names) => Answer::names(names.clone()),
            Told::Content(content) => {
                Answer::bytes(contents(*content).ok_or_else(|| content.name())?)
            }
        })
    }
}

/// Des octets dans le journal : tels quels quand ce sont du texte (la réponse
/// d'internet se lit dans le fichier), en hexadécimal sinon.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Bytes {
    Text(String),
    Hex { hex: String },
}

impl Bytes {
    fn of(bytes: &[u8]) -> Self {
        // Un texte qui se lit : sans caractère invisible, sauf la fin de ligne.
        let reads = |text: &str| {
            text.chars()
                .all(|one| !one.is_control() || "\n\r\t".contains(one))
        };
        match core::str::from_utf8(bytes) {
            Ok(text) if reads(text) => Bytes::Text(text.to_string()),
            Ok(_) | Err(_) => Bytes::Hex { hex: hex(bytes) },
        }
    }

    fn read(&self) -> Result<Vec<u8>, String> {
        match self {
            Bytes::Text(text) => Ok(text.as_bytes().to_vec()),
            Bytes::Hex { hex } => {
                unhex(hex).ok_or_else(|| "des octets mal écrits en hexadécimal".to_string())
            }
        }
    }
}

/// Lit un journal. Les lignes vides sont ignorées ; une ligne coupée à la fin
/// (le jeu s'est arrêté en l'écrivant) aussi.
pub fn read(text: &str) -> Result<Vec<Line>, String> {
    let rows: Vec<(usize, &str)> = text
        .lines()
        .enumerate()
        .filter(|(_, row)| !row.trim().is_empty())
        .collect();
    let last = rows.len().saturating_sub(1);
    let mut lines = Vec::new();
    for (rank, (number, row)) in rows.into_iter().enumerate() {
        match serde_json::from_str(row) {
            Ok(line) => lines.push(line),
            Err(_) if rank == last && rank > 0 => {}
            Err(error) => {
                return Err(format!(
                    "ligne {} illisible : {error}",
                    number.saturating_add(1)
                ));
            }
        }
    }
    Ok(lines)
}

/// Ce qu'un journal rejoué a donné.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replayed {
    /// Les pas joués.
    pub ticks: u64,
    /// Les entrées des pages rejouées.
    pub inputs: u64,
    /// Les repères retrouvés.
    pub marks: u64,
    /// Les notes (`oups!`) que le journal portait.
    pub notes: u64,
    /// L'empreinte de l'état à la fin.
    pub fingerprint: u64,
}

/// Pourquoi un journal ne se rejoue pas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Strayed {
    /// Le journal ne commence pas par son début, ou son début est abîmé.
    NoStart(String),
    /// Une commande que ce jeu ne sait plus lire : le journal vient d'une
    /// autre version du jeu.
    Unreadable { tick: u64, why: String },
    /// Une réponse du journal ne se relit pas : un contenu que la console ne
    /// garde plus, ou des octets abîmés.
    Gone { tick: u64, what: String },
    /// L'état n'est plus celui du repère : la machine ne joue plus la même
    /// partie.
    Differs {
        tick: u64,
        expected: String,
        found: String,
    },
}

/// Rejoue un journal avec la machine `M`, et vérifie chaque repère. Un
/// journal qui porte le ticket d'un contenu ne se rejoue qu'avec
/// [`replay_with`].
pub fn replay<M: Machine>(lines: &[Line]) -> Result<(M, Replayed), Strayed> {
    replay_with(lines, |_| None)
}

/// Rejoue un journal avec la machine `M`, et vérifie chaque repère.
/// `contents` rend les octets d'un contenu que la console garde.
pub fn replay_with<M: Machine>(
    lines: &[Line],
    contents: impl Fn(Content) -> Option<Vec<u8>>,
) -> Result<(M, Replayed), Strayed> {
    let mut rest = lines.iter();
    let Some(Line::Start {
        seed, state, tick, ..
    }) = rest.next()
    else {
        return Err(Strayed::NoStart(
            "la première ligne n'est pas un début".to_string(),
        ));
    };
    let mut machine = match state {
        Some(state) => unhex(state)
            .and_then(|bytes| M::load(&bytes))
            .ok_or_else(|| Strayed::NoStart("l'état de départ ne se relit pas".to_string()))?,
        None => M::new(
            seed.parse()
                .map_err(|_| Strayed::NoStart(format!("la graine `{seed}` n'est pas un nombre")))?,
        ),
    };
    // Le numéro du pas où la machine en est : un état repris ne repart pas de zéro.
    let mut clock = *tick;
    let mut done = Replayed {
        ticks: 0,
        inputs: 0,
        marks: 0,
        notes: 0,
        fingerprint: 0,
    };
    for line in rest {
        match line {
            // Un deuxième début n'a pas de sens : on s'arrête là.
            Line::Start { .. } => break,
            Line::Joined { tick, page } => {
                advance(&mut machine, &mut done, &mut clock, *tick);
                let _: Vec<_> = machine.apply(Input::Joined(*page));
                done.inputs = done.inputs.saturating_add(1);
            }
            Line::Left { tick, page } => {
                advance(&mut machine, &mut done, &mut clock, *tick);
                let _: Vec<_> = machine.apply(Input::Left(*page));
                done.inputs = done.inputs.saturating_add(1);
            }
            Line::Message {
                tick,
                page,
                command,
            } => {
                advance(&mut machine, &mut done, &mut clock, *tick);
                let command = serde_json::from_value(command.clone()).map_err(|error| {
                    Strayed::Unreadable {
                        tick: *tick,
                        why: error.to_string(),
                    }
                })?;
                let _: Vec<_> = machine.apply(Input::Message(*page, command));
                done.inputs = done.inputs.saturating_add(1);
            }
            Line::Answer { tick, id, answer } => {
                advance(&mut machine, &mut done, &mut clock, *tick);
                let answer = answer
                    .answer(&contents)
                    .map_err(|what| Strayed::Gone { tick: *tick, what })?;
                let _: Vec<_> = machine.answer(*id, answer);
                done.inputs = done.inputs.saturating_add(1);
            }
            Line::Mark { tick, fingerprint } => {
                advance(&mut machine, &mut done, &mut clock, *tick);
                let found = format!("{:016x}", self::fingerprint(&machine));
                if &found != fingerprint {
                    return Err(Strayed::Differs {
                        tick: *tick,
                        expected: fingerprint.clone(),
                        found,
                    });
                }
                done.marks = done.marks.saturating_add(1);
            }
            Line::Note { .. } => done.notes = done.notes.saturating_add(1),
        }
    }
    done.fingerprint = self::fingerprint(&machine);
    Ok((machine, done))
}

/// Joue les pas qui manquent pour arriver au pas numéro `tick`.
fn advance<M: Machine>(machine: &mut M, done: &mut Replayed, clock: &mut u64, tick: u64) {
    while *clock < tick {
        let _: Vec<_> = machine.apply(Input::Tick);
        *clock = clock.saturating_add(1);
        done.ticks = done.ticks.saturating_add(1);
    }
}

/// Des octets, en hexadécimal.
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: [char; 16] = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    ];
    let mut text = String::new();
    for byte in bytes {
        for half in [byte >> 4, byte & 0x0f] {
            text.push(DIGITS.get(usize::from(half)).copied().unwrap_or('0'));
        }
    }
    text
}

/// Des octets écrits en hexadécimal, relus.
pub fn unhex(text: &str) -> Option<Vec<u8>> {
    let digits: Vec<u32> = text
        .chars()
        .map(|digit| digit.to_digit(16))
        .collect::<Option<_>>()?;
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    digits
        .chunks(2)
        .map(|pair| match pair {
            [high, low] => u8::try_from(high.saturating_mul(16).saturating_add(*low)).ok(),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::borrow::ToOwned;

    use crate::{NoEffect, Role, State, To};

    use super::*;

    /// Une machine de rien : elle compte les pas et les tapes.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Counter {
        seed: u64,
        ticks: u64,
        taps: u64,
        here: Vec<Page>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    enum Tap {
        Tap,
    }

    impl Machine for Counter {
        type Command = Tap;
        type View = u64;
        type Effect = NoEffect;
        const TICKS_PER_SECOND: u32 = 10;

        fn new(seed: u64) -> Self {
            Self {
                seed,
                ticks: 0,
                taps: 0,
                here: Vec::new(),
            }
        }

        fn apply(&mut self, input: Input<Tap>) -> Vec<(To, NoEffect)> {
            match input {
                Input::Joined(page) => self.here.push(page),
                Input::Left(page) => self.here.retain(|one| *one != page),
                Input::Message(_, Tap::Tap) => self.taps = self.taps.saturating_add(1),
                Input::Tick => self.ticks = self.ticks.saturating_add(1),
            }
            Vec::new()
        }

        fn view(&self, _: Page) -> Option<u64> {
            Some(self.taps)
        }

        fn suggest(&self, _: Page) -> Vec<Tap> {
            std::vec![Tap::Tap]
        }
    }

    fn played() -> (Counter, Vec<Line>) {
        let ada = Page {
            id: 1,
            role: Role::Remote,
        };
        let mut game = Counter::new(7);
        let mut lines = std::vec![Line::start("compteur", 7, 10, None)];
        let _: Vec<_> = game.apply(Input::Joined(ada));
        lines.push(Line::Joined { tick: 0, page: ada });
        for _ in 0..3 {
            let _: Vec<_> = game.apply(Input::Tick);
        }
        let _: Vec<_> = game.apply(Input::Message(ada, Tap::Tap));
        lines.push(Line::Message {
            tick: 3,
            page: ada,
            command: serde_json::json!("Tap"),
        });
        lines.push(Line::mark(3, fingerprint(&game)));
        for _ in 0..2 {
            let _: Vec<_> = game.apply(Input::Tick);
        }
        let _: Vec<_> = game.apply(Input::Left(ada));
        lines.push(Line::Left { tick: 5, page: ada });
        lines.push(Line::mark(5, fingerprint(&game)));
        (game, lines)
    }

    #[test]
    fn a_journal_written_then_read_replays_the_same_game() {
        let (game, lines) = played();
        let text: String = lines.iter().map(|line| line.write() + "\n").collect();
        let (again, done) = replay::<Counter>(&read(&text).unwrap()).unwrap();
        assert_eq!(again, game);
        assert_eq!((done.ticks, done.inputs, done.marks), (5, 3, 2));
        assert_eq!(done.fingerprint, fingerprint(&game));
    }

    #[test]
    fn a_game_that_no_longer_plays_the_same_strays_at_the_first_mark() {
        let (_, played) = played();
        // Une tape de plus, glissée après l'arrivée : la suite n'est plus la même.
        let mut lines = played[..2].to_vec();
        lines.push(Line::Message {
            tick: 1,
            page: Page {
                id: 9,
                role: Role::Remote,
            },
            command: serde_json::json!("Tap"),
        });
        lines.extend_from_slice(&played[2..]);
        let strayed = replay::<Counter>(&lines).unwrap_err();
        assert!(matches!(strayed, Strayed::Differs { tick: 3, .. }));
    }

    #[test]
    fn a_command_from_another_version_is_named() {
        let (_, mut lines) = played();
        lines.push(Line::Message {
            tick: 6,
            page: Page {
                id: 1,
                role: Role::Remote,
            },
            command: serde_json::json!("Jump"),
        });
        let strayed = replay::<Counter>(&lines).unwrap_err();
        assert!(matches!(strayed, Strayed::Unreadable { tick: 6, .. }));
    }

    #[test]
    fn a_game_that_starts_from_a_kept_state_replays_from_it() {
        let (game, _) = played();
        let lines = [
            Line::start("compteur", 0, 10, Some((&game.save(), 5))),
            Line::mark(5, fingerprint(&game)),
            Line::Joined {
                tick: 7,
                page: Page {
                    id: 2,
                    role: Role::Remote,
                },
            },
        ];
        let (again, done) = replay::<Counter>(&lines).unwrap();
        // L'état repris avait déjà reçu cinq pas : il en reçoit deux de plus.
        assert_eq!((again.ticks, done.ticks), (game.ticks + 2, 2));
    }

    /// Une machine qui garde ce que le monde lui répond.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Listener {
        asked: bool,
        date: Option<Date>,
        heard: Vec<Vec<u8>>,
    }

    impl Machine for Listener {
        type Command = Tap;
        type View = ();
        type Effect = NoEffect;
        const TICKS_PER_SECOND: u32 = 10;

        fn new(_: u64) -> Self {
            Self {
                asked: true,
                date: None,
                heard: Vec::new(),
            }
        }

        fn apply(&mut self, _: Input<Tap>) -> Vec<(To, NoEffect)> {
            Vec::new()
        }

        fn view(&self, _: Page) -> Option<()> {
            None
        }

        fn suggest(&self, _: Page) -> Vec<Tap> {
            Vec::new()
        }

        fn asks(&self) -> Vec<Ask> {
            if self.asked {
                std::vec![Ask::date(1)]
            } else {
                Vec::new()
            }
        }

        fn answer(&mut self, _: u32, answer: Answer) -> Vec<(To, NoEffect)> {
            self.asked = false;
            self.date = self.date.or(answer.as_date());
            if let Some((_, body)) = answer.as_web() {
                self.heard.push(body.to_vec());
            }
            if let Some(bytes) = answer.as_bytes() {
                self.heard.push(bytes.to_vec());
            }
            Vec::new()
        }
    }

    #[test]
    fn what_the_world_answered_replays_from_the_journal() {
        let level = Content::of(b"un niveau", crate::Kind::Other);
        let lines = [
            Line::start("curieux", 1, 10, None),
            Line::answer(2, &Ask::date(1), &Answer::date(Date::at(86_400))),
            Line::answer(
                3,
                &Ask::web(2, crate::Web::get("https://juge.test/")),
                &Answer::web(200, b"{\"ok\":true}".to_vec()),
            ),
            Line::answer(
                3,
                &Ask::load(3, "score"),
                &Answer::bytes(std::vec![0x04, 0x00]),
            ),
            Line::answer(
                4,
                &Ask::content(4, level),
                &Answer::bytes(b"un niveau".to_vec()),
            ),
        ];
        let text: String = lines.iter().map(|line| line.write() + "\n").collect();
        // Un texte se lit dans le journal ; le reste est en hexadécimal ; un
        // contenu n'y laisse que son ticket.
        assert!(text.contains(r#""body":"{\"ok\":true}""#), "{text}");
        assert!(text.contains(r#"{"hex":"0400"}"#), "{text}");
        assert!(!text.contains("un niveau"), "{text}");
        let read = read(&text).unwrap();
        assert_eq!(read, lines);

        let store = |content| (content == level).then(|| b"un niveau".to_vec());
        let (again, done) = replay_with::<Listener>(&read, store).unwrap();
        assert_eq!(again.date, Some(Date::at(86_400)));
        assert_eq!(
            again.heard,
            [
                b"{\"ok\":true}".to_vec(),
                std::vec![0x04, 0x00],
                b"un niveau".to_vec()
            ]
        );
        assert_eq!((done.ticks, done.inputs), (4, 4));

        // Sans le contenu, la partie ne se rejoue pas au-delà.
        let strayed = replay::<Listener>(&read).unwrap_err();
        assert_eq!(
            strayed,
            Strayed::Gone {
                tick: 4,
                what: level.name()
            }
        );
    }

    #[test]
    fn a_line_cut_short_at_the_end_is_ignored() {
        let (_, lines) = played();
        let mut text: String = lines.iter().map(|line| line.write() + "\n").collect();
        text.push_str("{\"mark\":{\"tick\":9,\"finger");
        assert_eq!(read(&text).unwrap().len(), lines.len());
        assert!(read("pas du json\n{}\n").is_err());
    }

    #[test]
    fn bytes_survive_hexadecimal() {
        assert_eq!(hex(&[0, 255, 16]), "00ff10".to_owned());
        assert_eq!(unhex("00ff10"), Some(std::vec![0, 255, 16]));
        assert_eq!(unhex("0"), None);
        assert_eq!(unhex("zz"), None);
    }
}
