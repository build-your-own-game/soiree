//! The host's deterministic loop, used unchanged by the real room and tests.
//! The caller owns time and transports; the driver only sees ordered inputs.

use crate::Game;
use crate::apps::Apps;
use remote_protocol::web::PlayerId;
use remote_protocol::{DrawEvent, InputState};
use salon_core::{Caps, Host, HostConfig, LinkId, Ms, Output};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    Diagnostic {
        report: salon_core::journal::Diagnostic,
    },
    Opened {
        link: LinkId,
        caps: Caps,
    },
    Received {
        link: LinkId,
        bytes: Vec<u8>,
    },
    /// A link ended, whether it carried a device or an iPhone app.
    Closed {
        link: LinkId,
    },
    /// An iPhone app of the right version said who it is, on `link`.
    AppHello {
        link: LinkId,
        player: PlayerId,
        name: String,
    },
    AppInput {
        link: LinkId,
        state: InputState,
    },
    AppDraw {
        link: LinkId,
        event: DrawEvent,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Transmit { link: LinkId, bytes: Vec<u8> },
    Close { link: LinkId },
}

pub struct Driver<G: Game> {
    host: Host<G::Up, G::Down>,
    game: G,
    tick: u64,
    next_tick: Ms,
    now: Ms,
    last_tick: Option<Ms>,
    apps: Apps,
}

impl<G: Game> Driver<G> {
    pub fn new(now: Ms, config: HostConfig, tick_ms: u64, mut game: G) -> Self {
        let apps = Apps::new(config.timing, config.limits);
        let mut host = Host::new(now, config);
        game.start(now, &mut host);
        Self {
            host,
            game,
            tick: tick_ms.max(1),
            next_tick: now,
            now,
            last_tick: None,
            apps,
        }
    }

    /// The page an iPhone app shows unless [`Game::remote`] chooses.
    #[must_use]
    pub fn with_app_page(mut self, page: String) -> Self {
        self.apps.set_page(page);
        self
    }

    pub fn now(&self) -> Ms {
        self.now
    }

    pub fn game(&self) -> &G {
        &self.game
    }
    pub fn game_mut(&mut self) -> &mut G {
        &mut self.game
    }
    pub fn host(&self) -> &Host<G::Up, G::Down> {
        &self.host
    }

    pub fn next_deadline(&self) -> Ms {
        [
            self.host.next_deadline(),
            self.game.next_deadline(),
            Some(self.next_tick),
        ]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(self.next_tick)
    }

    /// A backwards clock is rejected without touching state. Network inputs
    /// are processed before due ticks, and device events after ticks, exactly
    /// as in the room loop. Preserve this ordering when changing this code.
    pub fn step(&mut self, now: Ms, inputs: impl IntoIterator<Item = Input>) -> Vec<Effect> {
        if now < self.now {
            return Vec::new();
        }
        self.now = now;
        let mut effects = Vec::new();
        let mut from_apps = Vec::new();
        for input in inputs {
            match input {
                Input::Diagnostic { report } => self.game.diagnostic(now, &report),
                Input::Opened { link, caps } => self.host.opened(now, link, caps),
                Input::Received { link, bytes } => self.host.received(now, link, &bytes),
                Input::Closed { link } => {
                    self.host.closed(now, link);
                    self.apps.closed(link, &mut from_apps);
                }
                Input::AppHello { link, player, name } => {
                    let (events, out) = (&mut from_apps, &mut effects);
                    self.apps.hello(now, link, player, &name, events, out);
                }
                Input::AppInput { link, state } => {
                    self.apps.input(now, link, state, &mut from_apps);
                }
                Input::AppDraw { link, event } => {
                    self.apps.draw(now, link, event, &mut from_apps);
                }
            }
        }
        if self.last_tick != Some(now)
            && (now >= self.next_tick || self.game.next_deadline().is_some_and(|due| due <= now))
        {
            self.game.tick(now, &mut self.host);
            self.last_tick = Some(now);
            if now >= self.next_tick {
                // Keep the periodic grid, skipping missed slots. Replaying
                // every missed tick at the same timestamp would duplicate
                // simulation work and create an immediate busy loop.
                let slots = now
                    .since(self.next_tick)
                    .checked_div(self.tick)
                    .unwrap_or(0)
                    .saturating_add(1);
                self.next_tick = self.next_tick.plus(self.tick.saturating_mul(slots));
            }
        }
        self.apps.expire(now, &mut from_apps, &mut effects);
        for event in from_apps {
            self.game.on_app(now, event, &mut self.host);
        }
        while let Some(output) = self.host.poll(now) {
            match output {
                Output::Transmit { link, bytes } => effects.push(Effect::Transmit { link, bytes }),
                Output::Close { link } => effects.push(Effect::Close { link }),
                Output::Device(event) => self.game.on(now, event, &mut self.host),
            }
        }
        // After the game has had its say on everything of this step.
        let game = &self.game;
        self.apps
            .show(now, |player| game.remote(player), &mut effects);
        self.apps.feedback(self.game.feedback(), &mut effects);
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppEvent;
    use remote_protocol::{Feedback, GameMessage, Haptic, Remote, StickValue, decode, presets};
    use salon_core::{DeviceEvent, Epoch, Limits, Timing, TvKey};

    fn config() -> HostConfig {
        HostConfig {
            game: 1,
            epoch: Epoch(1),
            tv_key: TvKey([0; 16]),
            timing: Timing::default(),
            limits: Limits {
                max_devices: 2,
                ..Limits::default()
            },
            known: Vec::new(),
        }
    }

    const PAGE: &str = "/play";
    const ADA: PlayerId = PlayerId::new([1; 16]);
    const BOB: PlayerId = PlayerId::new([2; 16]);
    const EVE: PlayerId = PlayerId::new([3; 16]);

    /// A game that shows `pad`, and keeps all it hears from the apps.
    #[derive(Default)]
    struct Remotes {
        pad: bool,
        heard: Vec<AppEvent>,
        feedback: Vec<(PlayerId, Feedback)>,
    }

    impl Game for Remotes {
        type Up = ();
        type Down = ();
        fn on(&mut self, _: Ms, _: DeviceEvent<()>, _: &mut Host<(), ()>) {}
        fn remote(&self, _: PlayerId) -> Option<Remote> {
            self.pad.then(|| Remote::Pad(presets::pad()))
        }
        fn on_app(&mut self, _: Ms, event: AppEvent, _: &mut Host<(), ()>) {
            self.heard.push(event);
        }
        fn feedback(&mut self) -> Vec<(PlayerId, Feedback)> {
            std::mem::take(&mut self.feedback)
        }
    }

    fn on_the_page() -> Driver<Remotes> {
        Driver::new(Ms(0), config(), 10, Remotes::default()).with_app_page(PAGE.to_owned())
    }

    fn on_a_pad() -> Driver<Remotes> {
        let mut driver = on_the_page();
        driver.game_mut().pad = true;
        driver
    }

    fn hello(link: u64, player: PlayerId) -> Input {
        Input::AppHello {
            link: LinkId(link),
            player,
            name: "Ada".to_owned(),
        }
    }

    fn input(link: u64) -> Input {
        Input::AppInput {
            link: LinkId(link),
            state: InputState::default(),
        }
    }

    /// What `link` is told in these effects.
    fn told(effects: &[Effect], link: u64) -> Vec<GameMessage> {
        effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::Transmit { link: to, bytes } if *to == LinkId(link) => {
                    Some(decode(bytes).unwrap())
                }
                Effect::Transmit { .. } | Effect::Close { .. } => None,
            })
            .collect()
    }

    fn closes(effects: &[Effect], link: u64) -> bool {
        effects.contains(&Effect::Close { link: LinkId(link) })
    }

    #[test]
    fn a_remote_is_sent_when_it_changes_and_only_then() {
        let pad = GameMessage::Show(Remote::Pad(presets::pad()));
        let page = GameMessage::Show(Remote::Web {
            url: PAGE.to_owned(),
        });
        let mut driver = on_a_pad();
        assert_eq!(
            told(&driver.step(Ms(1), [hello(1, ADA)]), 1),
            vec![pad.clone()]
        );
        assert_eq!(told(&driver.step(Ms(2), []), 1), []);
        driver.game_mut().pad = false;
        assert_eq!(told(&driver.step(Ms(3), []), 1), [page]);
        assert_eq!(told(&driver.step(Ms(4), []), 1), []);
        driver.game_mut().pad = true;
        assert_eq!(told(&driver.step(Ms(5), []), 1), [pad]);
    }

    /// A phone that vanishes says nothing: a pad's heartbeat is what tells.
    #[test]
    fn a_silent_pad_has_left() {
        let absent = Timing::default().absent;
        let mut driver = on_a_pad();
        let _: Vec<Effect> = driver.step(Ms(1), [hello(1, ADA)]);
        let _: Vec<Effect> = driver.step(Ms(absent), [input(1)]);
        let effects = driver.step(Ms(absent + absent - 1), []);
        assert!(!closes(&effects, 1));
        assert!(closes(&driver.step(Ms(absent + absent), []), 1));
        assert_eq!(
            driver.game().heard.last(),
            Some(&AppEvent::Left { player: ADA })
        );
        // Its socket closes afterwards: this is not a second departure.
        let lefts = driver.game().heard.len();
        let _: Vec<Effect> = driver.step(Ms(2 * absent + 1), [Input::Closed { link: LinkId(1) }]);
        assert_eq!(driver.game().heard.len(), lefts);
    }

    /// The app's page may stay quiet: it sends nothing here.
    #[test]
    fn a_quiet_page_stays() {
        let mut driver = on_the_page();
        let _: Vec<Effect> = driver.step(Ms(1), [hello(1, ADA)]);
        assert!(!closes(&driver.step(Ms(600_000), []), 1));
        assert_eq!(driver.game().heard.len(), 1);
    }

    /// Going from the page to a pad gives the app time to answer.
    #[test]
    fn a_pad_is_not_silent_before_it_is_shown() {
        let mut driver = on_the_page();
        let _: Vec<Effect> = driver.step(Ms(1), [hello(1, ADA)]);
        driver.game_mut().pad = true;
        assert!(!closes(&driver.step(Ms(600_000), []), 1));
        assert!(!closes(&driver.step(Ms(600_001), []), 1));
    }

    #[test]
    fn the_table_does_not_grow_with_what_strangers_claim_to_be() {
        let mut driver = on_a_pad();
        let _: Vec<Effect> = driver.step(Ms(1), [hello(1, ADA), hello(2, BOB)]);
        let effects = driver.step(Ms(2), [hello(3, EVE)]);
        assert!(matches!(
            told(&effects, 3).as_slice(),
            [GameMessage::Refuse { .. }]
        ));
        assert!(closes(&effects, 3));
        // A player who is away gives up the place, and the game still knows
        // her: `Arrived` is for who this run has not seen.
        let _: Vec<Effect> = driver.step(Ms(3), [Input::Closed { link: LinkId(1) }]);
        let effects = driver.step(Ms(4), [hello(4, EVE)]);
        assert!(!closes(&effects, 4));
        let _: Vec<Effect> = driver.step(Ms(5), [Input::Closed { link: LinkId(2) }]);
        let _: Vec<Effect> = driver.step(Ms(6), [hello(5, ADA)]);
        assert_eq!(
            driver.game().heard.last(),
            Some(&AppEvent::Back { player: ADA })
        );
    }

    /// How Ada comes back after `strangers` connections took places in turn.
    /// The table holds two, and Bob keeps one.
    fn back_after(strangers: u32) -> Option<AppEvent> {
        let stranger = |number: u32| {
            let mut id = [0xee; 16];
            id[..4].copy_from_slice(&number.to_le_bytes());
            PlayerId::new(id)
        };
        let mut driver = on_a_pad();
        let _: Vec<Effect> = driver.step(Ms(1), [hello(1, ADA), hello(2, BOB)]);
        let _: Vec<Effect> = driver.step(Ms(2), [Input::Closed { link: LinkId(1) }]);
        for number in 0..strangers {
            let link = u64::from(number) + 3;
            let _: Vec<Effect> = driver.step(Ms(link), [hello(link, stranger(number))]);
            let _: Vec<Effect> = driver.step(Ms(link), [Input::Closed { link: LinkId(link) }]);
        }
        let link = u64::from(strangers) + 3;
        let _: Vec<Effect> = driver.step(Ms(link), [hello(link, ADA)]);
        driver.game().heard.last().cloned()
    }

    /// Whoever claims a place gets one, so the room cannot remember for ever
    /// those who gave theirs up: the oldest falls off, at this exact bound.
    #[test]
    fn the_players_who_lost_their_place_are_remembered_up_to_a_bound() {
        let kept = u32::try_from(crate::apps::EVICTED_KEPT).unwrap();
        assert_eq!(back_after(kept), Some(AppEvent::Back { player: ADA }));
        assert!(matches!(
            back_after(kept + 1),
            Some(AppEvent::Arrived { player, .. }) if player == ADA
        ));
    }

    #[test]
    fn a_flood_of_frames_is_dropped_unread() {
        let burst = usize::try_from(Limits::default().packets_burst).unwrap();
        let mut driver = on_a_pad();
        let _: Vec<Effect> = driver.step(Ms(1), [hello(1, ADA)]);
        let _: Vec<Effect> = driver.step(Ms(1), std::iter::repeat_n(input(1), burst * 2));
        let heard = &driver.game().heard;
        // The hello is the first frame of the burst.
        assert_eq!(heard.len(), burst);
    }

    /// A connection is one player, the first it names: what it claims next
    /// is not read. A claim used to be a whole new player, with a budget of
    /// its own.
    #[test]
    fn a_connection_keeps_the_player_it_first_named() {
        let mut driver = on_a_pad();
        let _: Vec<Effect> = driver.step(Ms(1), [hello(1, ADA), hello(1, BOB)]);
        let heard = &driver.game().heard;
        assert!(matches!(
            heard.as_slice(),
            [AppEvent::Arrived { player, .. }] if *player == ADA
        ));
        // Bob is nobody here: Ada still sends, and Bob can join on his own.
        let _: Vec<Effect> = driver.step(Ms(2), [input(1), hello(2, BOB)]);
        let heard = &driver.game().heard;
        assert!(matches!(heard[1], AppEvent::Input { player, .. } if player == ADA));
        assert!(matches!(heard[2], AppEvent::Arrived { player, .. } if player == BOB));
    }

    /// E3 at the app's door: the budget is the connection's. It was the
    /// player's, and read after the player's place was taken, so a new name
    /// on each frame got through all of it: 100 000 frames, 199 999 events.
    #[test]
    fn a_flood_of_new_players_on_one_connection_stays_within_its_budget() {
        let limits = Limits::default();
        let stranger = |frame: u32| {
            let mut id = [0xff; 16];
            id[..4].copy_from_slice(&frame.to_le_bytes());
            PlayerId::new(id)
        };
        let config = HostConfig { limits, ..config() };
        let mut driver =
            Driver::new(Ms(0), config, 10, Remotes::default()).with_app_page(PAGE.to_owned());
        driver.game_mut().pad = true;
        let _: Vec<Effect> = driver.step(Ms(1), [hello(1, ADA)]);
        let _: Vec<Effect> = driver.step(Ms(2), [Input::Closed { link: LinkId(1) }]);
        let flood = (0..100_000).map(|frame| hello(2, stranger(frame)));
        let _: Vec<Effect> = driver.step(Ms(3), flood);
        // The first name of the connection, and nothing else.
        assert_eq!(driver.game().heard.len(), 3);
        // Ada lost no place.
        let _: Vec<Effect> = driver.step(Ms(4), [hello(3, ADA)]);
        assert_eq!(
            driver.game().heard.last(),
            Some(&AppEvent::Back { player: ADA })
        );
    }

    /// Frames past the budget of a connection are not read, whatever they
    /// are, and hellos count as much as inputs.
    #[test]
    fn hellos_spend_the_budget_of_their_connection() {
        let burst = usize::try_from(Limits::default().packets_burst).unwrap();
        let mut driver = on_a_pad();
        let flood = vec![hello(1, ADA); burst];
        let _: Vec<Effect> = driver.step(Ms(1), flood);
        let _: Vec<Effect> = driver.step(Ms(1), [input(1)]);
        assert_eq!(driver.game().heard.len(), 1);
        // A second of that budget comes back with time.
        let _: Vec<Effect> = driver.step(Ms(1_001), [input(1)]);
        assert!(matches!(
            driver.game().heard.last(),
            Some(AppEvent::Input { .. })
        ));
    }

    /// A second connection of a player takes her place in silence: the game
    /// hears nothing, the first one closes, and the numbers start again.
    #[test]
    fn a_second_connection_takes_the_place_without_a_word() {
        let state = |seq| InputState {
            seq,
            ..InputState::default()
        };
        let frame = |link: u64, seq| Input::AppInput {
            link: LinkId(link),
            state: state(seq),
        };
        let mut driver = on_a_pad();
        let _: Vec<Effect> = driver.step(Ms(1), [hello(1, ADA)]);
        let _: Vec<Effect> = driver.step(Ms(2), [frame(1, 9)]);
        let effects = driver.step(Ms(3), [hello(2, ADA)]);
        assert!(closes(&effects, 1));
        assert!(!closes(&effects, 2));
        assert_eq!(
            told(&effects, 2),
            [GameMessage::Show(Remote::Pad(presets::pad()))]
        );
        assert_eq!(driver.game().heard.len(), 2, "an arrival and an input");
        // The old connection is not heard any more, not even as a goodbye.
        let _: Vec<Effect> = driver.step(Ms(4), [frame(1, 10)]);
        assert_eq!(driver.game().heard.len(), 2);
        let _: Vec<Effect> = driver.step(Ms(5), [Input::Closed { link: LinkId(1) }]);
        assert_eq!(driver.game().heard.len(), 2);
        let _: Vec<Effect> = driver.step(Ms(6), [frame(2, 1)]);
        assert_eq!(
            driver.game().heard.last(),
            Some(&AppEvent::Input {
                player: ADA,
                state: state(1)
            })
        );
        // Saying hello again on the same connection closes nothing.
        assert!(!closes(&driver.step(Ms(7), [hello(2, ADA)]), 2));
    }

    /// `Game::on_app` says only a `Remote::Pad` sends these. An app on the
    /// phone page has no control: what it claims is not the game's to read.
    #[test]
    fn nothing_but_a_pad_sends_an_input_or_a_stroke() {
        let stroke = |link: u64| Input::AppDraw {
            link: LinkId(link),
            event: DrawEvent::Clear,
        };
        let mut driver = on_the_page();
        let _: Vec<Effect> = driver.step(Ms(1), [hello(1, ADA)]);
        let _: Vec<Effect> = driver.step(Ms(2), [input(1), stroke(1)]);
        assert_eq!(driver.game().heard.len(), 1, "only the arrival");
        driver.game_mut().pad = true;
        let _: Vec<Effect> = driver.step(Ms(3), []);
        let _: Vec<Effect> = driver.step(Ms(4), [input(1), stroke(1)]);
        assert_eq!(driver.game().heard.len(), 3);
        driver.game_mut().pad = false;
        let _: Vec<Effect> = driver.step(Ms(5), []);
        let _: Vec<Effect> = driver.step(Ms(6), [input(1), stroke(1)]);
        assert_eq!(driver.game().heard.len(), 3);
    }

    /// -127 to 127, says the contract: -128 fits the byte and would read as
    /// -1.008, past what `tilt()` and `stick()` promise a game.
    #[test]
    fn an_input_reaches_the_game_within_its_range() {
        let wild = InputState {
            seq: 1,
            sticks: vec![StickValue {
                id: 1,
                x: i8::MIN,
                y: i8::MIN,
            }],
            tilt: i8::MIN,
            ..InputState::default()
        };
        let mut driver = on_a_pad();
        let _: Vec<Effect> = driver.step(Ms(1), [hello(1, ADA)]);
        let frame = Input::AppInput {
            link: LinkId(1),
            state: wild,
        };
        let _: Vec<Effect> = driver.step(Ms(2), [frame]);
        let Some(AppEvent::Input { state, .. }) = driver.game().heard.last() else {
            panic!("an input");
        };
        assert!(state.tilt() >= -1.0, "{}", state.tilt());
        assert_eq!(state.stick(1), (-1.0, -1.0));
    }

    #[test]
    fn a_frame_from_an_unknown_link_is_nobodys() {
        let mut driver = on_a_pad();
        let _: Vec<Effect> = driver.step(Ms(1), [input(9)]);
        assert_eq!(driver.game().heard, []);
    }

    #[test]
    fn a_vibration_goes_to_its_player_and_is_lost_when_away() {
        let buzz = Feedback {
            haptic: Some(Haptic::Press),
            color: None,
        };
        let mut driver = on_a_pad();
        let _: Vec<Effect> = driver.step(Ms(1), [hello(1, ADA), hello(2, BOB)]);
        driver.game_mut().feedback = vec![(BOB, buzz)];
        let effects = driver.step(Ms(2), []);
        assert_eq!(told(&effects, 1), []);
        assert_eq!(told(&effects, 2), [GameMessage::Feedback(buzz)]);
        driver.game_mut().feedback = vec![(BOB, buzz), (EVE, buzz)];
        let effects = driver.step(Ms(3), [Input::Closed { link: LinkId(2) }]);
        assert_eq!(effects, []);
    }

    #[derive(Default)]
    struct Ticks(Vec<Ms>);
    impl Game for Ticks {
        type Up = ();
        type Down = ();
        fn on(&mut self, _: Ms, _: DeviceEvent<()>, _: &mut Host<(), ()>) {}
        fn tick(&mut self, now: Ms, _: &mut Host<(), ()>) {
            self.0.push(now);
        }
    }
    #[test]
    fn late_wakes_skip_missed_periodic_slots_without_double_ticking() {
        let mut driver = Driver::new(Ms(0), config(), 17, Ticks::default());
        let _: Vec<Effect> = driver.step(Ms(0), []);
        let _: Vec<Effect> = driver.step(Ms(1000), []);
        assert_eq!(driver.next_deadline(), Ms(1003));
        let _: Vec<Effect> = driver.step(Ms(1000), []);
        let _: Vec<Effect> = driver.step(Ms(999), []);
        assert_eq!(driver.game().0, vec![Ms(0), Ms(1000)]);
        let _: Vec<Effect> = driver.step(Ms(1003), []);
        assert_eq!(driver.game().0, vec![Ms(0), Ms(1000), Ms(1003)]);
    }
}
