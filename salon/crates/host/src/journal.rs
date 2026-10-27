//! Bounded, nonblocking typed replay writer. Serialization and disk writes
//! happen on its own thread. Only journal-owned `.jsonl` files older than
//! thirty days are removed; the simulation never waits for disk.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use crate::{HttpHandler, Response};
use salon_core::{Ms, journal::Line};
use serde::Serialize;
use serde::de::DeserializeOwned;

const RETENTION: Duration = Duration::from_hours(720);
const QUEUE: usize = 1024;
static NEXT: AtomicU64 = AtomicU64::new(1);

pub use salon_core::journal::Stored;

#[derive(Debug)]
pub enum RecordError {
    Full,
    Unavailable,
    Finished,
}

impl std::fmt::Display for RecordError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Full => "journal queue full: recording stopped",
            Self::Unavailable => "journal writer unavailable",
            Self::Finished => "journal already finished",
        })
    }
}
impl std::error::Error for RecordError {}

enum Message<H, E> {
    Line(Line<H, E>),
    Flush(SyncSender<io::Result<()>>),
}

pub struct Recorder<H, E> {
    path: PathBuf,
    sender: Option<SyncSender<Message<H, E>>>,
    failed: Arc<AtomicBool>,
    finished: bool,
}

impl<H: Serialize + Send + 'static, E: Serialize + Send + 'static> Recorder<H, E> {
    /// Creates a unique, never-overwritten session. Existing files in the
    /// journal directory are pruned according to the fixed retention rule.
    pub fn start(directory: impl AsRef<Path>, prefix: &str, header: H) -> io::Result<Self> {
        if prefix.is_empty()
            || !prefix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid journal prefix",
            ));
        }
        let directory = directory.as_ref();
        fs::create_dir_all(directory)?;
        prune(directory, wall_time())?;
        let stamp = wall_time()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let (path, file) = loop {
            let suffix = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = directory.join(format!(
                "{prefix}-{stamp}-{}-{suffix}.jsonl",
                std::process::id()
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => break (path, file),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        };
        Ok(Self::with_file(path, file, header))
    }

    /// A game with an established journal naming contract may choose the
    /// filename explicitly. Existing files are always refused.
    pub fn start_file(path: impl AsRef<Path>, header: H) -> io::Result<Self> {
        let path = path.as_ref();
        if let Some(directory) = path
            .parent()
            .filter(|directory| !directory.as_os_str().is_empty())
        {
            fs::create_dir_all(directory)?;
            prune(directory, wall_time())?;
        }
        let file = OpenOptions::new().write(true).create_new(true).open(path)?;
        Ok(Self::with_file(path.to_owned(), file, header))
    }

    fn with_file(path: PathBuf, file: File, header: H) -> Self {
        let (sender, receiver) = sync_channel(QUEUE);
        let failed = Arc::new(AtomicBool::new(false));
        let worker_failed = Arc::clone(&failed);
        let worker_path = path.clone();
        std::thread::spawn(move || {
            let mut writer = BufWriter::new(file);
            let mut sequence = 0_u64;
            let result = (|| -> io::Result<()> {
                write_line(&mut writer, sequence, Line::<H, E>::Header(header))?;
                for message in receiver {
                    let line = match message {
                        Message::Line(line) => line,
                        Message::Flush(reply) => {
                            let flushed = writer.flush();
                            let failed = flushed.is_err();
                            let _: Result<(), _> = reply.send(flushed);
                            if failed {
                                return Err(io::Error::other("journal flush failed"));
                            }
                            continue;
                        }
                    };
                    sequence = sequence
                        .checked_add(1)
                        .ok_or_else(|| io::Error::other("journal sequence exhausted"))?;
                    write_line(&mut writer, sequence, line)?;
                }
                writer.flush()
            })();
            if let Err(error) = result {
                worker_failed.store(true, Ordering::Release);
                eprintln!("journal: {}: {error}", worker_path.display());
            }
        });
        Self {
            path,
            sender: Some(sender),
            failed,
            finished: false,
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Waits for persistence on this calling thread. Intended for HTTP
    /// handlers, command-line replay tools and tests, never the game loop.
    pub fn flush(&self) -> io::Result<()> {
        let sender = self
            .sender
            .as_ref()
            .ok_or_else(|| io::Error::other("journal closed"))?;
        let (reply, answer) = sync_channel(1);
        sender
            .send(Message::Flush(reply))
            .map_err(|_| io::Error::other("journal writer disconnected"))?;
        answer.recv().map_err(io::Error::other)?
    }
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
    pub fn record(&mut self, entry: E) -> Result<(), RecordError> {
        self.send(Line::Entry(entry))
    }
    pub fn marker(&mut self, at: Ms, note: impl Into<String>) -> Result<(), RecordError> {
        self.send(Line::Marker {
            at,
            note: note.into(),
        })
    }
    pub fn checkpoint(&mut self, fingerprint: impl Into<String>) -> Result<(), RecordError> {
        self.send(Line::Checkpoint {
            fingerprint: fingerprint.into(),
        })
    }
    pub fn finish(&mut self, fingerprint: impl Into<String>) -> Result<(), RecordError> {
        self.send(Line::Final {
            fingerprint: fingerprint.into(),
        })?;
        self.finished = true;
        Ok(())
    }
    fn send(&mut self, line: Line<H, E>) -> Result<(), RecordError> {
        if self.finished {
            return Err(RecordError::Finished);
        }
        if self.failed() {
            return Err(RecordError::Unavailable);
        }
        let Some(sender) = self.sender.as_ref() else {
            return Err(RecordError::Finished);
        };
        match sender.try_send(Message::Line(line)) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.failed.store(true, Ordering::Release);
                self.sender = None;
                match error {
                    TrySendError::Full(_) => Err(RecordError::Full),
                    TrySendError::Disconnected(_) => Err(RecordError::Unavailable),
                }
            }
        }
    }
}

fn write_line<H: Serialize, E: Serialize>(
    writer: &mut BufWriter<File>,
    sequence: u64,
    line: Line<H, E>,
) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, &Stored { sequence, line }).map_err(io::Error::other)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

#[expect(
    clippy::disallowed_methods,
    reason = "retention is a filesystem shell concern, never simulation time"
)]
fn wall_time() -> SystemTime {
    SystemTime::now()
}

/// Prunes only plain `.jsonl` files in the dedicated journal directory.
/// Symlinks and directories are never followed or removed.
pub fn prune(directory: &Path, now: SystemTime) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_file()
            || entry.path().extension().is_none_or(|ext| ext != "jsonl")
        {
            continue;
        }
        if let Ok(age) = entry
            .metadata()?
            .modified()
            .and_then(|at| now.duration_since(at).map_err(io::Error::other))
            && age > RETENTION
            && owns(&entry.path())
        {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

/// The first typed envelope identifies files this writer owns. Never
/// infer ownership from the extension alone: the directory may also hold
/// another tool's JSON lines. Oversized or unreadable headers are preserved.
fn owns(path: &Path) -> bool {
    use std::io::Read as _;
    let mut first = String::new();
    let Ok(file) = File::open(path) else {
        return false;
    };
    let mut reader = BufReader::new(file).take(4 * 1024 * 1024);
    if reader.read_line(&mut first).is_err() || !first.ends_with('\n') {
        return false;
    }
    let Ok(Stored {
        sequence: 0,
        line: Line::Header(_),
    }) = serde_json::from_str::<Stored<serde_json::Value, serde_json::Value>>(&first)
    else {
        return false;
    };
    true
}

/// Parsed journal. `entries` is limited to the last checkpoint: a batch
/// interrupted before its checkpoint cannot claim a verified final state.
pub struct Journal<H, E> {
    pub header: H,
    pub entries: Vec<E>,
    pub markers: Vec<(Ms, String)>,
    pub fingerprint: String,
    pub finished: bool,
    /// The final physical line was cut; only the preceding checkpoint is replayed.
    pub truncated_tail: bool,
}

impl<H: DeserializeOwned, E: DeserializeOwned> Journal<H, E> {
    pub fn read(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::from_reader(BufReader::new(File::open(path)?))
    }
    pub fn from_reader(mut reader: impl BufRead) -> io::Result<Self> {
        let mut text = String::new();
        let mut sequence = 0_u64;
        let mut header = None;
        let mut entries = Vec::new();
        let mut markers = Vec::new();
        let mut checkpoint = None;
        let mut finished = false;
        let mut truncated_tail = false;
        loop {
            text.clear();
            if reader.read_line(&mut text)? == 0 {
                break;
            }
            if !text.ends_with('\n') && checkpoint.is_some() && !finished {
                truncated_tail = true;
                break;
            }
            if !text.ends_with('\n') {
                return Err(invalid(
                    "truncated journal line without a verified checkpoint",
                ));
            }
            if finished {
                return Err(invalid("data after journal final state"));
            }
            let stored: Stored<H, E> =
                serde_json::from_str(&text).map_err(|error| invalid(error.to_string()))?;
            if stored.sequence != sequence {
                return Err(invalid("journal sequence missing or out of order"));
            }
            sequence = sequence
                .checked_add(1)
                .ok_or_else(|| invalid("sequence overflow"))?;
            match stored.line {
                Line::Header(value) if header.is_none() && sequence == 1 => header = Some(value),
                Line::Header(_) => return Err(invalid("duplicate journal header")),
                Line::Entry(value) if header.is_some() => entries.push(value),
                Line::Marker { at, note } if header.is_some() => markers.push((at, note)),
                Line::Checkpoint { fingerprint } if header.is_some() => {
                    checkpoint = Some((entries.len(), fingerprint));
                }
                Line::Final { fingerprint } if header.is_some() => {
                    checkpoint = Some((entries.len(), fingerprint));
                    finished = true;
                }
                Line::Entry(_)
                | Line::Marker { .. }
                | Line::Checkpoint { .. }
                | Line::Final { .. } => return Err(invalid("journal has no header")),
            }
        }
        let header = header.ok_or_else(|| invalid("journal has no header"))?;
        let (count, fingerprint) =
            checkpoint.ok_or_else(|| invalid("journal has no state checkpoint"))?;
        entries.truncate(count);
        Ok(Self {
            header,
            entries,
            markers,
            fingerprint,
            finished,
            truncated_tail,
        })
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// A typed HTTP receiver for a solo game's browser journal. The page keeps
/// its game state; this handler only persists its already ordered inputs.
/// Requests must originate on the host itself. Session filenames are ours.
pub fn browser_sink<H, E>(directory: PathBuf, path: &'static str) -> HttpHandler
where
    H: Serialize + DeserializeOwned + 'static,
    E: Serialize + DeserializeOwned + 'static,
{
    let state = Mutex::new(BrowserState::default());
    Arc::new(move |request| {
        if request.path != path {
            return None;
        }
        if request.method != "POST" || !request.local {
            return Some(browser_response(403, "local browser journal only"));
        }
        let Ok(mut state) = state.lock() else {
            return Some(browser_response(503, "journal unavailable"));
        };
        Some(browser_request::<H, E>(
            &mut state,
            &directory,
            &request.body,
        ))
    })
}

const BROWSER_BYTES: usize = 4 * 1024 * 1024;
#[derive(Default)]
struct BrowserState {
    sessions: BrowserSessions,
    partial: std::collections::BTreeMap<String, Assembly>,
}
struct Assembly {
    sequence: u64,
    total: usize,
    bytes: Vec<u8>,
}

fn browser_request<H: Serialize + DeserializeOwned, E: Serialize + DeserializeOwned>(
    state: &mut BrowserState,
    directory: &Path,
    body: &[u8],
) -> Response {
    if let Ok(chunk) = serde_json::from_slice::<salon_core::journal::Chunk>(body) {
        return receive_chunk::<H, E>(state, directory, &chunk);
    }
    decode_browser::<H, E>(state, directory, body)
}

fn decode_browser<H: Serialize + DeserializeOwned, E: Serialize + DeserializeOwned>(
    state: &mut BrowserState,
    directory: &Path,
    body: &[u8],
) -> Response {
    let Ok(submission) = serde_json::from_slice::<salon_core::journal::Submission<H, E>>(body)
    else {
        return browser_response(400, "invalid journal line");
    };
    if !valid_session(&submission.session) {
        return browser_response(400, "invalid session");
    }
    // Validate the game's exact types before erasing for the worker;
    // browser models may contain Rc and need not implement Send.
    let Ok(value) = serde_json::to_value(submission.line) else {
        return browser_response(400, "journal serialization failed");
    };
    let Ok(line) = serde_json::from_value::<Line<serde_json::Value, serde_json::Value>>(value)
    else {
        return browser_response(400, "invalid journal line");
    };
    let header = matches!(line, Line::Header(_));
    let terminal = matches!(line, Line::Final { .. });
    let session = submission.session;
    let response = receive_browser(
        &mut state.sessions,
        directory,
        session.clone(),
        submission.sequence,
        line,
    );
    if header && response.status == 201 {
        discard_owner_assemblies(state, &session);
    }
    if terminal && response.status == 200 {
        state.partial.remove(&session);
    }
    response
}

fn valid_session(session: &str) -> bool {
    !session.is_empty()
        && session.len() <= 96
        && session
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Assemblies have a hard bound, not an idle deadline: a suspended tab
/// can continue its exact next piece at any later time. A new run of the
/// same tab explicitly replaces the previous run's abandoned transfer.
fn discard_owner_assemblies(state: &mut BrowserState, session: &str) {
    if let Some(new_owner) = owner(session) {
        state
            .partial
            .retain(|previous, _| owner(previous) != Some(new_owner));
    }
}

fn receive_chunk<H: Serialize + DeserializeOwned, E: Serialize + DeserializeOwned>(
    state: &mut BrowserState,
    directory: &Path,
    chunk: &salon_core::journal::Chunk,
) -> Response {
    if !valid_session(&chunk.session)
        || chunk.total > BROWSER_BYTES
        || chunk.bytes.is_empty()
        || chunk.bytes.len() > 8 * 1024
        || chunk.total == 0
    {
        return browser_response(413, "journal chunk exceeds limit");
    }
    if chunk.offset == 0 {
        if state.partial.contains_key(&chunk.session) {
            return browser_response(409, "journal transfer already started");
        }
        discard_owner_assemblies(state, &chunk.session);
        if state.partial.len() >= 8 {
            return browser_response(503, "too many journal transfers");
        }
        state.partial.insert(
            chunk.session.clone(),
            Assembly {
                sequence: chunk.sequence,
                total: chunk.total,
                bytes: Vec::new(),
            },
        );
    }
    let Some(assembly) = state.partial.get_mut(&chunk.session) else {
        return browser_response(409, "journal transfer not started");
    };
    if assembly.sequence != chunk.sequence
        || assembly.total != chunk.total
        || assembly.bytes.len() != chunk.offset
        || chunk.bytes.len() > assembly.total.saturating_sub(assembly.bytes.len())
    {
        state.partial.remove(&chunk.session);
        return browser_response(409, "journal chunk out of order");
    }
    assembly.bytes.extend_from_slice(&chunk.bytes);
    if assembly.bytes.len() < assembly.total {
        return browser_response(202, "piece received");
    }
    let Some(assembly) = state.partial.remove(&chunk.session) else {
        return browser_response(503, "journal transfer lost");
    };
    let Ok(submission) =
        serde_json::from_slice::<salon_core::journal::Submission<H, E>>(&assembly.bytes)
    else {
        return browser_response(400, "invalid assembled journal line");
    };
    if submission.session != chunk.session || submission.sequence != chunk.sequence {
        return browser_response(409, "journal transfer identity mismatch");
    }
    decode_browser::<H, E>(state, directory, &assembly.bytes)
}

type BrowserSessions = std::collections::BTreeMap<String, BrowserSession>;
struct BrowserSession {
    recorder: Recorder<serde_json::Value, serde_json::Value>,
    next_sequence: u64,
}

fn browser_response(status: u16, text: &str) -> Response {
    Response {
        status,
        content_type: "text/plain".to_owned(),
        body: text.as_bytes().to_vec(),
    }
}

fn owner(session: &str) -> Option<&str> {
    let (owner, _) = session.split_once('-')?;
    (owner.len() == 32 && owner.chars().all(|c| c.is_ascii_hexdigit())).then_some(owner)
}

fn receive_browser(
    sessions: &mut BrowserSessions,
    directory: &Path,
    session: String,
    sequence: u64,
    line: Line<serde_json::Value, serde_json::Value>,
) -> Response {
    if let Line::Header(header) = line {
        if sequence != 0 {
            return browser_response(409, "header must start sequence zero");
        }
        if sessions.contains_key(&session) {
            return browser_response(409, "session already started");
        }
        // A reloaded tab gets a new run, but keeps its owner. Closing the
        // old writer leaves its last checkpoint intact and releases memory.
        if let Some(new_owner) = owner(&session) {
            sessions.retain(|previous, _| owner(previous) != Some(new_owner));
        }
        if sessions.len() >= 128 {
            return browser_response(503, "too many active journal sessions");
        }
        return match Recorder::start(directory, "browser", header) {
            Ok(recorder) => {
                sessions.insert(
                    session,
                    BrowserSession {
                        recorder,
                        next_sequence: 1,
                    },
                );
                browser_response(201, "started")
            }
            Err(_) => browser_response(503, "journal unavailable"),
        };
    }
    let Some(state) = sessions.get_mut(&session) else {
        return browser_response(409, "journal not started");
    };
    if sequence != state.next_sequence {
        return browser_response(409, "journal sequence out of order");
    }
    let done = matches!(line, Line::Final { .. });
    let checkpoint = matches!(line, Line::Final { .. } | Line::Checkpoint { .. });
    let result = state
        .recorder
        .send(line)
        .map_err(|error| error.to_string())
        .and_then(|()| {
            if checkpoint {
                state.recorder.flush().map_err(|error| error.to_string())
            } else {
                Ok(())
            }
        });
    state.next_sequence = state.next_sequence.saturating_add(1);
    if done || result.is_err() {
        sessions.remove(&session);
    }
    match result {
        Ok(()) => browser_response(200, "recorded"),
        Err(_) => browser_response(503, "recording stopped"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn journal(lines: &[Line<String, u32>]) -> String {
        lines
            .iter()
            .enumerate()
            .map(|(sequence, line)| {
                serde_json::to_string(&Stored {
                    sequence: u64::try_from(sequence).unwrap(),
                    line: line.clone(),
                })
                .unwrap()
                    + "\n"
            })
            .collect()
    }
    #[test]
    fn complete_and_interrupted_sessions_have_the_last_verified_state() {
        let lines = [
            Line::Header("settings".to_owned()),
            Line::Entry(1),
            Line::Checkpoint {
                fingerprint: "a".into(),
            },
            Line::Entry(2),
        ];
        let parsed = Journal::<String, u32>::from_reader(journal(&lines).as_bytes()).unwrap();
        assert_eq!(parsed.entries, vec![1]);
        assert_eq!(parsed.fingerprint, "a");
        assert!(!parsed.finished);
        let mut lines = lines.to_vec();
        lines.push(Line::Final {
            fingerprint: "b".into(),
        });
        let parsed = Journal::<String, u32>::from_reader(journal(&lines).as_bytes()).unwrap();
        assert_eq!(parsed.entries, vec![1, 2]);
        assert!(parsed.finished);
    }
    #[test]
    fn corruption_missing_lines_and_unfinished_batches_are_not_certified() {
        let header = Line::Header("settings".to_owned());
        assert!(
            Journal::<String, u32>::from_reader(
                journal(&[header.clone(), Line::Entry(1)]).as_bytes()
            )
            .is_err()
        );
        let text = journal(&[
            header,
            Line::Checkpoint {
                fingerprint: "x".into(),
            },
        ]);
        assert!(Journal::<String, u32>::from_reader(text.trim_end().as_bytes()).is_err());
        assert!(
            Journal::<String, u32>::from_reader(
                text.replace("\"sequence\":1", "\"sequence\":2").as_bytes()
            )
            .is_err()
        );
    }
    #[test]
    fn a_cut_physical_tail_recovers_only_the_previous_verified_checkpoint() {
        let mut text = journal(&[
            Line::Header("settings".into()),
            Line::Entry(1),
            Line::Checkpoint {
                fingerprint: "one".into(),
            },
            Line::Entry(2),
        ]);
        text.push_str("{\"sequence\":4,\"line\":{\"Entry\":");
        let recovered = Journal::<String, u32>::from_reader(text.as_bytes()).unwrap();
        assert_eq!(recovered.entries, vec![1]);
        assert!(recovered.truncated_tail);
        assert!(!recovered.finished);
        assert_eq!(recovered.fingerprint, "one");
        let mut complete = journal(&[
            Line::Header("settings".into()),
            Line::Final {
                fingerprint: "empty".into(),
            },
        ]);
        complete.push('{');
        assert!(Journal::<String, u32>::from_reader(complete.as_bytes()).is_err());
    }
    #[test]
    fn large_browser_lines_are_assembled_typed_and_missing_pieces_are_refused() {
        let directory = std::env::temp_dir().join(format!(
            "salon-browser-chunks-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut state = BrowserState::default();
        let header = "image-data-".repeat(20_000);
        let text = serde_json::to_vec(&salon_core::journal::Submission {
            session: "large".to_owned(),
            sequence: 0,
            line: Line::<String, u32>::Header(header.clone()),
        })
        .unwrap();
        assert!(text.len() > 64 * 1024);
        let mut offset = 0_usize;
        for bytes in text.chunks(8 * 1024) {
            let chunk = salon_core::journal::Chunk {
                session: "large".into(),
                sequence: 0,
                offset,
                total: text.len(),
                bytes: bytes.to_vec(),
            };
            let body = serde_json::to_vec(&chunk).unwrap();
            assert!(body.len() < 64 * 1024);
            let response = browser_request::<String, u32>(&mut state, &directory, &body);
            offset = offset.saturating_add(bytes.len());
            assert_eq!(
                response.status,
                if offset == text.len() { 201 } else { 202 }
            );
        }
        assert!(state.partial.is_empty());
        let active = &state.sessions["large"];
        active.recorder.flush().unwrap();
        let file = File::open(active.recorder.path()).unwrap();
        let mut line = String::new();
        BufReader::new(file).read_line(&mut line).unwrap();
        let stored: Stored<String, u32> = serde_json::from_str(&line).unwrap();
        assert_eq!(stored.line, Line::Header(header));
        let bad = salon_core::journal::Chunk {
            session: "broken".into(),
            sequence: 0,
            offset: 0,
            total: 100,
            bytes: vec![0; 10],
        };
        assert_eq!(
            receive_chunk::<String, u32>(&mut state, &directory, &bad).status,
            202
        );
        let bad = salon_core::journal::Chunk { offset: 20, ..bad };
        assert_eq!(
            receive_chunk::<String, u32>(&mut state, &directory, &bad).status,
            409
        );
        assert!(state.partial.is_empty());
        let too_big = salon_core::journal::Chunk {
            total: BROWSER_BYTES.saturating_add(1),
            offset: 0,
            ..bad
        };
        assert_eq!(
            receive_chunk::<String, u32>(&mut state, &directory, &too_big).status,
            413
        );
        drop(state);
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn paused_chunk_transfer_resumes_until_explicit_replacement_or_final() {
        let directory = std::env::temp_dir().join(format!(
            "salon-paused-chunks-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut state = BrowserState::default();
        let owner = "0123456789abcdef0123456789abcdef";
        let session = format!("{owner}-paused");
        let body = serde_json::to_vec(&salon_core::journal::Submission {
            session: session.clone(),
            sequence: 0,
            line: Line::<String, u32>::Header("paused".repeat(20)),
        })
        .unwrap();
        let first = salon_core::journal::Chunk {
            session: session.clone(),
            sequence: 0,
            offset: 0,
            total: body.len(),
            bytes: body.iter().take(50).copied().collect(),
        };
        assert_eq!(
            receive_chunk::<String, u32>(&mut state, &directory, &first).status,
            202
        );
        // No clock or timestamp exists in transport state. Interleaved
        // activity and an arbitrarily long real pause cannot expire it.
        let other = serde_json::to_vec(&salon_core::journal::Submission {
            session: "other".to_owned(),
            sequence: 0,
            line: Line::<String, u32>::Header("other".into()),
        })
        .unwrap();
        assert_eq!(
            browser_request::<String, u32>(&mut state, &directory, &other).status,
            201
        );
        assert!(state.partial.contains_key(&session));
        let last = salon_core::journal::Chunk {
            offset: 50,
            bytes: body.iter().skip(50).copied().collect(),
            ..first
        };
        assert_eq!(
            receive_chunk::<String, u32>(&mut state, &directory, &last).status,
            201
        );
        assert!(state.partial.is_empty());
        let abandoned = salon_core::journal::Chunk {
            session: format!("{owner}-abandoned"),
            sequence: 0,
            offset: 0,
            total: 2,
            bytes: vec![b'{'],
        };
        assert_eq!(
            receive_chunk::<String, u32>(&mut state, &directory, &abandoned).status,
            202
        );
        let replacement = serde_json::to_vec(&salon_core::journal::Submission {
            session: format!("{owner}-new"),
            sequence: 0,
            line: Line::<String, u32>::Header("replacement".into()),
        })
        .unwrap();
        assert_eq!(
            browser_request::<String, u32>(&mut state, &directory, &replacement).status,
            201
        );
        assert!(state.partial.is_empty());
        let pending = salon_core::journal::Chunk {
            session: format!("{owner}-new"),
            sequence: 1,
            ..abandoned
        };
        assert_eq!(
            receive_chunk::<String, u32>(&mut state, &directory, &pending).status,
            202
        );
        let final_line = serde_json::to_vec(&salon_core::journal::Submission {
            session: format!("{owner}-new"),
            sequence: 1,
            line: Line::<String, u32>::Final {
                fingerprint: "done".into(),
            },
        })
        .unwrap();
        assert_eq!(
            browser_request::<String, u32>(&mut state, &directory, &final_line).status,
            200
        );
        assert!(state.partial.is_empty());
        drop(state);
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn browser_reloads_replace_old_sessions_but_long_idle_does_not_expire_them() {
        let directory = std::env::temp_dir().join(format!(
            "salon-browser-reload-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut sessions = BrowserSessions::new();
        let owner = "0123456789abcdef0123456789abcdef";
        for run in 0..130 {
            let response = receive_browser(
                &mut sessions,
                &directory,
                format!("{owner}-{run}"),
                0,
                Line::Header(serde_json::Value::Null),
            );
            assert_eq!(response.status, 201);
            assert_eq!(sessions.len(), 1);
        }
        let session = format!("{owner}-129");
        assert_eq!(
            receive_browser(
                &mut sessions,
                &directory,
                session.clone(),
                2,
                Line::Entry(serde_json::Value::Null)
            )
            .status,
            409
        );
        assert_eq!(sessions.get(&session).unwrap().next_sequence, 1);
        assert_eq!(
            receive_browser(
                &mut sessions,
                &directory,
                session.clone(),
                1,
                Line::Checkpoint {
                    fingerprint: "empty".into()
                }
            )
            .status,
            200
        );
        assert_eq!(
            receive_browser(
                &mut sessions,
                &directory,
                session.clone(),
                1,
                Line::Entry(serde_json::Value::Null)
            )
            .status,
            409
        );
        // Transport state has no wall clock: arbitrary idle/suspension
        // cannot expire this live recorder. Only explicit lifecycle inputs can.
        let body = serde_json::to_vec(&salon_core::journal::Submission {
            session: session.clone(),
            sequence: 2,
            line: Line::<serde_json::Value, serde_json::Value>::Entry(serde_json::Value::Null),
        })
        .unwrap();
        let mut state = BrowserState {
            sessions,
            partial: std::collections::BTreeMap::new(),
        };
        assert_eq!(
            browser_request::<serde_json::Value, serde_json::Value>(&mut state, &directory, &body)
                .status,
            200
        );
        assert_eq!(state.sessions.get(&session).unwrap().next_sequence, 3);
        assert_eq!(state.sessions.len(), 1);
        drop(state);
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn native_writer_flushes_typed_entries_and_never_overwrites() {
        let directory = std::env::temp_dir().join(format!(
            "salon-journal-writer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("party.jsonl");
        let mut writer = Recorder::<String, u32>::start_file(&path, "settings".into()).unwrap();
        writer.record(7).unwrap();
        writer.checkpoint("seven").unwrap();
        writer.marker(Ms(17), "bug").unwrap();
        writer.finish("seven").unwrap();
        writer.flush().unwrap();
        assert!(matches!(writer.record(8), Err(RecordError::Finished)));
        let read = Journal::<String, u32>::read(&path).unwrap();
        assert_eq!(read.entries, vec![7]);
        assert_eq!(read.markers, vec![(Ms(17), "bug".into())]);
        assert!(read.finished);
        assert!(Recorder::<String, u32>::start_file(&path, "overwrite".into()).is_err());
        drop(writer);
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn browser_sink_validates_types_and_local_authority_before_writing() {
        let directory = std::env::temp_dir().join(format!(
            "salon-browser-sink-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let sink = browser_sink::<String, u32>(directory.clone(), "/journal");
        let request = |local, body: &[u8]| crate::Request {
            method: "POST".into(),
            path: "/journal".into(),
            query: String::new(),
            body: body.to_vec(),
            local,
        };
        let header = serde_json::to_vec(&salon_core::journal::Submission {
            session: "session".to_owned(),
            sequence: 0,
            line: Line::<String, u32>::Header("settings".into()),
        })
        .unwrap();
        assert_eq!(sink(&request(false, &header)).unwrap().status, 403);
        assert_eq!(sink(&request(true, b"invalid")).unwrap().status, 400);
        assert!(!directory.exists());
        assert_eq!(sink(&request(true, &header)).unwrap().status, 201);
        assert_eq!(sink(&request(true, &header)).unwrap().status, 409);
        drop(sink);
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn retention_preserves_recent_files_and_other_extensions() {
        let directory = std::env::temp_dir().join(format!(
            "salon-journal-retention-{}",
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("recent.jsonl"),
            journal(&[Line::Header("owned".into())]),
        )
        .unwrap();
        fs::write(
            directory.join("unrelated.jsonl"),
            "{\"from\":\"another tool\"}\n",
        )
        .unwrap();
        fs::write(directory.join("other.txt"), "x").unwrap();
        let future = wall_time()
            .checked_add(RETENTION + Duration::from_secs(1))
            .unwrap();
        assert!(!owns(&directory.join("missing.jsonl")));
        prune(&directory, future).unwrap();
        assert!(!directory.join("recent.jsonl").exists());
        assert!(directory.join("other.txt").exists());
        assert!(directory.join("unrelated.jsonl").exists());
        fs::remove_dir_all(directory).unwrap();
    }
}
