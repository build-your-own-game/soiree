//! Le tronçon étagère–relais : ce qui arrive aux téléphones venus par
//! internet quand le lien de l'étagère avec le relais ralentit, se fige ou
//! tombe. Tous ces téléphones en dépendent.
//!
//! Le banc joue ici le rôle de l'étagère. Il lance un relais sur cet
//! ordinateur, le rejoint à travers le tuyau comme l'étagère rejoint le sien
//! (`internet.rs`, le même code), et lui amène les téléphones jusqu'à un jeu
//! lancé à part. Les téléphones d'essai, eux, vont droit au relais : c'est le
//! lien de l'étagère qu'on abîme, pas le leur.

use std::net::{Ipv4Addr, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::{Asked, Entering, Open, PATIENCE, Pipe, Setup, Target, chosen, free_port, measure};
use crate::internet::Passage;

/// La clé du relais d'essai : il ne vit que le temps du banc, sur cet
/// ordinateur.
const KEY: &str = "banc-du-reseau-cle-d-essai-0123456789";

const HOW: &str = "`soiree reseau lien` abîme le lien entre l'étagère et le relais. Il lui faut \
un relais et un jeu, lancés à part :

  --relais <programme>   le programme du relais. Dans le clone de la console :
                         `cargo build --release -p relais`, puis `target/release/relais`
  --jeu <port>           le port d'un jeu lancé sans l'étagère, sur cet ordinateur :
                         `PORT=8200 cargo run --release`, dans son dossier";

/// Le relais d'essai, lancé pour la durée du banc.
struct Relay {
    child: Child,
    phones: u16,
    shelf: u16,
}

impl Drop for Relay {
    fn drop(&mut self) {
        let _: std::io::Result<()> = self.child.kill();
        let _: std::io::Result<std::process::ExitStatus> = self.child.wait();
    }
}

impl Relay {
    fn start(program: &Path) -> Result<Self, String> {
        let (phones, shelf) = (free_port()?, free_port()?);
        let child = Command::new(program)
            .env("RELAIS_CLE", KEY)
            .env("RELAIS_PORT_TELEPHONES", phones.to_string())
            .env("RELAIS_PORT_ETAGERE", shelf.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| {
                format!("{} ne se lance pas ({error}).\n\n{HOW}", program.display())
            })?;
        let relay = Self {
            child,
            phones,
            shelf,
        };
        let since = Instant::now();
        while TcpStream::connect((Ipv4Addr::LOCALHOST, phones)).is_err() {
            if since.elapsed() > PATIENCE {
                return Err(format!("{} ne répond pas.", program.display()));
            }
            thread::sleep(Duration::from_millis(50));
        }
        Ok(relay)
    }
}

pub(super) fn run(runtime: &tokio::runtime::Runtime, asked: &Asked) -> Result<(), String> {
    let (Some(program), Some(game)) = (asked.relay.as_deref(), asked.game) else {
        return Err(HOW.to_owned());
    };
    if TcpStream::connect((Ipv4Addr::LOCALHOST, game)).is_err() {
        return Err(format!("aucun jeu ne répond sur le port {game}.\n\n{HOW}"));
    }
    let scenarios = chosen(asked)?;
    let relay = Relay::start(program)?;
    let local = |port: u16| Target {
        secure: false,
        host: "127.0.0.1".to_owned(),
        port,
        ticket: None,
    };
    // Le tuyau est entre l'étagère (le banc) et la porte que le relais lui ouvre.
    let pipe = Pipe::open(&local(relay.shelf), Open::ToThisComputer, Entering::Shelf)?;
    let passage = Passage::direct(
        &format!("http://127.0.0.1:{}", relay.phones),
        &format!("http://127.0.0.1:{}", pipe.port),
        KEY,
    )
    .ok_or("le relais d'essai n'a pas d'adresse lisible")?;
    let link = passage.open(game);
    let since = Instant::now();
    while !link.linked() {
        if since.elapsed() > PATIENCE {
            return Err("le banc ne rejoint pas le relais d'essai.".to_owned());
        }
        thread::sleep(Duration::from_millis(50));
    }
    println!(
        "soiree {} : le banc du réseau, sur le lien étagère–relais",
        crate::VERSION
    );
    println!(
        "Un relais d'essai tourne sur cet ordinateur. Le banc le rejoint comme une étagère, à \
         travers le tuyau,\net lui amène les téléphones jusqu'au jeu du port {game}.\n"
    );
    let target = local(relay.phones);
    let setup = Setup {
        pipe: &pipe,
        target: &target,
        through: relay.phones,
        wire: Some(link.flag()),
    };
    for scenario in scenarios {
        measure(runtime, &setup, scenario, asked)?;
    }
    Ok(())
}
