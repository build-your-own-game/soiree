//! What the demo's devices and host say, shared by both sides.

use salon_core::Qr;
use serde::{Deserialize, Serialize};

/// The game's version: salon refuses a page built from other sources
/// (`salon/version.rs`).
pub use crate::VERSION;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Up {
    Tap,
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
    /// For the TV: where phones join.
    pub join: Option<Join>,
    /// For a phone: its own taps.
    pub mine: Option<u32>,
}

/// The address phones open, and its QR code.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Join {
    pub url: String,
    pub qr: Qr,
}
