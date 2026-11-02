//! Identities and the TV key, kept on disk so a restart does not forget
//! anybody, and the TV page still holds the right key. Written to a temporary
//! file then renamed: a crash while saving leaves the previous file whole.
//!
//! The epoch is not kept: it is drawn at each start. Counted from this file, it
//! went back to 1 when the file was lost, and stayed at 1 in a folder that
//! cannot be written. A page then took the new run for the one it knew, and
//! heard nothing more.

use std::fs;
use std::path::{Path, PathBuf};

use salon_core::{DeviceId, Epoch, Kind, Known, Name, Token};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    /// Absent in files written before the key was kept. Files of that time
    /// also name an epoch, which reads as nothing now.
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
    /// start, never a failure) and draws this run's epoch. The TV key stays
    /// the same across runs, so the TV page reconnects by itself after a
    /// restart; `fresh_key` is used only the first time.
    ///
    /// Fails only without secure random numbers: a fixed number in their place
    /// could be the epoch of the run a page remembers.
    pub fn open(dir: Option<&Path>, fresh_key: [u8; 16]) -> Result<Store, String> {
        let epoch = salon_crypto::random::<4>()
            .map_err(|error| format!("no secure random numbers: {error}"))?;
        // 0 means "no run yet" on the wire. Two runs share a number once in
        // 2^32: a counter could not promise more without a folder to keep it.
        let epoch = Epoch(u32::from_le_bytes(epoch).max(1));
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
        Ok(Store {
            path,
            epoch,
            tv_key,
            saved: known,
        })
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

    /// Saves `known` if it changed. What could not be written goes again at
    /// the next call.
    pub fn save(&mut self, known: Vec<Known>) {
        if known != self.saved {
            self.write(known);
        }
    }

    /// Writes the file at start, so the next run finds the TV key even if no
    /// device ever joins.
    pub fn save_at_start(&mut self) {
        self.write(self.saved.clone());
    }

    fn write(&mut self, known: Vec<Known>) {
        let Some(path) = &self.path else {
            self.saved = known;
            return;
        };
        let saved = Saved {
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
            // A failure shows on the write just below.
            let _: std::io::Result<()> = fs::create_dir_all(dir);
        }
        let partial = path.with_extension("json.partial");
        if fs::write(&partial, bytes).is_ok() && fs::rename(&partial, path).is_ok() {
            self.saved = known;
        } else {
            crate::say(format_args!(
                "salon: cannot save identities to {}",
                path.display()
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use salon_core::{
        Caps, Client, ClientConfig, ClientOutput, Host, HostConfig, Limits, LinkId, Ms, Output,
        Status, Timing, TvKey,
    };

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("salon-store-{name}-{}", std::process::id()));
        let _: std::io::Result<()> = fs::remove_dir_all(&dir);
        dir
    }

    /// C7: after a restart, the epoch moves on, the TV key and the devices
    /// stay.
    #[test]
    fn a_restart_keeps_the_tv_key_and_the_devices() {
        let dir = scratch("restart");
        let mut first = Store::open(Some(&dir), [1; 16]).unwrap();
        first.save_at_start();
        let ada = Known {
            device: DeviceId(3),
            token: Token([5; 16]),
            kind: Kind::Phone,
            name: Name::new("Ada"),
        };
        first.save(vec![ada.clone()]);
        let second = Store::open(Some(&dir), [2; 16]).unwrap();
        assert_eq!(second.tv_key(), [1; 16]);
        assert_ne!(second.epoch(), first.epoch());
        assert_eq!(second.known(), vec![ada]);
        let _: std::io::Result<()> = fs::remove_dir_all(&dir);
    }

    /// C9 on the host's side: a broken file is a fresh start.
    #[test]
    fn a_broken_file_is_a_fresh_start() {
        let dir = scratch("broken");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("salon.json"), b"{ not json").unwrap();
        let store = Store::open(Some(&dir), [4; 16]).unwrap();
        assert_eq!(store.tv_key(), [4; 16]);
        assert_eq!(store.known(), Vec::new());
        let _: std::io::Result<()> = fs::remove_dir_all(&dir);
    }

    /// A write that fails is logged, and the next save tries again.
    #[test]
    fn what_could_not_be_written_goes_again() {
        let dir = scratch("again");
        fs::create_dir_all(&dir).unwrap();
        let blocked = dir.join("not-a-folder");
        fs::write(&blocked, b"").unwrap();
        let mut store = Store::open(Some(&blocked), [1; 16]).unwrap();
        let ada = Known {
            device: DeviceId(3),
            token: Token([5; 16]),
            kind: Kind::Phone,
            name: None,
        };
        store.save(vec![ada.clone()]);
        assert_eq!(store.known(), Vec::new());
        fs::remove_file(&blocked).unwrap();
        store.save(vec![ada.clone()]);
        assert_eq!(Store::open(Some(&blocked), [2; 16]).unwrap().known(), [ada]);
        let _: std::io::Result<()> = fs::remove_dir_all(&dir);
    }

    /// A file of the versions that counted epochs still gives what it holds.
    #[test]
    fn a_file_that_names_an_epoch_still_reads() {
        let dir = scratch("epoch");
        fs::create_dir_all(&dir).unwrap();
        let key = format!("{:?}", [1; 16]);
        let token = format!("{:?}", [5; 16]);
        let file = format!(
            r#"{{"epoch":4,"tv_key":{key},"devices":[{{"device":3,"token":{token},"kind":"Phone","name":"Ada"}}]}}"#
        );
        fs::write(dir.join("salon.json"), file).unwrap();
        let store = Store::open(Some(&dir), [2; 16]).unwrap();
        assert_eq!(store.tv_key(), [1; 16]);
        assert_eq!(store.known().len(), 1);
        let _: std::io::Result<()> = fs::remove_dir_all(&dir);
    }

    /// One step of the page and the run it talks to: what the page got.
    fn exchange(
        room: &mut Host<u32, u32>,
        page: &mut Client<u32, u32>,
        now: Ms,
    ) -> Vec<ClientOutput<u32>> {
        let mut got = Vec::new();
        while let Some(output) = page.poll(now) {
            if let ClientOutput::Transmit(bytes) = output {
                room.received(now, LinkId(1), &bytes);
            } else {
                got.push(output);
            }
        }
        while let Some(output) = room.poll(now) {
            if let Output::Transmit { bytes, .. } = output {
                page.received(now, &bytes);
            }
        }
        while let Some(output) = page.poll(now) {
            got.push(output);
        }
        got
    }

    fn run_of(store: &Store) -> Host<u32, u32> {
        let mut room = Host::new(
            Ms(0),
            HostConfig {
                game: 1,
                epoch: store.epoch(),
                tv_key: TvKey(store.tv_key()),
                timing: Timing::default(),
                limits: Limits::default(),
                known: store.known(),
            },
        );
        room.opened(Ms(0), LinkId(1), Caps::STREAM);
        room
    }

    /// Whether the folder lost its file or was never written, a new run is a
    /// new run: the same number told a page "same run". It got no `Reset`, no
    /// state, and what it sent was acknowledged and lost after `send` said `Ok`.
    #[test]
    fn a_lost_or_unwritable_folder_cannot_repeat_an_epoch() {
        let lost = scratch("lost");
        let unwritable = PathBuf::from("/dev/null/salon");
        for dir in [lost.as_path(), unwritable.as_path()] {
            let mut store = Store::open(Some(dir), [1; 16]).unwrap();
            store.save_at_start();
            let mut room = run_of(&store);
            let mut page: Client<u32, u32> = Client::new(
                Ms(0),
                ClientConfig {
                    game: 1,
                    kind: Kind::Phone,
                    token: Token([3; 16]),
                    name: None,
                    tv_key: None,
                    seed: 1,
                    timing: Timing::default(),
                    limits: Limits::default(),
                },
            );
            let got = exchange(&mut room, &mut page, Ms(0));
            assert!(got.contains(&ClientOutput::Connect), "{got:?}");
            page.opened(Ms(0), Caps::STREAM);
            exchange(&mut room, &mut page, Ms(0));
            assert_eq!(page.status(), Status::Connected);

            let _: std::io::Result<()> = fs::remove_file(dir.join("salon.json"));
            let store = Store::open(Some(dir), [1; 16]).unwrap();
            let mut room = run_of(&store);
            page.closed(Ms(1_000));
            let got = exchange(&mut room, &mut page, Ms(5_000));
            assert!(got.contains(&ClientOutput::Connect), "{got:?}");
            page.opened(Ms(5_000), Caps::STREAM);
            let got = exchange(&mut room, &mut page, Ms(5_000));
            assert_eq!(page.status(), Status::Connected);
            assert!(
                got.contains(&ClientOutput::Reset),
                "the page took a new run for the one it knew: {got:?}"
            );
        }
        let _: std::io::Result<()> = fs::remove_dir_all(&lost);
    }
}
