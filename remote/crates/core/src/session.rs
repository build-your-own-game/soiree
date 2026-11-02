//! The phone's side of the connection to a game: which frames to send and
//! when, and what the game's frames change. The app owns the WebSocket; this
//! owns the messages, so it is tested without a network.
//!
//! Inputs are state, not events (Sidequest's lessons): one pending state, the
//! latest wins, sent when it changes but at most 60 times a second, and every
//! 250 ms anyway so a game that missed one catches up. Drawing steps are
//! events and all go, in order, before the state.
//!
//! Between two sockets, the drawing steps wait (`CONTRAT.md` promises them
//! all) and the states do not: the next one is read fresh. A new socket says
//! `Hello` first, and what waited goes once the game has shown its remote:
//! salon drops the frames of an app that shows no pad yet.

use std::collections::VecDeque;

use remote_protocol::{
    DrawEvent, Feedback, GameMessage, InputState, PhoneMessage, Remote as Shown, VERSION, decode,
    encode,
};

use crate::remote::Remote;
use crate::{Insets, Size};

/// The fastest a state goes out, in seconds.
const MIN_INTERVAL: f64 = 1.0 / 60.0;
/// The slowest: an unchanged state is sent again this often.
const HEARTBEAT: f64 = 0.25;
/// How fast drawing steps that waited go out, a second. Salon takes 240
/// frames a second from an app, and states may take 60 of them.
const BACKLOG_RATE: f64 = 180.0;

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

/// Where the socket stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Link {
    /// Open, and the game knows this phone.
    Up,
    /// Gone: nothing can leave.
    Down,
    /// `Hello` is on its way, and the game has not shown anything since.
    Greeted,
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
    link: Link,
    /// Drawing steps waiting for a game that can read them, oldest first.
    backlog: VecDeque<Vec<u8>>,
    /// How many of them may go now, and when that was reckoned.
    allowance: (f64, f64),
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
            link: Link::Up,
            backlog: VecDeque::new(),
            allowance: (0.0, 0.0),
        }
    }

    /// The socket (re)opened: say hello again, with the same identity.
    pub fn connected(&mut self) {
        self.outbox.push(encode(&PhoneMessage::Hello {
            version: VERSION,
            player: self.player,
            name: self.name.clone(),
        }));
        if self.link == Link::Down {
            self.link = Link::Greeted;
        }
        self.last_sent = None;
    }

    /// The socket is gone. What had not left stays for the next one, if it
    /// is a drawing step: an older state is worth nothing by then.
    pub fn disconnected(&mut self) {
        self.link = Link::Down;
        self.last_sent = None;
        let unsent = std::mem::take(&mut self.outbox);
        let steps = unsent
            .into_iter()
            .filter(|frame| matches!(decode(frame), Ok(PhoneMessage::Draw(_))));
        let mut backlog: VecDeque<Vec<u8>> = steps.collect();
        backlog.append(&mut self.backlog);
        self.backlog = backlog;
    }

    pub fn receive(&mut self, frame: &[u8]) -> Event {
        match decode::<GameMessage>(frame) {
            Ok(GameMessage::Show(Shown::Pad(layout))) => {
                self.let_go();
                self.remote = Some(Remote::new(layout, self.screen, self.insets));
                self.shown();
                Event::ShowPad
            }
            Ok(GameMessage::Show(Shown::Web { url })) => {
                self.let_go();
                self.remote = None;
                self.shown();
                Event::ShowWeb(url)
            }
            Ok(GameMessage::Feedback(feedback)) => Event::Feedback(feedback),
            Ok(GameMessage::Refuse { reason }) => Event::Refused(reason),
            Err(_) => Event::Unreadable,
        }
    }

    /// Before the remote is replaced: every finger lets go, and the drawing
    /// steps still in its canvas go out, the stroke's `End` included. The
    /// socket delivers every stroke (`CONTRAT.md`), even one the game cut
    /// short by changing the remote.
    fn let_go(&mut self) {
        let Some(old) = &mut self.remote else {
            return;
        };
        old.release_all();
        for event in old.drain_drawing() {
            self.push_step(&event);
        }
    }

    /// The game answered this socket: what waited may go.
    fn shown(&mut self) {
        self.last_sent = None;
        if self.link == Link::Greeted {
            self.link = Link::Up;
        }
    }

    /// A step leaves behind those that wait, never ahead of them.
    fn push_step(&mut self, event: &DrawEvent) {
        let frame = encode(&PhoneMessage::Draw(*event));
        if self.link == Link::Up && self.backlog.is_empty() {
            self.outbox.push(frame);
        } else {
            self.backlog.push_back(frame);
        }
    }

    /// Lets out what the elapsed time allows of the backlog.
    fn release_backlog(&mut self, now: f64) {
        if self.link != Link::Up || self.backlog.is_empty() {
            self.allowance = (now, 0.0);
            return;
        }
        let (since, earned) = self.allowance;
        let mut earned = earned + (now - since).max(0.0) * BACKLOG_RATE;
        while earned >= 1.0 {
            let Some(frame) = self.backlog.pop_front() else {
                break;
            };
            self.outbox.push(frame);
            earned -= 1.0;
        }
        self.allowance = (now, earned);
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
        let steps = self
            .remote
            .as_mut()
            .map(Remote::drain_drawing)
            .unwrap_or_default();
        for event in &steps {
            self.push_step(event);
        }
        self.release_backlog(now);
        let Some(remote) = self.remote.as_ref().filter(|_| self.link == Link::Up) else {
            return;
        };
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
    use remote_protocol::{Haptic, Remote as Shown, presets};

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

    /// Starts a stroke on a shown canvas, on a socket that has opened.
    fn drawing() -> (Session, Point) {
        let mut session = session();
        session.connected();
        show(&mut session, presets::draw());
        let rect = session
            .remote()
            .and_then(Remote::canvas_rect)
            .unwrap_or_default();
        let at = Point::new(rect.center_x(), rect.center_y());
        session.remote_mut().map(|r| r.touch_down(1, at, 0.0));
        session.tick(0.0);
        session.drain();
        (session, at)
    }

    #[test]
    fn what_is_drawn_between_two_sockets_waits_behind_hello_and_the_games_show() {
        let (mut session, at) = drawing();
        session.disconnected();
        if let Some(remote) = session.remote_mut() {
            remote.touch_move(1, Point::new(at.x + 40.0, at.y), 0.1);
            remote.touch_up(1);
        }
        session.tick(0.5);
        assert_eq!(sent(&mut session), [], "no socket: nothing leaves");
        session.tick(1.0);

        // A try that opens and dies spends nothing kept.
        session.connected();
        assert!(matches!(
            sent(&mut session).as_slice(),
            [PhoneMessage::Hello { .. }]
        ));
        session.tick(1.1);
        assert_eq!(sent(&mut session), [], "the game has not shown yet");
        session.disconnected();
        session.tick(1.5);

        session.connected();
        assert!(matches!(
            sent(&mut session).as_slice(),
            [PhoneMessage::Hello { .. }]
        ));
        show(&mut session, presets::draw());
        session.tick(1.6);
        session.tick(3.0);
        let steps: Vec<PhoneMessage> = sent(&mut session)
            .into_iter()
            .filter(|message| matches!(message, PhoneMessage::Draw(_)))
            .collect();
        assert!(matches!(
            steps.first(),
            Some(PhoneMessage::Draw(DrawEvent::Point { .. }))
        ));
        assert!(matches!(
            steps.last(),
            Some(PhoneMessage::Draw(DrawEvent::End { .. }))
        ));
    }

    #[test]
    fn a_state_from_before_a_cut_is_not_kept() {
        let (mut session, _) = drawing();
        session.tick(0.5);
        session.disconnected();
        session.tick(1.0);
        session.connected();
        show(&mut session, presets::draw());
        session.tick(2.0);
        let states = sent(&mut session)
            .into_iter()
            .filter(|message| matches!(message, PhoneMessage::Input(_)))
            .count();
        assert_eq!(states, 1);
    }

    #[test]
    fn a_long_backlog_goes_out_all_and_at_a_pace_salon_accepts() {
        let (mut session, at) = drawing();
        session.disconnected();
        let mut x = at.x;
        for step in 0..600 {
            x += 2.0;
            let time = f64::from(step) * 0.016;
            session
                .remote_mut()
                .map(|r| r.touch_move(1, Point::new(x, at.y), time));
            session.tick(time);
        }
        session.remote_mut().map(|r| r.touch_up(1));
        session.tick(10.0);
        session.connected();
        session.drain();
        show(&mut session, presets::draw());

        let (mut steps, mut most, mut now) = (Vec::new(), 0, 10.0);
        while now < 20.0 {
            now += 1.0 / 60.0;
            session.tick(now);
            let sent: Vec<_> = sent(&mut session)
                .into_iter()
                .filter(|message| matches!(message, PhoneMessage::Draw(_)))
                .collect();
            most = most.max(sent.len());
            steps.extend(sent);
        }
        assert!(most <= 4, "{most} steps in one frame: 240 a second at most");
        assert!(steps.len() > 500);
        assert!(matches!(
            steps.last(),
            Some(PhoneMessage::Draw(DrawEvent::End { .. }))
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
