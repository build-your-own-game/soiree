//! Le jeu se branche-t-il ? On le lance comme le soir même, avec
//! `cargo run --release` et le port que la console lui donne, et on regarde
//! s'il sert une page.

use std::io::{Read as _, Write as _};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::{Duration, Instant};

/// Le premier lancement compile tout, pages comprises.
const PATIENCE: Duration = Duration::from_mins(30);
const BETWEEN_TRIES: Duration = Duration::from_millis(500);
const ANSWER_WITHIN: Duration = Duration::from_secs(2);

pub fn run(root: &Path) -> Result<(), String> {
    let port = free_port().map_err(|error| format!("  aucun port libre : {error}"))?;
    println!(
        "\n── PORT={port} cargo run --release   (la première fois, la compilation prend quelques minutes)"
    );
    let mut game = Command::new("cargo")
        .current_dir(root)
        .args(["run", "--release"])
        .env("PORT", port.to_string())
        .spawn()
        .map_err(|error| format!("  impossible de lancer cargo : {error}"))?;
    let started = Instant::now();
    let outcome = loop {
        match game.try_wait() {
            Ok(Some(status)) => {
                break Err(format!(
                    "  `cargo run --release` s'est arrêté ({status}) avant de servir une page.\n  Le jeu doit se lancer par cette seule commande, depuis la racine du dépôt, et rester ouvert."
                ));
            }
            Ok(None) => {}
            Err(error) => break Err(format!("  le jeu ne répond plus : {error}")),
        }
        if serves_a_page(port) {
            break Ok(());
        }
        if started.elapsed() > PATIENCE {
            break Err(format!(
                "  le jeu tourne mais ne sert aucune page sur le port {port}.\n  Il doit écouter sur le port de la variable `PORT` et répondre à `/` par sa page."
            ));
        }
        sleep(BETWEEN_TRIES);
    };
    stop(&mut game);
    outcome
}

fn free_port() -> std::io::Result<u16> {
    Ok(TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?
        .local_addr()?
        .port())
}

/// `GET /` répond 200 avec une page HTML.
fn serves_a_page(port: u16) -> bool {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut stream) = TcpStream::connect_timeout(&address, ANSWER_WITHIN) else {
        return false;
    };
    if stream.set_read_timeout(Some(ANSWER_WITHIN)).is_err()
        || stream
            .write_all(b"GET / HTTP/1.0\r\nHost: localhost\r\nAccept: text/html\r\n\r\n")
            .is_err()
    {
        return false;
    }
    let mut answer = Vec::new();
    // Une lecture coupée par le délai garde ce qui est déjà arrivé.
    let _: std::io::Result<usize> = stream.read_to_end(&mut answer);
    let answer = String::from_utf8_lossy(&answer).to_lowercase();
    let ok = answer
        .lines()
        .next()
        .is_some_and(|status| status.split(' ').nth(1) == Some("200"));
    ok && answer.contains("<html")
}

/// `cargo run` laisse sa place au jeu une fois la compilation finie :
/// arrêter ce processus arrête le jeu.
fn stop(game: &mut Child) {
    let _: std::io::Result<()> = game.kill();
    let _: std::io::Result<std::process::ExitStatus> = game.wait();
}
