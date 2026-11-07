//! `soiree-relais` : le relais de la soirée (PHASE-3, § 2.5).
//!
//! ```text
//! soiree-relais [--port 7878] [--adresse <ip> | --sans-reseau] [--garde <dossier>] [--banc] [--enregistrer <fichier>]
//!               [--paquet <dossier>] [--jeux <dossier>]
//! soiree-relais oublier <SALLE> [--garde <dossier>]
//! soiree-relais preparer [--commun <dossier>] [--cible <dossier>]
//! soiree-relais jouer [<dossier du jeu>] [--module <crate>] [--catalogue <dossier>] [--non-eprouve]
//!                     [--sans-ouvrir] [--commun <dossier>] [options du relais]
//! ```
//!
//! `preparer` (lot 10b) construit le paquet de la page (`relais::preparer`), depuis le
//! commun dont ce relais a été compilé, et écrit son dossier. `jouer` (lot 10b, P35) : la
//! commande unique du jour J, depuis le dossier d'un jeu (`.` par défaut) : le module
//! construit par `soiree-epreuve --construire`, son rapport cherché par empreinte dans le
//! catalogue (`$HOME/.soiree/catalogue` par défaut), refusé sans rapport sauf
//! `--non-eprouve` (P17), le paquet (`--paquet`, sinon construit), le relais, puis
//! `open http://localhost:<port>/?jeu=<empreinte>` (sauf `--sans-ouvrir`) ; les options du
//! relais sont les siennes.
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
//! Code de sortie : 0 arrêté proprement, ou `oublier` fait, ou le paquet préparé ; 1 le
//! relais ne démarre pas (le port pris dans une famille de la boucle locale, une
//! `--adresse` qu'aucune interface ne porte), le paquet ne se construit pas, ou `jouer`
//! refuse un module sans rapport ; 2 une commande illisible.

#![forbid(unsafe_code)]
#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "un programme qui parle à l'hôte dans son terminal"
)]

use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use liaison::CodeSalle;
use relais::coque::{self, PORT, Reglages};
use relais::garde::Dossier;
use relais::http::{Banc, hote};
use relais::paquet::{Jeux, Paquet};

const USAGE: &str = "usage : soiree-relais [--port 7878] [--adresse <ip> | --sans-reseau] [--garde <dossier>] [--banc] [--enregistrer <fichier>] [--paquet <dossier>] [--jeux <dossier>]\n        soiree-relais oublier <SALLE> [--garde <dossier>]\n        soiree-relais preparer [--commun <dossier>] [--cible <dossier>]\n        soiree-relais jouer [<dossier du jeu>] [--module <crate>] [--catalogue <dossier>] [--non-eprouve] [--sans-ouvrir] [--commun <dossier>] [options du relais]";

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

/// Lit une option du relais (celles de `soiree-relais` et de `jouer`) ; `Ok(false)` : ce
/// n'en est pas une.
fn option_du_relais(
    a: &str,
    i: &mut std::slice::Iter<'_, String>,
    r: &mut Reglages,
) -> Result<bool, ExitCode> {
    match a {
        "--port" => match i.next().and_then(|p| p.parse().ok()) {
            Some(p) => r.port = p,
            None => return Err(usage()),
        },
        "--adresse" => match i.next().and_then(|p| p.parse::<IpAddr>().ok()) {
            Some(ip) => r.adresse = Some(ip),
            None => return Err(usage()),
        },
        "--garde" => match i.next() {
            Some(g) => r.garde = PathBuf::from(g),
            None => return Err(usage()),
        },
        "--enregistrer" => match i.next() {
            Some(e) => r.enregistrer = Some(PathBuf::from(e)),
            None => return Err(usage()),
        },
        "--banc" => r.banc = Banc::Oui,
        "--sans-reseau" => r.interfaces = Some(Vec::new()),
        "--paquet" => match i.next().map(|d| Paquet::lire(Path::new(d))) {
            Some(Ok(p)) => r.paquet = Some(p),
            Some(Err(e)) => {
                eprintln!("soiree-relais : le paquet : {e}");
                return Err(ExitCode::from(1));
            }
            None => return Err(usage()),
        },
        "--jeux" => match i.next().map(|d| Jeux::lire(Path::new(d))) {
            Some(Ok(j)) => r.jeux = j,
            Some(Err(e)) => {
                eprintln!("soiree-relais : les jeux : {e}");
                return Err(ExitCode::from(1));
            }
            None => return Err(usage()),
        },
        _ => return Ok(false),
    }
    Ok(true)
}

/// `soiree-relais preparer` : le paquet de la page, construit et nommé.
fn preparer(args: &[String]) -> ExitCode {
    let mut commun = relais::preparer::commun_des_sources();
    let mut cible = None;
    let mut i = args.iter();
    while let Some(a) = i.next() {
        match (a.as_str(), i.next()) {
            ("--commun", Some(d)) => commun = PathBuf::from(d),
            ("--cible", Some(d)) => cible = Some(PathBuf::from(d)),
            _ => return usage(),
        }
    }
    println!(
        "soiree-relais preparer : le paquet de la page, depuis {} (web/ pour wasm32, puis wasm-bindgen {})…",
        commun.display(),
        relais::preparer::WASM_BINDGEN
    );
    match relais::preparer::construire_le_paquet(&commun, cible.as_deref()) {
        Ok(d) => {
            println!("{}", d.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("soiree-relais preparer : {e}");
            ExitCode::from(1)
        }
    }
}

/// Le module du jeu, construit par `soiree-epreuve --construire` (la compilation de
/// l'étape 1 de l'épreuve) : son empreinte, son chemin, ses octets.
fn construire(
    jeu: &Path,
    module: Option<&str>,
) -> Result<(relais::jouer::Construit, Vec<u8>), String> {
    let epreuve = relais::jouer::epreuve();
    println!(
        "soiree-relais jouer : le module de {} se construit ({} --construire)…",
        jeu.display(),
        epreuve.display()
    );
    let mut c = std::process::Command::new(&epreuve);
    c.arg("--construire").arg(jeu);
    if let Some(m) = module {
        c.args(["--module", m]);
    }
    let sortie = match c.stderr(std::process::Stdio::inherit()).output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        Ok(o) => {
            return Err(format!(
                "{} --construire {} : {}",
                epreuve.display(),
                jeu.display(),
                o.status
            ));
        }
        Err(e) => return Err(format!("{} : {e}", epreuve.display())),
    };
    let construit = relais::jouer::lire_construit(&sortie)?;
    let octets = std::fs::read(&construit.chemin)
        .map_err(|e| format!("{} : {e}", construit.chemin.display()))?;
    Ok((construit, octets))
}

/// `soiree-relais jouer` : la commande unique du jour J (P35).
fn jouer(args: &[String]) -> ExitCode {
    let mut r = Reglages::soiree(&garde_par_defaut());
    let mut jeu = PathBuf::from(".");
    let mut module = None;
    let mut catalogue = relais::jouer::catalogue_par_defaut();
    let mut commun = relais::preparer::commun_des_sources();
    let (mut non_eprouve, mut ouvrir, mut jeu_donne) = (false, true, false);
    let mut i = args.iter();
    while let Some(a) = i.next() {
        match option_du_relais(a, &mut i, &mut r) {
            Ok(true) => continue,
            Ok(false) => {}
            Err(code) => return code,
        }
        match a.as_str() {
            "--non-eprouve" => non_eprouve = true,
            "--sans-ouvrir" => ouvrir = false,
            "--module" => match i.next() {
                Some(m) => module = Some(m.clone()),
                None => return usage(),
            },
            "--catalogue" => match i.next() {
                Some(c) => catalogue = PathBuf::from(c),
                None => return usage(),
            },
            "--commun" => match i.next() {
                Some(c) => commun = PathBuf::from(c),
                None => return usage(),
            },
            _ if a.starts_with("--") || jeu_donne => return usage(),
            _ => {
                jeu = PathBuf::from(a);
                jeu_donne = true;
            }
        }
    }
    if r.adresse.is_some() && r.interfaces.is_some() {
        return usage();
    }
    // 1. Le module, par la compilation de l'étape 1 de l'épreuve.
    let (construit, octets) = match construire(&jeu, module.as_deref()) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("soiree-relais jouer : {e}");
            return ExitCode::from(1);
        }
    };
    // 2. Son rapport, par empreinte (P17).
    let sans_rapport = if non_eprouve {
        relais::jouer::SansRapport::Jouer
    } else {
        relais::jouer::SansRapport::Refuser
    };
    r.jeux = match relais::jouer::les_jeux(&catalogue, &construit, octets, sans_rapport, &jeu) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("soiree-relais jouer : {e}");
            return ExitCode::from(1);
        }
    };
    let au_catalogue = r.jeux.eprouve(&construit.empreinte);
    // 3. Le paquet, le relais, l'écran.
    if r.paquet.is_none() {
        println!("soiree-relais jouer : le paquet de la page se construit…");
        let paquet =
            relais::preparer::construire_le_paquet(&commun, None).and_then(|d| Paquet::lire(&d));
        match paquet {
            Ok(p) => r.paquet = Some(p),
            Err(e) => {
                eprintln!("soiree-relais jouer : le paquet : {e}");
                return ExitCode::from(1);
            }
        }
    }
    let relais = match coque::lancer(&r) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("soiree-relais : {e}");
            return ExitCode::from(1);
        }
    };
    let url = format!(
        "http://localhost:{}/?jeu={}",
        relais.port(),
        construit.empreinte
    );
    annoncer(&relais, &r);
    if au_catalogue {
        println!(
            "le module {} : éprouvé (son rapport est au catalogue)",
            construit.empreinte
        );
    } else {
        println!(
            "le module {} : NON ÉPROUVÉ (--non-eprouve) ; l'écran l'affiche en permanence",
            construit.empreinte
        );
    }
    println!("l'écran : {url}");
    if ouvrir && let Err(e) = std::process::Command::new("open").arg(&url).status() {
        eprintln!("soiree-relais jouer : open {url} : {e} ; ouvre l'adresse à la main");
    }
    relais.attendre();
    ExitCode::SUCCESS
}

/// Ce que le relais lancé dit à l'hôte dans son terminal.
fn annoncer(relais: &coque::Relais, r: &Reglages) {
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
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("oublier") => return oublier(args.get(1..).unwrap_or(&[])),
        Some("preparer") => return preparer(args.get(1..).unwrap_or(&[])),
        Some("jouer") => return jouer(args.get(1..).unwrap_or(&[])),
        _ => {}
    }
    let mut r = Reglages::soiree(&garde_par_defaut());
    let mut i = args.iter();
    while let Some(a) = i.next() {
        match option_du_relais(a, &mut i, &mut r) {
            Ok(true) => {}
            Ok(false) => return usage(),
            Err(code) => return code,
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
    annoncer(&relais, &r);
    relais.attendre();
    ExitCode::SUCCESS
}
