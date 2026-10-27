//! A typed replay header, ordered inputs and an exact-state check.
use crate::{Game, Input};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Header {
    pub format: u16,
    pub version: u32,
    pub seed: u64,
}
impl Default for Header {
    fn default() -> Self {
        Self {
            format: 1,
            version: crate::VERSION,
            seed: 0,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub at: u64,
    pub input: Input,
}
pub fn fingerprint(game: &Game) -> Result<String, String> {
    salon_core::journal::fingerprint(game).map_err(|error| error.to_string())
}
pub fn replay(header: &Header, entries: &[Entry], expected: &str) -> Result<Game, String> {
    if header.format != 1 || header.seed != 0 {
        return Err("unsupported replay format".into());
    }
    let mut game = Game::default();
    let mut previous = 0;
    for entry in entries {
        if entry.at < previous {
            return Err("replay time moved backwards".into());
        }
        previous = entry.at;
        game.apply(entry.input.clone());
    }
    if fingerprint(&game)? != expected {
        return Err("replay state mismatch".into());
    }
    Ok(game)
}
