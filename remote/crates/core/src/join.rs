//! From what the player scans to where the app connects.
//!
//! A game's QR code holds the address of its phone page,
//! `http://host:port/path?query`. The app connects to the remote's WebSocket
//! on the same host and port, `ws://host:port/remote?query`: the query (a
//! room code…) passes as is, so a game keeps its QR code. A page may also
//! link to `telecommande://join?url=<page address, percent-encoded>`, which
//! opens the app on the game.

use remote_protocol::PATH;

/// Where to connect, and the page it came from (the base of a relative
/// address the game sends).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub socket: String,
    pub page: String,
}

/// The game a scanned text or an opened link points to, if any.
pub fn target(text: &str) -> Option<Target> {
    let text = text.trim();
    if let Some(query) = text.strip_prefix("telecommande://join?") {
        let encoded = query
            .split('&')
            .find_map(|pair| pair.strip_prefix("url="))?;
        return page_target(&percent_decode(encoded)?);
    }
    page_target(text)
}

fn page_target(page: &str) -> Option<Target> {
    let (socket_scheme, rest) = match page.strip_prefix("http://") {
        Some(rest) => ("ws", rest),
        None => ("wss", page.strip_prefix("https://")?),
    };
    let rest = rest.split('#').next().unwrap_or_default();
    let authority_end = rest.find(['/', '?']).unwrap_or(rest.len());
    let (authority, after) = rest.split_at(authority_end);
    if authority.is_empty() || authority.contains(char::is_whitespace) {
        return None;
    }
    let query = after
        .split_once('?')
        .map(|(_, query)| query)
        .filter(|q| !q.is_empty());
    let socket = match query {
        Some(query) => format!("{socket_scheme}://{authority}{PATH}?{query}"),
        None => format!("{socket_scheme}://{authority}{PATH}"),
    };
    Some(Target {
        socket,
        page: page.split('#').next().unwrap_or(page).to_string(),
    })
}

/// `%XX` escapes back to bytes; `None` if one is broken or the result is not
/// UTF-8.
fn percent_decode(text: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut rest = text.as_bytes();
    while let Some((&byte, tail)) = rest.split_first() {
        if byte == b'%' {
            let (hex, after) = tail.split_at_checked(2)?;
            let hex = std::str::from_utf8(hex).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
            rest = after;
        } else {
            bytes.push(byte);
            rest = tail;
        }
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn socket(text: &str) -> Option<String> {
        target(text).map(|t| t.socket)
    }

    #[test]
    fn a_game_page_leads_to_its_remote_socket_with_the_same_query() {
        assert_eq!(
            socket("http://192.168.1.12:8090/play?room=AB12"),
            Some("ws://192.168.1.12:8090/remote?room=AB12".into())
        );
        assert_eq!(
            socket("http://mac.local:2567/play"),
            Some("ws://mac.local:2567/remote".into())
        );
        assert_eq!(
            socket("https://game.example/?seat=2#top"),
            Some("wss://game.example/remote?seat=2".into())
        );
        assert_eq!(
            socket("  http://10.0.0.2:8095  "),
            Some("ws://10.0.0.2:8095/remote".into())
        );
    }

    #[test]
    fn the_app_link_carries_the_page() {
        let link = "telecommande://join?url=http%3A%2F%2F192.168.1.5%3A8095%2Fplay%3Froom%3DAB";
        assert_eq!(
            target(link),
            Some(Target {
                socket: "ws://192.168.1.5:8095/remote?room=AB".into(),
                page: "http://192.168.1.5:8095/play?room=AB".into(),
            })
        );
    }

    #[test]
    fn anything_else_is_not_a_game() {
        for text in [
            "",
            "hello",
            "ftp://host/file",
            "http://",
            "http:///play",
            "telecommande://join?name=x",
            "telecommande://join?url=http%3",
            "telecommande://join?url=ftp%3A%2F%2Fhost",
        ] {
            assert_eq!(target(text), None, "{text}");
        }
    }
}
