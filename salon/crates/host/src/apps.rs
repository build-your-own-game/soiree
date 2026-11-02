//! The iPhone apps of the room, by player, for the driver: a game sees players
//! that arrive, leave and come back, never a socket or a reconnection. What
//! an app shows is derived from the game ([`crate::Game::remote`]): salon sends
//! a change, and puts the remote back up after a reconnection.

use std::collections::BTreeMap;

use remote_protocol::web::PlayerId;
use remote_protocol::{DrawEvent, Feedback, GameMessage, InputState, Remote, encode};
use salon_core::{Bucket, Limits, LinkId, Ms, Name, Timing};

use crate::driver::Effect;

/// What an iPhone app did ([`crate::Game::on_app`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppEvent {
    /// A player this run had not seen. Everything is indexed by `player`, the
    /// same on every reconnection, and in every game.
    Arrived {
        player: PlayerId,
        name: Option<Name>,
    },
    /// Its connection dropped, or its pad fell silent. The app retries by
    /// itself: pause, skip a turn, keep the place.
    Left { player: PlayerId },
    /// It came back: salon has already put its remote back up.
    Back { player: PlayerId },
    /// The whole state of its controls, for a [`Remote::Pad`]. The latest one
    /// wins, and an older one never gets here.
    Input { player: PlayerId, state: InputState },
    /// One step of a drawing: all of them, in order.
    Draw { player: PlayerId, event: DrawEvent },
}

/// What an app was last told to show.
enum Shown {
    Nothing,
    /// [`crate::Game::remote`] said `None`.
    Page,
    Chosen(Remote),
}

impl Shown {
    fn is(&self, wanted: Option<&Remote>) -> bool {
        match (self, wanted) {
            (Shown::Page, None) => true,
            (Shown::Chosen(shown), Some(wanted)) => shown == wanted,
            (Shown::Nothing | Shown::Chosen(_), None) | (Shown::Nothing | Shown::Page, Some(_)) => {
                false
            }
        }
    }
}

struct App {
    /// `None`: away.
    link: Option<LinkId>,
    shown: Shown,
    heard: Ms,
    /// The latest state of this connection.
    seq: Option<u32>,
    frames: Bucket,
}

/// Every app the room knows, at most [`Limits::max_devices`].
pub(crate) struct Apps {
    players: BTreeMap<PlayerId, App>,
    page: Option<String>,
    timing: Timing,
    limits: Limits,
}

fn transmit(effects: &mut Vec<Effect>, link: LinkId, message: &GameMessage) {
    effects.push(Effect::Transmit {
        link,
        bytes: encode(message),
    });
}

impl Apps {
    pub(crate) fn new(timing: Timing, limits: Limits) -> Apps {
        Apps {
            players: BTreeMap::new(),
            page: None,
            timing,
            limits,
        }
    }

    /// The phone page, what an app shows unless the game chooses.
    pub(crate) fn set_page(&mut self, page: String) {
        self.page = Some(page);
    }

    /// The player's place. A new one takes that of a player who is away, or
    /// is refused: the table does not grow with what strangers claim to be.
    fn seat(
        &mut self,
        now: Ms,
        link: LinkId,
        player: PlayerId,
        effects: &mut Vec<Effect>,
    ) -> Option<&mut App> {
        if !self.players.contains_key(&player) && self.players.len() >= self.limits.max_devices {
            let away = self
                .players
                .iter()
                .find_map(|(player, app)| app.link.is_none().then_some(*player));
            let Some(away) = away else {
                let reason = "This game is full.".to_owned();
                transmit(effects, link, &GameMessage::Refuse { reason });
                effects.push(Effect::Close { link });
                return None;
            };
            self.players.remove(&away);
        }
        let limits = self.limits;
        Some(self.players.entry(player).or_insert_with(|| App {
            link: None,
            shown: Shown::Nothing,
            heard: now,
            seq: None,
            frames: Bucket::new(now, limits.packets_per_s, limits.packets_burst),
        }))
    }

    pub(crate) fn hello(
        &mut self,
        now: Ms,
        link: LinkId,
        player: PlayerId,
        name: &str,
        events: &mut Vec<AppEvent>,
        effects: &mut Vec<Effect>,
    ) {
        // The same connection, as someone else: the previous one has left.
        for (other, app) in &mut self.players {
            if *other != player && app.link == Some(link) {
                app.link = None;
                app.shown = Shown::Nothing;
                events.push(AppEvent::Left { player: *other });
            }
        }
        let known = self.players.contains_key(&player);
        let Some(app) = self.seat(now, link, player, effects) else {
            return;
        };
        if !app.frames.allow(now) {
            return;
        }
        app.heard = now;
        app.shown = Shown::Nothing;
        match app.link.replace(link) {
            Some(old) if old != link => {
                app.seq = None;
                effects.push(Effect::Close { link: old });
            }
            Some(_) => {}
            None if known => {
                app.seq = None;
                events.push(AppEvent::Back { player });
            }
            None => events.push(AppEvent::Arrived {
                player,
                name: Name::new(name),
            }),
        }
    }

    /// The app on `link`, if the frame fits its budget. Above, frames are
    /// dropped unread, as those of a device.
    fn on_link(&mut self, now: Ms, link: LinkId) -> Option<(PlayerId, &mut App)> {
        let (player, app) = self
            .players
            .iter_mut()
            .find(|(_, app)| app.link == Some(link))?;
        app.heard = now;
        app.frames.allow(now).then_some((*player, app))
    }

    pub(crate) fn input(
        &mut self,
        now: Ms,
        link: LinkId,
        state: InputState,
        events: &mut Vec<AppEvent>,
    ) {
        let Some((player, app)) = self.on_link(now, link) else {
            return;
        };
        if app.seq.is_none_or(|latest| state.seq >= latest) {
            app.seq = Some(state.seq);
            events.push(AppEvent::Input { player, state });
        }
    }

    pub(crate) fn draw(
        &mut self,
        now: Ms,
        link: LinkId,
        event: DrawEvent,
        events: &mut Vec<AppEvent>,
    ) {
        if let Some((player, _)) = self.on_link(now, link) {
            events.push(AppEvent::Draw { player, event });
        }
    }

    pub(crate) fn closed(&mut self, link: LinkId, events: &mut Vec<AppEvent>) {
        if let Some((player, app)) = self
            .players
            .iter_mut()
            .find(|(_, app)| app.link == Some(link))
        {
            app.link = None;
            app.shown = Shown::Nothing;
            events.push(AppEvent::Left { player: *player });
        }
    }

    /// A pad sends at least every 250 ms: its silence means a phone gone
    /// without a word. A page may stay quiet as long as it likes.
    pub(crate) fn expire(
        &mut self,
        now: Ms,
        events: &mut Vec<AppEvent>,
        effects: &mut Vec<Effect>,
    ) {
        for (player, app) in &mut self.players {
            if let Some(link) = app.link
                && matches!(app.shown, Shown::Chosen(Remote::Pad(_)))
                && now.since(app.heard) >= self.timing.absent
            {
                app.link = None;
                app.shown = Shown::Nothing;
                effects.push(Effect::Close { link });
                events.push(AppEvent::Left { player: *player });
            }
        }
    }

    /// Sends each connected app what the game wants it to show, if that
    /// changed. Allocates nothing for an app that stays on the phone page.
    pub(crate) fn show(
        &mut self,
        now: Ms,
        remote: impl Fn(PlayerId) -> Option<Remote>,
        effects: &mut Vec<Effect>,
    ) {
        for (player, app) in &mut self.players {
            let Some(link) = app.link else {
                continue;
            };
            let wanted = remote(*player);
            if app.shown.is(wanted.as_ref()) {
                continue;
            }
            let (message, shown) = match (wanted, &self.page) {
                (Some(remote), _) => (remote.clone(), Shown::Chosen(remote)),
                (None, Some(url)) => (Remote::Web { url: url.clone() }, Shown::Page),
                // No page, no door: no app can get here.
                (None, None) => continue,
            };
            transmit(effects, link, &GameMessage::Show(message));
            app.heard = now;
            app.shown = shown;
        }
    }

    /// A vibration for a player who is away would come too late: it is lost.
    pub(crate) fn feedback(&self, feedback: Vec<(PlayerId, Feedback)>, effects: &mut Vec<Effect>) {
        for (player, feedback) in feedback {
            if let Some(link) = self.players.get(&player).and_then(|app| app.link) {
                transmit(effects, link, &GameMessage::Feedback(feedback));
            }
        }
    }
}
