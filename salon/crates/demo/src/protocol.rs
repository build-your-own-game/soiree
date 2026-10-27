//! What the demo's devices and host say, shared by both sides.

use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Up {
    Tap,
    Name(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    pub taps: u32,
    pub here: bool,
    pub rtt: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct View {
    pub taps: u32,
    pub players: Vec<Player>,
    /// For the TV: the address and QR code phones use.
    pub join: Option<(String, String)>,
    /// For a phone: its own taps.
    pub mine: Option<u32>,
}
