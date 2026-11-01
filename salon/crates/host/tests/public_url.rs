//! `SALON_PUBLIC_URL`, read from the real environment: only a child process
//! can have another one.

#![expect(
    clippy::expect_used,
    reason = "test: join_url_for is a helper, outside the #[test] functions Clippy recognizes"
)]

use std::process::{Command, Output};

const CHILD: &str = "SALON_TEST_PRINT_JOIN_URL";

/// What [`join_url_for`] runs in a child process.
#[test]
#[ignore = "only a printer: join_url_for runs it in a child process"]
fn print_the_join_url() {
    if std::env::var_os(CHILD).is_some() {
        println!("url={}", salon_host::join_url(8090, "/play"));
    }
}

/// The address a game gets with `public` in its environment, and its log.
fn join_url_for(public: Option<&str>) -> (String, String) {
    let mut child = Command::new(std::env::current_exe().expect("the path of this test binary"));
    child
        .args(["--exact", "print_the_join_url", "--ignored", "--nocapture"])
        .env(CHILD, "1");
    match public {
        Some(public) => child.env(salon_host::PUBLIC_URL, public),
        None => child.env_remove(salon_host::PUBLIC_URL),
    };
    let Output {
        status,
        stdout,
        stderr,
    } = child.output().expect("the child runs");
    assert!(status.success());
    let stdout = String::from_utf8_lossy(&stdout);
    let url = stdout
        .split("url=")
        .nth(1)
        .and_then(|rest| rest.lines().next())
        .expect("the child prints the address");
    (
        url.to_owned(),
        String::from_utf8_lossy(&stderr).into_owned(),
    )
}

#[test]
fn the_public_url_replaces_the_network_address() {
    let (url, log) = join_url_for(Some(" https://party.example/ "));
    assert_eq!(url, "https://party.example/play");
    assert_eq!(log, "");
    let (url, _) = join_url_for(Some("http://192.168.1.5:8090"));
    assert_eq!(url, "http://192.168.1.5:8090/play");
}

#[test]
fn without_a_public_url_phones_get_the_network_address() {
    let expected = salon_host::lan_address().map_or_else(
        || "http://localhost:8090/play".to_owned(),
        |ip| format!("http://{ip}:8090/play"),
    );
    assert_eq!(join_url_for(None).0, expected);
    assert_eq!(join_url_for(Some("  ")).0, expected);
}

#[test]
fn a_public_url_that_is_no_address_is_refused_out_loud() {
    let (url, log) = join_url_for(Some("party.example"));
    assert!(!url.contains("party.example"), "{url}");
    assert!(log.contains(salon_host::PUBLIC_URL), "{log}");
}
