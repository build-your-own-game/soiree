//! What tells a page's [`Client`] to look again: the browser's signals, named
//! once, apart from the listeners so that a test can run them without a browser.
//! `salon/tests/ui-browser.mjs` fires `resume` in WebKit and Chromium.

use salon_core::{Client, Ms};
use serde::Serialize;
use serde::de::DeserializeOwned;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Window,
    Document,
}

/// What a signal says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum News {
    /// The page runs again.
    PageBack,
    /// The browser says the network is back.
    NetworkBack,
}

impl News {
    pub(crate) fn check<Up: Serialize, Down: DeserializeOwned>(
        self,
    ) -> fn(&mut Client<Up, Down>, Ms) {
        match self {
            News::PageBack => Client::wake,
            News::NetworkBack => Client::network_back,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Signal {
    pub target: Target,
    pub event: &'static str,
    pub news: News,
}

pub(crate) const SIGNALS: [Signal; 4] = [
    Signal {
        target: Target::Window,
        event: "online",
        news: News::NetworkBack,
    },
    Signal {
        target: Target::Window,
        event: "pageshow",
        news: News::PageBack,
    },
    Signal {
        target: Target::Document,
        event: "visibilitychange",
        news: News::PageBack,
    },
    // A page that thaws: `visibilitychange` no longer always follows
    // (Phoenix, Chrome 149).
    Signal {
        target: Target::Document,
        event: "resume",
        news: News::PageBack,
    },
];

#[cfg(test)]
mod tests {
    use salon_core::{Caps, ClientConfig, ClientOutput, Kind, Limits, Timing, Token};

    use super::*;

    type Page = Client<u8, u8>;

    /// The signal a page gets on this target under this name, if it listens.
    fn listens(target: Target, event: &str) -> Option<News> {
        SIGNALS
            .iter()
            .find(|signal| signal.target == target && signal.event == event)
            .map(|signal| signal.news)
    }

    /// A page with an open link it stopped hearing at `Ms(0)`.
    fn quiet_page() -> Page {
        let config = ClientConfig {
            game: 1,
            kind: Kind::Phone,
            token: Token([1; 16]),
            name: None,
            tv_key: None,
            seed: 1,
            timing: Timing::default(),
            limits: Limits::default(),
        };
        let mut page = Page::new(Ms(0), config);
        while let Some(output) = page.poll(Ms(0)) {
            if output == ClientOutput::Connect {
                page.opened(Ms(0), Caps::STREAM);
            }
        }
        page
    }

    /// Reconnections the page asks for at `now`.
    fn connects(page: &mut Page, now: Ms) -> usize {
        std::iter::from_fn(|| page.poll(now))
            .filter(|output| *output == ClientOutput::Connect)
            .count()
    }

    /// reconnexion-23: on a page that thaws, Chrome no longer reliably sends
    /// `visibilitychange`. `resume` alone must then do what it would have
    /// done, on `document`, the only place the browser sends it.
    #[test]
    fn a_page_thawing_without_visibilitychange_checks_its_link_at_once() {
        let thawed = Ms(Timing::default().silent);
        let mut untold = quiet_page();
        assert_eq!(connects(&mut untold, thawed), 0, "nothing without a signal");

        let news = listens(Target::Document, "resume").unwrap();
        let mut told = quiet_page();
        news.check()(&mut told, thawed);
        assert_eq!(connects(&mut told, thawed), 1);
        assert_eq!(listens(Target::Window, "resume"), None);
    }
}
