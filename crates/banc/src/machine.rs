//! Le scénario `soiree-banc machine` (PHASE-3, lot 3a) : les essais de
//! `moteur_commun::Machine` sur `moteur-navigateur`, dans Chrome, et leur contrôle négatif.
//!
//! 1. La règle « mêmes lints » de `web/` ([`crate::web`]).
//! 2. La construction, sauf `--sans-construire` : l'exemple `essais-navigateur` de
//!    `web/moteur-navigateur`, pour `wasm32-unknown-unknown`, puis `wasm-bindgen
//!    --target web`, à la version de la crate (0.2.129, REM-156). Elle finit avant que
//!    le navigateur ne parte : jamais de navigateur pendant une compilation (OUT-89).
//! 3. Deux passages, chacun dans un Chrome neuf sur un profil neuf : la vraie machine,
//!    où tout doit réussir et où chaque famille que le lot demande doit avoir tourné ; puis
//!    la machine fautive (`?fautif=i64-par-nombre`, des `i64` lus par un nombre), où les
//!    essais `globales_i64` doivent échouer, et eux seuls.
//!
//! La page (`pages/machine.html`) envoie son rapport à `POST /banc/rapport` ; le banc
//! écoute en même temps sa console et ses exceptions par CDP. Il ne retente rien (OUT-91).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use crate::chrome::Chrome;
use crate::horloge::{maintenant, reste};
use crate::rapport::{self, Attendu, Rapport, SUITE_MACHINE};
use crate::serveur::{Requete, Serveur};
use crate::{commun, web};

/// La version de `wasm-bindgen`, la crate de `web/` comme l'outil (PHASE-3, P32).
pub const WASM_BINDGEN: &str = "0.2.129";

/// Les réglages du scénario.
#[derive(Debug, Clone)]
pub struct Options {
    /// Construire l'exemple et sa colle avant de jouer.
    pub construire: bool,
    /// L'exécutable de Chrome.
    pub chrome: PathBuf,
    /// Le plus long qu'on attend le rapport d'une page.
    pub delai: Duration,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            construire: true,
            chrome: PathBuf::from(crate::chrome::CHROME),
            delai: Duration::from_secs(60),
        }
    }
}

/// Un passage dans un navigateur.
#[derive(Debug, Clone)]
pub struct Passage {
    /// `""` pour la vraie machine, ou le nom du défaut injecté.
    pub fautif: &'static str,
    /// Ce qu'on attendait.
    pub attendu: Attendu,
    /// Le navigateur (`Browser.getVersion`).
    pub navigateur: String,
    /// Le rapport, s'il est arrivé et se lit.
    pub rapport: Option<Rapport>,
    /// Ce que la console et les exceptions de la page ont dit.
    pub console: Vec<String>,
    /// Le jugement : ce que le passage montre, ou pourquoi il échoue.
    pub verdict: Result<String, String>,
}

/// Les deux passages du scénario.
pub const PASSAGES: [(&str, Attendu); 2] = [
    ("", Attendu::ToutReussi),
    ("i64-par-nombre", Attendu::SeuleEchoue("globales_i64")),
];

fn lancer(c: &mut Command, quoi: &str) -> Result<String, String> {
    let sortie = c.output().map_err(|e| format!("{quoi} : {e}"))?;
    if sortie.status.success() {
        Ok(String::from_utf8_lossy(&sortie.stdout).into_owned())
    } else {
        let erreur = String::from_utf8_lossy(&sortie.stderr);
        let fin: Vec<&str> = erreur.lines().rev().take(30).collect();
        Err(format!(
            "{quoi} : {}\n{}",
            sortie.status,
            fin.into_iter().rev().collect::<Vec<_>>().join("\n")
        ))
    }
}

/// Construit l'exemple `essais-navigateur` et sa colle dans `paquet`.
fn construire(web: &Path, paquet: &Path) -> Result<(), String> {
    let cible = web.join("target");
    let mut cargo = Command::new("cargo");
    cargo
        .current_dir(web)
        .args([
            "build",
            "-p",
            "moteur-navigateur",
            "--example",
            "essais-navigateur",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "--target-dir",
        ])
        .arg(&cible);
    if std::env::var_os("CARGO_BUILD_JOBS").is_none() {
        cargo.env("CARGO_BUILD_JOBS", "4");
    }
    lancer(&mut cargo, "cargo build de essais-navigateur")?;
    let version = lancer(
        Command::new("wasm-bindgen").arg("--version"),
        "wasm-bindgen --version",
    )?;
    if version.split_whitespace().nth(1) != Some(WASM_BINDGEN) {
        return Err(format!(
            "wasm-bindgen {} installé, la crate est à {WASM_BINDGEN} (REM-156)",
            version.trim()
        ));
    }
    let wasm = cible.join("wasm32-unknown-unknown/release/examples/essais_navigateur.wasm");
    lancer(
        Command::new("wasm-bindgen")
            .args(["--target", "web", "--no-typescript", "--out-dir"])
            .arg(paquet)
            .arg(&wasm),
        "wasm-bindgen",
    )?;
    Ok(())
}

/// Un passage : un Chrome neuf, la page, son rapport, puis Chrome fermé.
fn passer(
    o: &Options,
    origine: &str,
    profils: &Path,
    recus: &mpsc::Receiver<Requete>,
    (fautif, attendu): (&'static str, Attendu),
) -> Passage {
    let mut p = Passage {
        fautif,
        attendu,
        navigateur: String::new(),
        rapport: None,
        console: Vec::new(),
        verdict: Err(String::from("pas exécuté")),
    };
    let mut chrome = match Chrome::lancer(&o.chrome, profils) {
        Ok(c) => c,
        Err(e) => {
            p.verdict = Err(format!("pas exécuté : {e}"));
            return p;
        }
    };
    p.navigateur.clone_from(&chrome.version);
    let url = format!("{origine}/banc/machine.html?fautif={fautif}");
    let corps = chrome.ouvrir(&url).and_then(|()| {
        let echeance = maintenant()
            .checked_add(o.delai)
            .ok_or_else(|| String::from("délai"))?;
        loop {
            if let Ok(q) = recus.recv_timeout(Duration::from_millis(100)) {
                p.console
                    .extend(chrome.ecouter(Duration::from_millis(200))?);
                return Ok(q.corps);
            }
            p.console
                .extend(chrome.ecouter(Duration::from_millis(100))?);
            if reste(echeance).is_zero() {
                return Err(format!("aucun rapport en {:?}", o.delai));
            }
        }
    });
    chrome.fermer();
    p.verdict = corps.and_then(|c| {
        let r = rapport::lire(&c)?;
        let v = rapport::juger(&r, &SUITE_MACHINE, fautif, attendu);
        p.rapport = Some(r);
        v
    });
    p
}

/// Pourquoi le scénario s'arrête avant ses passages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arret {
    /// Une règle refuse ce qu'on allait jouer : un échec (code 1), jamais « pas
    /// exécuté », pour qu'un écart ne passe pas pour une machine qui ne permet rien.
    Echec(String),
    /// Rien n'a pu tourner : la construction, ou ce qu'elle demande, manque (code 2).
    PasExecute(String),
}

/// Joue le scénario, ou dit pourquoi il s'arrête avant ses passages.
pub fn jouer(o: &Options) -> Result<Vec<Passage>, Arret> {
    let soiree = commun();
    let web = soiree.join("web");
    let ecarts = web::ecarts(&soiree, &web);
    if !ecarts.is_empty() {
        return Err(Arret::Echec(format!(
            "la règle « mêmes lints » refuse web/ :\n  {}",
            ecarts.join("\n  ")
        )));
    }
    let banc = soiree.join("target/banc");
    let paquet = banc.join("paquet");
    if o.construire {
        construire(&web, &paquet).map_err(Arret::PasExecute)?;
    }
    if !paquet.join("essais_navigateur.js").exists() {
        return Err(Arret::PasExecute(format!(
            "{} manque : construire d'abord (sans --sans-construire)",
            paquet.join("essais_navigateur.js").display()
        )));
    }
    let (envoi, recus) = mpsc::channel();
    let serveur = Serveur::demarrer(
        vec![
            (String::from("/banc/"), soiree.join("crates/banc/pages")),
            (String::from("/paquet/"), paquet),
        ],
        envoi,
    )
    .map_err(|e| Arret::PasExecute(format!("serveur : {e}")))?;
    let origine = serveur.origine();
    let profils = banc.join("profils");
    let passages = PASSAGES
        .iter()
        .map(|a| passer(o, &origine, &profils, &recus, *a))
        .collect();
    serveur.arreter();
    Ok(passages)
}
