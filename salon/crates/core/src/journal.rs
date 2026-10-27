//! Typed journal envelopes, shared by native writers, browsers and replayers.
//! Game-specific headers and entries remain the game's Rust types.

use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::Ms;

/// One complete JSON line. A checkpoint records the state after every
/// applied batch, so even an abruptly stopped session has a verified prefix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Line<H, E> {
    Header(H),
    Entry(E),
    Marker { at: Ms, note: String },
    Checkpoint { fingerprint: String },
    Final { fingerprint: String },
}

/// Browser panic information. Never contains a session token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanicReport {
    pub message: String,
    pub page: String,
    pub at: Ms,
    pub attempt: u8,
}

/// Browser journal delivery, decoded by the same Rust type on the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Submission<H, E> {
    pub session: String,
    pub sequence: u64,
    pub line: Line<H, E>,
}

/// Shared page diagnostics, also delivered to the game's active journal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Diagnostic {
    Panic(PanicReport),
    Marker { at: Ms, page: String, note: String },
}
impl Diagnostic {
    pub fn page(&self) -> &str {
        match self {
            Self::Panic(report) => &report.page,
            Self::Marker { page, .. } => page,
        }
    }
    pub fn message(&self) -> &str {
        match self {
            Self::Panic(report) => &report.message,
            Self::Marker { note, .. } => note,
        }
    }
}

/// On-disk sequence envelope. Shared with core-only replay readers; a gap
/// or duplicate must be rejected before replay.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stored<H, E> {
    pub sequence: u64,
    pub line: Line<H, E>,
}

/// SHA-256 of the exact postcard representation. Serialize the complete
/// simulation state, including its RNG, rather than a rendered view. The
/// same Rust type produces the same digest in native and WASM builds.
pub fn fingerprint<T: Serialize>(state: &T) -> Result<String, postcard::Error> {
    use core::fmt::Write as _;
    use sha2::{Digest, Sha256};
    let bytes = postcard::to_allocvec(state)?;
    let digest = Sha256::digest(bytes);
    let mut text = String::new();
    for byte in digest {
        // Writing to a String has no failure path.
        let _: Result<(), core::fmt::Error> = write!(text, "{byte:02x}");
    }
    Ok(text)
}

/// A bounded transport piece for a browser journal line larger than one
/// HTTP request (for example a solo board containing image data URLs).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chunk {
    pub session: String,
    pub sequence: u64,
    pub offset: usize,
    pub total: usize,
    pub bytes: alloc::vec::Vec<u8>,
}
