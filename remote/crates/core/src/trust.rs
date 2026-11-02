//! Who may reach the app from inside a web view: a trust boundary, since the
//! page comes from the network, and through a QR code anyone can print.
//!
//! The app's bridge sends vibrations, sensors and UDP, and `window.telecommande`
//! holds the player's identity, the same 16 bytes in every game. They belong
//! to the page the game asked for, and to no one it loads or links to.
//!
//! - [`may_open`]: the top of the web view stays on that page's origin.
//! - [`may_call`]: only the top frame, on that origin, reaches the bridge.
//!
//! An origin is a scheme, a host and a port: all three must match, spelled the
//! same but for letter case and a default port. `127.0.0.1` and `localhost`
//! are two origins, and so are two ports of one host. Only `http` and `https`
//! have one here: `about:`, `data:`, `blob:` and `file:` are refused, and so is
//! whatever cannot be read for certain, like a `user@` in the address or a
//! backslash, which browsers and this parser could read differently.

/// The frame a call or a request comes from, as WebKit gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame<'a> {
    /// The top of the web view, not an iframe.
    pub main: bool,
    pub scheme: &'a str,
    pub host: &'a str,
    /// 0 for the scheme's default, as `WKSecurityOrigin` says it.
    pub port: u16,
}

#[derive(Debug, PartialEq, Eq)]
struct Origin {
    scheme: String,
    host: String,
    port: u16,
}

fn origin(scheme: &str, host: &str, port: u16) -> Option<Origin> {
    let scheme = scheme.to_ascii_lowercase();
    let standard = match scheme.as_str() {
        "http" => 80,
        "https" => 443,
        _ => return None,
    };
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    let readable = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':');
    if host.is_empty() || !host.chars().all(readable) {
        return None;
    }
    Some(Origin {
        scheme,
        host: host.to_ascii_lowercase(),
        port: if port == 0 { standard } else { port },
    })
}

/// The origin of an absolute address, if it has one beyond doubt.
fn origin_of(address: &str) -> Option<Origin> {
    let (scheme, rest) = address.split_once("://")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = rest.get(..end)?;
    let (host, port) = match authority.strip_prefix('[') {
        Some(inner) => {
            let (host, after) = inner.split_once(']')?;
            (
                host,
                after
                    .strip_prefix(':')
                    .or(Some(after).filter(|a| a.is_empty()))?,
            )
        }
        None => authority.split_once(':').unwrap_or((authority, "")),
    };
    let port = if port.is_empty() {
        0
    } else {
        port.parse().ok()?
    };
    origin(scheme, host, port)
}

/// Whether the top of the web view may go to `to`, when the game asked for
/// `page`. An unreadable address goes nowhere.
pub fn may_open(page: &str, to: &str) -> bool {
    origin_of(page).is_some_and(|page| origin_of(to).is_some_and(|to| to == page))
}

/// Whether `frame` may reach the bridge, when the game asked for `page`.
pub fn may_call(page: &str, frame: &Frame<'_>) -> bool {
    frame.main
        && origin_of(page).is_some_and(|page| {
            origin(frame.scheme, frame.host, frame.port).is_some_and(|frame| frame == page)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = "http://192.168.1.12:8090/play?room=AB12";

    #[test]
    fn the_web_view_stays_on_the_origin_of_the_page_the_game_asked_for() {
        for to in [
            "http://192.168.1.12:8090/play",
            "http://192.168.1.12:8090/elsewhere?x=1#top",
            "http://192.168.1.12:8090",
            "HTTP://192.168.1.12:8090/",
        ] {
            assert!(may_open(PAGE, to), "{to}");
        }
        assert!(may_open(
            "http://Mac.local/play",
            "http://mac.local:80/next"
        ));
        assert!(may_open(
            "https://game.example/",
            "https://game.example:443/x"
        ));
        assert!(may_open("http://[::1]:8090/play", "http://[::1]:8090/x"));

        for to in [
            "http://192.168.1.12:8091/play",
            "http://192.168.1.12/play",
            "https://192.168.1.12:8090/play",
            "http://192.168.1.13:8090/play",
            "http://localhost:8090/play",
            "http://192.168.1.12.example:8090/",
            "http://192.168.1.12:8090@evil.example/",
            "http://user@192.168.1.12:8090/",
            "http://evil.example\\@192.168.1.12:8090/",
            "http://192.168.1.12%2eexample:8090/",
            "http://192.168.1.12:port/",
            "http://192.168.1.12:99999/",
            "//192.168.1.12:8090/play",
            "/play",
            "about:blank",
            "data:text/html,hello",
            "blob:http://192.168.1.12:8090/1234",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "telecommande://join?url=http%3A%2F%2F192.168.1.12",
            "",
        ] {
            assert!(!may_open(PAGE, to), "{to}");
        }
    }

    #[test]
    fn a_page_with_no_origin_opens_nothing() {
        for page in ["", "about:blank", "ftp://host/", "http://", "/play"] {
            assert!(!may_open(page, page), "{page}");
            assert!(!may_open(page, PAGE), "{page}");
        }
    }

    #[test]
    fn only_the_top_frame_on_the_pages_origin_reaches_the_bridge() {
        let frame = |main, scheme, host, port| Frame {
            main,
            scheme,
            host,
            port,
        };
        assert!(may_call(PAGE, &frame(true, "http", "192.168.1.12", 8090)));
        assert!(may_call(
            "http://mac.local/play",
            &frame(true, "http", "Mac.Local", 0)
        ));
        assert!(may_call(
            "https://game.example/",
            &frame(true, "https", "game.example", 0)
        ));
        assert!(may_call(
            "http://[::1]:8090/",
            &frame(true, "http", "[::1]", 8090)
        ));

        for refused in [
            frame(false, "http", "192.168.1.12", 8090),
            frame(true, "http", "192.168.1.12", 8091),
            frame(true, "https", "192.168.1.12", 8090),
            frame(true, "http", "localhost", 8090),
            frame(true, "http", "192.168.1.12", 0),
            frame(true, "about", "", 0),
            frame(true, "", "", 0),
            frame(true, "file", "192.168.1.12", 8090),
        ] {
            assert!(!may_call(PAGE, &refused), "{refused:?}");
        }
    }
}
