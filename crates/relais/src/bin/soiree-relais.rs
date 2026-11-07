//! `soiree-relais` : le relais de la soirée (PHASE-3, § 2.5).
//!
//! ```text
//! soiree-relais [--port 7878] [--adresse <ip> | --sans-reseau] [--garde <dossier>] [--banc] [--enregistrer <fichier>]
//!               [--paquet <dossier>] [--jeux <dossier>]
//! soiree-relais oublier <SALLE> [--garde <dossier>]
//! ```
//!
//! Sans `--garde`, la garde est dans `$HOME/.soiree/garde`. `--banc` : les outils natifs
//! du banc, sans `Origin`, prennent tous les rôles depuis la boucle locale ; jamais en
//! soirée. `--sans-reseau` : la boucle locale seulement, comme une machine sans Wi-Fi
//! (le banc s'en sert pour ne rien ouvrir sur le réseau). `--enregistrer` : tout ce que le relais voit, dans ce fichier (le banc y
//! cherche ce qu'il ne doit pas voir). `--paquet` : le paquet de la page, un dossier nommé
//! par son empreinte (lot 10a). `--jeux` : les modules livrés, `<empreinte>.wasm`, que
//! l'écran charge par `GET /jeux/…` (lot 10a). Les deux se lisent au lancement, une fois. `oublier` efface la preuve et la garde d'une
//! salle : l'autorité suivante enregistre la sienne sous le même code (P41).
//!
//! Code de sortie : 0 arrêté proprement, ou `oublier` fait ; 1 le relais ne démarre pas
//! (le port pris dans une famille de la boucle locale, une `--adresse` qu'aucune
//! interface ne porte) ; 2 une commande illisible.

#![forbid(unsafe_code)]
#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "un programme qui parle à l'hôte dans son terminal"
)]

use std::net::IpAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use liaison::CodeSalle;
use relais::coque::{self, PORT, Reglages};
use relais::garde::Dossier;
use relais::http::{Banc, hote};
use relais::paquet::{Jeux, Paquet};

const USAGE: &str = "usage : soiree-relais [--port 7878] [--adresse <ip> | --sans-reseau] [--garde <dossier>] [--banc] [--enregistrer <fichier>] [--paquet <dossier>] [--jeux <dossier>]\n        soiree-relais oublier <SALLE> [--garde <dossier>]";

fn garde_par_defaut() -> PathBuf {
    std::env::var_os("HOME").map_or_else(
        || PathBuf::from(".soiree/garde"),
        |h| PathBuf::from(h).join(".soiree/garde"),
    )
}

fn oublier(args: &[String]) -> ExitCode {
    let mut salle = None;
    let mut garde = garde_par_defaut();
    let mut i = args.iter();
    while let Some(a) = i.next() {
        match a.as_str() {
            "--garde" => match i.next() {
                Some(g) => garde = PathBuf::from(g),
                None => return usage(),
            },
            s if salle.is_none() => {
                let Some(c) = CodeSalle::lire(s) else {
                    eprintln!(
                        "« {s} » n'est pas un code de salle (8 chiffres hexadécimaux majuscules)"
                    );
                    return ExitCode::from(2);
                };
                salle = Some(c);
            }
            _ => return usage(),
        }
    }
    let Some(salle) = salle else {
        return usage();
    };
    match Dossier::ouvrir(&garde).and_then(|d| d.oublier(salle)) {
        Ok(true) => {
            println!(
                "salle {salle} oubliée : sa preuve et sa garde sont effacées ; l'autorité suivante enregistrera la sienne sous ce code"
            );
            ExitCode::SUCCESS
        }
        Ok(false) => {
            println!("salle {salle} : rien à oublier dans {}", garde.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("oublier {salle} dans {} : {e}", garde.display());
            ExitCode::from(1)
        }
    }
}

fn usage() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("oublier") {
        return oublier(args.get(1..).unwrap_or(&[]));
    }
    let mut r = Reglages::soiree(&garde_par_defaut());
    let mut i = args.iter();
    while let Some(a) = i.next() {
        match a.as_str() {
            "--port" => match i.next().and_then(|p| p.parse().ok()) {
                Some(p) => r.port = p,
                None => return usage(),
            },
            "--adresse" => match i.next().and_then(|p| p.parse::<IpAddr>().ok()) {
                Some(ip) => r.adresse = Some(ip),
                None => return usage(),
            },
            "--garde" => match i.next() {
                Some(g) => r.garde = PathBuf::from(g),
                None => return usage(),
            },
            "--enregistrer" => match i.next() {
                Some(e) => r.enregistrer = Some(PathBuf::from(e)),
                None => return usage(),
            },
            "--banc" => r.banc = Banc::Oui,
            "--sans-reseau" => r.interfaces = Some(Vec::new()),
            "--paquet" => match i.next().map(|d| Paquet::lire(std::path::Path::new(d))) {
                Some(Ok(p)) => r.paquet = Some(p),
                Some(Err(e)) => {
                    eprintln!("soiree-relais : le paquet : {e}");
                    return ExitCode::from(1);
                }
                None => return usage(),
            },
            "--jeux" => match i.next().map(|d| Jeux::lire(std::path::Path::new(d))) {
                Some(Ok(j)) => r.jeux = j,
                Some(Err(e)) => {
                    eprintln!("soiree-relais : les jeux : {e}");
                    return ExitCode::from(1);
                }
                None => return usage(),
            },
            _ => return usage(),
        }
    }
    if r.adresse.is_some() && r.interfaces.is_some() {
        return usage();
    }
    let relais = match coque::lancer(&r) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("soiree-relais : {e}");
            return ExitCode::from(1);
        }
    };
    let port = relais.port();
    println!(
        "soiree-relais : http://localhost:{port}/ (l'écran), garde dans {}",
        r.garde.display()
    );
    if relais.annoncees().is_empty() {
        println!(
            "Pas de réseau : les téléphones ne peuvent pas se relier ; Tier list se joue sans eux."
        );
    }
    for ip in relais.annoncees() {
        println!("les téléphones : http://{}/s/<SALLE>", hote(*ip, port));
    }
    if r.banc == Banc::Oui {
        println!(
            "mode banc : les outils natifs sans Origin prennent tous les rôles depuis la boucle locale"
        );
    }
    if port != PORT {
        println!("(port {port}, et non {PORT})");
    }
    relais.attendre();
    ExitCode::SUCCESS
}
