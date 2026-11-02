//! Where phones reach this computer. The operating system's default route is
//! a good guess, not a fact: without a network there is none, and behind a
//! full VPN it leads to an address no phone reaches. `SALON_PUBLIC_URL` then
//! says it, for every game.

use std::net::{IpAddr, UdpSocket};

/// Replaces the base of the address phones open ([`join_url`]).
pub const PUBLIC_URL: &str = "SALON_PUBLIC_URL";

/// A base phones can open: scheme, host, optional port and path, nothing else.
#[derive(Debug, PartialEq, Eq)]
struct PublicUrl<'a> {
    /// Without a final `/`, to be followed by a path.
    base: &'a str,
    host: &'a str,
}

impl<'a> PublicUrl<'a> {
    /// `None`: blank, as good as unset. A value that a phone could not open
    /// is an error rather than a QR code that leads nowhere.
    fn parse(text: &'a str) -> Result<Option<PublicUrl<'a>>, &'static str> {
        let base = text.trim().trim_end_matches('/');
        if base.is_empty() {
            return Ok(None);
        }
        let rest = base
            .strip_prefix("http://")
            .or_else(|| base.strip_prefix("https://"))
            .ok_or("it must start with http:// or https://")?;
        if base.contains(|c: char| c.is_whitespace() || c.is_control() || "?#@".contains(c)) {
            return Err("it is a base: scheme, host, port and path, nothing else");
        }
        let authority = rest.split('/').next().unwrap_or_default();
        let (host, port) = match authority.strip_prefix('[') {
            Some(ipv6) => {
                let (host, after) = ipv6.split_once(']').ok_or("its IPv6 address has no `]`")?;
                (
                    host,
                    after
                        .strip_prefix(':')
                        .or(Some(after))
                        .filter(|port| !port.is_empty()),
                )
            }
            None => match authority.split_once(':') {
                Some((host, port)) => (host, Some(port)),
                None => (authority, None),
            },
        };
        if host.is_empty() {
            return Err("it has no host");
        }
        if port.is_some_and(|port| port.parse::<u16>().is_err()) {
            return Err("its port is not a port");
        }
        Ok(Some(PublicUrl { base, host }))
    }
}

/// Runs `with` on `SALON_PUBLIC_URL`, if it is usable. If not, says so: the
/// game goes on with the network address.
fn with_public_url<T>(with: impl FnOnce(PublicUrl<'_>) -> Option<T>) -> Option<T> {
    let value = std::env::var(PUBLIC_URL).ok()?;
    match PublicUrl::parse(&value) {
        Ok(url) => url.and_then(with),
        Err(why) => {
            crate::say(format_args!(
                "salon: {PUBLIC_URL} ignored ({value:?}): {why}"
            ));
            None
        }
    }
}

/// The address other devices on the network reach this computer at.
/// Connecting a UDP socket sends nothing; it only asks the OS which
/// interface it would use.
pub fn lan_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    socket.connect(("192.0.2.1", 9)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

/// The address phones open: `http://<lan address>:<port><path>`, or
/// `SALON_PUBLIC_URL` followed by `path` when it is set.
pub fn join_url(port: u16, path: &str) -> String {
    if let Some(base) = with_public_url(|url| Some(url.base.to_owned())) {
        return format!("{base}{path}");
    }
    let host = lan_address().map_or_else(
        || {
            crate::say(format_args!("salon: no network address found: phones cannot reach http://localhost; set {PUBLIC_URL}"));
            "localhost".to_owned()
        },
        |ip| ip.to_string(),
    );
    format!("http://{host}:{port}{path}")
}

/// The address WebRTC announces to phones: the one `SALON_PUBLIC_URL` names,
/// when its host is an IP address, since a name cannot be announced.
pub(crate) fn rtc_address() -> Option<IpAddr> {
    with_public_url(|url| url.host.parse().ok()).or_else(lan_address)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(text: &str) -> Option<&str> {
        PublicUrl::parse(text).unwrap().map(|url| url.base)
    }

    #[test]
    fn a_base_keeps_what_phones_need() {
        assert_eq!(base("https://party.example"), Some("https://party.example"));
        assert_eq!(
            base(" https://party.example/ \n"),
            Some("https://party.example")
        );
        assert_eq!(base("http://10.0.0.2:8080"), Some("http://10.0.0.2:8080"));
        assert_eq!(
            base("https://example.org/soiree/"),
            Some("https://example.org/soiree")
        );
        assert_eq!(base("http://[fe80::1]:8090"), Some("http://[fe80::1]:8090"));
    }

    #[test]
    fn blank_is_unset() {
        assert_eq!(base(""), None);
        assert_eq!(base("  \t"), None);
        assert_eq!(base("/"), None);
    }

    #[test]
    fn what_a_phone_could_not_open_is_refused() {
        for text in [
            "party.example",
            "ftp://party.example",
            "http://",
            "http://:8090",
            "http://party example",
            "http://party.example:port",
            "http://party.example:99999",
            "http://party.example/?room=1",
            "http://party.example/#top",
            "http://me@party.example",
            "http://[fe80::1",
            "http://[fe80::1]:port",
        ] {
            assert!(PublicUrl::parse(text).is_err(), "{text}");
        }
    }

    /// WebRTC's candidate comes from this host: an IP address or nothing.
    #[test]
    fn the_host_of_a_base_is_found() {
        let host = |text| PublicUrl::parse(text).unwrap().unwrap().host;
        assert_eq!(host("http://192.168.1.5:8090"), "192.168.1.5");
        assert_eq!(host("https://party.example/soiree"), "party.example");
        assert_eq!(host("http://[fe80::1]:8090"), "fe80::1");
        assert_eq!(host("http://[fe80::1]"), "fe80::1");
    }
}
