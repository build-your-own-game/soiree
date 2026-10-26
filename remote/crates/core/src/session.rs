//! The phone's side of the connection to a game: which frames to send and
//! when, and what the game's frames change. The app owns the WebSocket; this
//! owns the messages, so it is tested without a network.
//!
//! Inputs are state, not events (Sidequest's lessons): one pending state, the
//! latest wins, sent when it changes but at most 60 times a second, and every
//! 250 ms anyway so a game that missed one catches up. Drawing steps are
//! events and all go, in order, before the state.

use remote_protocol::{
    Feedback, GameMessage, InputState, PhoneMessage, Remote as Shown, VERSION, decode, encode,
};

use crate::remote::Remote;
use crate::{Insets, Size};

/// The fastest a state goes out, in seconds.
const MIN_INTERVAL: f64 = 1.0 / 60.0;
/// The slowest: an unchanged state is sent again this often.
const HEARTBEAT: f64 = 0.25;

/// What a frame from the game changed, for the app to act on.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Draw [`Session::remote`] from now on.
    ShowPad,
    /// Show the game's page, at this absolute or relative address.
    ShowWeb(String),
    Feedback(Feedback),
    Refused(String),
    /// A frame that could not be read: kept out, nothing changes.
    Unreadable,
}

#[derive(Clone, Debug)]
pub struct Session {
    player: [u8; 16],
    name: String,
    screen: Size,
    insets: Insets,
    remote: Option<Remote>,
    seq: u32,
    /// When the last state went out, and what it was (its `seq` aside).
    last_sent: Option<(f64, InputState)>,
    outbox: Vec<Vec<u8>>,
}

impl Session {
    pub fn new(player: [u8; 16], name: String, screen: Size, insets: Insets) -> Self {
        Self {
            player,
            name,
            screen,
            insets,
            remote: None,
            seq: 0,
            last_sent: None,
            outbox: Vec::new(),
        }
    }

    /// The socket (re)opened: say hello again, with the same identity.
    pub fn connected(&mut self) {
        self.outbox.push(encode(&PhoneMessage::Hello {
            version: VERSION,
            player: self.player,
            name: self.name.clone(),
        }));
        self.last_sent = None;
    }

    pub fn receive(&mut self, frame: &[u8]) -> Event {
        match decode::<GameMessage>(frame) {
            Ok(GameMessage::Show(Shown::Pad(layout))) => {
                if let Some(old) = &mut self.remote {
                    old.release_all();
                }
                self.remote = Some(Remote::new(layout, self.screen, self.insets));
                self.last_sent = None;
                Event::ShowPad
            }
            Ok(GameMessage::Show(Shown::Web { url })) => {
                self.remote = None;
                Event::ShowWeb(url)
            }
            Ok(GameMessage::Feedback(feedback)) => Event::Feedback(feedback),
            Ok(GameMessage::Refuse { reason }) => Event::Refused(reason),
            Err(_) => Event::Unreadable,
        }
    }

    pub fn resize(&mut self, screen: Size, insets: Insets) {
        self.screen = screen;
        self.insets = insets;
        if let Some(remote) = &mut self.remote {
            remote.resize(screen, insets);
        }
    }

    pub fn remote(&self) -> Option<&Remote> {
        self.remote.as_ref()
    }

    pub fn remote_mut(&mut self) -> Option<&mut Remote> {
        self.remote.as_mut()
    }

    /// Queues what is due at `now` (seconds): drawing steps, then the state.
    pub fn tick(&mut self, now: f64) {
        let Some(remote) = &mut self.remote else {
            return;
        };
        for event in remote.drain_drawing() {
            self.outbox.push(encode(&PhoneMessage::Draw(event)));
        }
        let state = remote.state(0);
        let due = match &self.last_sent {
            None => true,
            Some((time, last)) => {
                let elapsed = now - time;
                (elapsed >= MIN_INTERVAL && *last != state) || elapsed >= HEARTBEAT
            }
        };
        if due {
            self.seq = self.seq.wrapping_add(1);
            self.outbox.push(encode(&PhoneMessage::Input(InputState {
                seq: self.seq,
                ..state.clone()
            })));
            self.last_sent = Some((now, state));
        }
    }

    /// Takes the frames to send, in order.
    pub fn drain(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.outbox)
    }
}

#[cfg(test)]
mod tests {
    use remote_protocol::{DrawEvent, Haptic, Remote as Shown, presets};

    use super::*;
    use crate::Point;
    use crate::tests::PHONES;

    fn session() -> Session {
        let (screen, insets) = PHONES[0];
        Session::new([3; 16], "Axel".into(), screen, insets)
    }

    fn sent(session: &mut Session) -> Vec<PhoneMessage> {
        session
            .drain()
            .iter()
            .filter_map(|frame| decode(frame).ok())
            .collect()
    }

    fn show(session: &mut Session, layout: remote_protocol::Layout) -> Event {
        session.receive(&encode(&GameMessage::Show(Shown::Pad(layout))))
    }

    #[test]
    fn hello_again_after_each_reconnection_with_the_same_player() {
        let mut session = session();
        session.connected();
        session.connected();
        let hello = PhoneMessage::Hello {
            version: VERSION,
            player: [3; 16],
            name: "Axel".into(),
        };
        assert_eq!(sent(&mut session), [hello.clone(), hello]);
    }

    #[test]
    fn the_game_picks_the_remote_and_can_change_it() {
        let mut session = session();
        assert_eq!(show(&mut session, presets::buzzer()), Event::ShowPad);
        assert_eq!(session.remote().map(|r| r.layout().buttons.len()), Some(1));
        assert_eq!(show(&mut session, presets::n64()), Event::ShowPad);
        assert!(
            session
                .remote()
                .is_some_and(|r| r.layout().sticks.len() == 1)
        );
        let web = session.receive(&encode(&GameMessage::Show(Shown::Web {
            url: "/free".into(),
        })));
        assert_eq!(web, Event::ShowWeb("/free".into()));
        assert!(session.remote().is_none());
        let feedback = Feedback {
            haptic: Some(Haptic::Success),
            color: None,
        };
        assert_eq!(
            session.receive(&encode(&GameMessage::Feedback(feedback))),
            Event::Feedback(feedback)
        );
        assert_eq!(session.receive(&[0xFF, 0xFF]), Event::Unreadable);
    }

    #[test]
    fn a_state_goes_out_on_change_at_most_60_times_a_second_and_every_250_ms() {
        let mut session = session();
        show(&mut session, presets::buzzer());
        session.tick(0.0);
        assert!(matches!(
            sent(&mut session).as_slice(),
            [PhoneMessage::Input(InputState {
                seq: 1,
                pressed: 0,
                ..
            })]
        ));

        let buzz = session
            .remote()
            .map(|r| r.layout().buttons[0].center)
            .unwrap_or_default();
        session.remote_mut().map(|r| r.touch_down(1, buzz, 0.005));
        session.tick(0.005);
        assert_eq!(sent(&mut session), [], "too soon after the last one");
        session.tick(0.02);
        assert!(matches!(
            sent(&mut session).as_slice(),
            [PhoneMessage::Input(InputState {
                seq: 2,
                pressed: 1,
                ..
            })]
        ));

        session.tick(0.1);
        assert_eq!(sent(&mut session), [], "unchanged");
        session.tick(0.3);
        assert!(matches!(
            sent(&mut session).as_slice(),
            [PhoneMessage::Input(InputState {
                seq: 3,
                pressed: 1,
                ..
            })]
        ));
    }

    #[test]
    fn drawing_steps_all_go_in_order_before_the_state() {
        let mut session = session();
        show(&mut session, presets::draw());
        let rect = session
            .remote()
            .and_then(Remote::canvas_rect)
            .unwrap_or_default();
        let p = Point::new(rect.center_x(), rect.center_y());
        if let Some(remote) = session.remote_mut() {
            remote.touch_down(1, p, 0.0);
            remote.touch_move(1, Point::new(p.x + 20.0, p.y), 0.01);
            remote.touch_up(1);
        }
        session.tick(0.02);
        let messages = sent(&mut session);
        assert!(matches!(
            messages.first(),
            Some(PhoneMessage::Draw(DrawEvent::Begin { stroke: 0, .. }))
        ));
        assert!(matches!(
            messages.get(messages.len() - 2),
            Some(PhoneMessage::Draw(DrawEvent::End { stroke: 0 }))
        ));
        assert!(matches!(messages.last(), Some(PhoneMessage::Input(_))));
    }
}
