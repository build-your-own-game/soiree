//! Identities, the epoch and the TV key, kept on disk so a restart does not
//! forget anybody, and the TV page still holds the right key. Written to a temporary file then renamed: a crash while saving
//! leaves the previous file whole.

use std::fs;
use std::path::{Path, PathBuf};

use salon_core::{DeviceId, Epoch, Kind, Known, Name, Token};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    epoch: u32,
    /// Absent in files written before the key was kept.
    #[serde(default)]
    tv_key: Option<[u8; 16]>,
    devices: Vec<SavedDevice>,
}

#[derive(Serialize, Deserialize)]
struct SavedDevice {
    device: u32,
    token: [u8; 16],
    kind: Kind,
    name: Option<String>,
}

pub struct Store {
    path: Option<PathBuf>,
    epoch: Epoch,
    tv_key: [u8; 16],
    saved: Vec<Known>,
}

impl Store {
    /// Reads what the last run left (a missing or broken file is a fresh
    /// start, never a failure) and moves to the next epoch. The TV key stays
    /// the same across runs, so the TV page reconnects by itself after a
    /// restart; `fresh_key` is used only the first time.
    pub fn open(dir: Option<&Path>, fallback_epoch: u32, fresh_key: [u8; 16]) -> Store {
        let path = dir.map(|dir| dir.join("salon.json"));
        let saved: Saved = path
            .as_ref()
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let tv_key = saved.tv_key.unwrap_or(fresh_key);
        let known = saved
            .devices
            .into_iter()
            .map(|device| Known {
                device: DeviceId(device.device),
                token: Token(device.token),
                kind: device.kind,
                name: device.name.as_deref().and_then(Name::new),
            })
            .collect();
        let epoch = if path.is_some() {
            saved.epoch.wrapping_add(1).max(1)
        } else {
            fallback_epoch
        };
        Store {
            path,
            epoch: Epoch(epoch),
            tv_key,
            saved: known,
        }
    }

    pub fn epoch(&self) -> Epoch {
        self.epoch
    }

    pub fn tv_key(&self) -> [u8; 16] {
        self.tv_key
    }

    pub fn known(&self) -> Vec<Known> {
        self.saved.clone()
    }

    /// Saves `known` if it changed.
    pub fn save(&mut self, known: Vec<Known>) {
        if known != self.saved {
            self.write(known);
        }
    }

    /// Writes the epoch at start, so the next run moves past it even if no
    /// device ever joins.
    pub fn save_epoch(&mut self) {
        self.write(self.saved.clone());
    }

    fn write(&mut self, known: Vec<Known>) {
        let Some(path) = &self.path else {
            self.saved = known;
            return;
        };
        let saved = Saved {
            epoch: self.epoch.0,
            tv_key: Some(self.tv_key),
            devices: known
                .iter()
                .map(|known| SavedDevice {
                    device: known.device.0,
                    token: known.token.0,
                    kind: known.kind,
                    name: known.name.as_ref().map(|name| name.as_str().to_owned()),
                })
                .collect(),
        };
        let Ok(bytes) = serde_json::to_vec(&saved) else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let partial = path.with_extension("json.partial");
        if fs::write(&partial, bytes).is_ok() && fs::rename(&partial, path).is_ok() {
            self.saved = known;
        } else {
            eprintln!("salon: cannot save identities to {}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("salon-store-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    /// C7: after a restart, the epoch moves on, the TV key and the devices
    /// stay.
    #[test]
    fn a_restart_keeps_the_tv_key_and_the_devices() {
        let dir = scratch("restart");
        let mut first = Store::open(Some(&dir), 7, [1; 16]);
        first.save_epoch();
        let ada = Known {
            device: DeviceId(3),
            token: Token([5; 16]),
            kind: Kind::Phone,
            name: Name::new("Ada"),
        };
        first.save(vec![ada.clone()]);
        let second = Store::open(Some(&dir), 99, [2; 16]);
        assert_eq!(second.tv_key(), [1; 16]);
        assert!(second.epoch() > first.epoch());
        assert_eq!(second.known(), vec![ada]);
        let _ = fs::remove_dir_all(&dir);
    }

    /// C9 on the host's side: a broken file is a fresh start.
    #[test]
    fn a_broken_file_is_a_fresh_start() {
        let dir = scratch("broken");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("salon.json"), b"{ not json").unwrap();
        let store = Store::open(Some(&dir), 7, [4; 16]);
        assert_eq!(store.tv_key(), [4; 16]);
        assert_eq!(store.known(), Vec::new());
        let _ = fs::remove_dir_all(&dir);
    }
}
