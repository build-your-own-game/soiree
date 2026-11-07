//! Le scénario `soiree-banc machine` (PHASE-3, lot 3a) : les essais de
//! `moteur_commun::Machine` sur `moteur-navigateur`, dans Chrome, et leur contrôle négatif.
//!
//! 1. La règle « mêmes lints » de `web/` ([`crate::web`]).
//! 2. La construction, sauf `--sans-construire` : l'exemple `essais-navigateur` de
//!    `web/moteur-navigateur`, pour `wasm32-unknown-unknown`, puis `wasm-bindgen
//!    --target web`, à la version de la crate (0.2.129, REM-156). Elle finit avant que
//!    le navigateur ne parte : jamais de navigateur pendant une compilation (OUT-89).
//! 3. Deux passages, chacun dans un navigateur neuf sur un profil neuf (Chrome par
//!    défaut, Firefox avec `--navigateur firefox`, lot 3b) : la vraie machine,
//!    où tout doit réussir et où chaque famille que le lot demande doit avoir tourné ; puis
//!    la machine fautive (`?fautif=i64-par-nombre`, des `i64` lus par un nombre), où les
//!    essais `globales_i64` doivent échouer, et eux seuls.
//!
//! La page (`pages/machine.html`) envoie son rapport à `POST /banc/rapport` ; le banc
//! écoute en même temps sa console et ses exceptions (CDP, `BiDi`). Il ne retente rien
//! (OUT-91).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use crate::horloge::{maintenant, reste};
use crate::navigateur::{Lance, Navigateur};
use crate::rapport::{self, Attendu, Rapport, SUITE_MACHINE};
use crate::serveur::{Requete, Serveur};
use crate::{commun, web};

/// La version de `wasm-bindgen`, la crate de `web/` comme l'outil (PHASE-3, P32).
/// Sa seule définition est celle du relais, qui construit le paquet de la page (lot 10b).
pub use relais::preparer::WASM_BINDGEN;

/// Les réglages du scénario.
#[derive(Debug, Clone)]
pub struct Options {
    /// Construire l'exemple et sa colle avant de jouer.
    pub construire: bool,
    /// Le navigateur.
    pub navigateur: Navigateur,
    /// Son exécutable, s'il n'est pas celui de la machine.
    pub executable: Option<PathBuf>,
    /// Le plus long qu'on attend le rapport d'une page.
    pub delai: Duration,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            construire: true,
            navigateur: Navigateur::Chrome,
            executable: None,
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

pub(crate) fn lancer(c: &mut Command, quoi: &str) -> Result<String, String> {
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

/// Construit les exemples `exemples` de `web/moteur-navigateur` pour
/// `wasm32-unknown-unknown`, en une commande, puis la colle de chacun dans `paquet`
/// (`wasm-bindgen --target web`, à la version de la crate : REM-156).
pub(crate) fn construire(web: &Path, paquet: &Path, exemples: &[&str]) -> Result<(), String> {
    construire_de(web, paquet, "moteur-navigateur", exemples)
}

/// Construit les exemples `exemples` de la crate `crate_web` de `web/`, puis leur colle
/// dans `paquet`, comme [`construire`] (lot 9 : `ecran`).
pub(crate) fn construire_de(
    web: &Path,
    paquet: &Path,
    crate_web: &str,
    exemples: &[&str],
) -> Result<(), String> {
    let cible = web.join("target");
    let mut cargo = Command::new("cargo");
    cargo.current_dir(web).args(["build", "-p", crate_web]);
    for e in exemples {
        cargo.args(["--example", e]);
    }
    cargo
        .args([
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "--target-dir",
        ])
        .arg(&cible);
    if std::env::var_os("CARGO_BUILD_JOBS").is_none() {
        cargo.env("CARGO_BUILD_JOBS", "4");
    }
    lancer(
        &mut cargo,
        &format!("cargo build de {}", exemples.join(", ")),
    )?;
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
    for e in exemples {
        let wasm = cible.join(format!(
            "wasm32-unknown-unknown/release/examples/{}.wasm",
            e.replace('-', "_")
        ));
        lancer(
            Command::new("wasm-bindgen")
                .args(["--target", "web", "--no-typescript", "--out-dir"])
                .arg(paquet)
                .arg(&wasm),
            &format!("wasm-bindgen de {e}"),
        )?;
    }
    Ok(())
}

/// Un passage : un navigateur neuf, la page, son rapport, puis le navigateur fermé.
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
    let mut nav = match Lance::lancer(o.navigateur, o.executable.as_deref(), profils) {
        Ok(c) => c,
        Err(e) => {
            p.verdict = Err(format!("pas exécuté : {e}"));
            return p;
        }
    };
    nav.version().clone_into(&mut p.navigateur);
    let url = format!("{origine}/banc/machine.html?fautif={fautif}");
    let corps = nav.ouvrir(&url).and_then(|()| {
        let echeance = maintenant()
            .checked_add(o.delai)
            .ok_or_else(|| String::from("délai"))?;
        loop {
            if let Ok(q) = recus.recv_timeout(Duration::from_millis(100)) {
                p.console.extend(nav.ecouter(Duration::from_millis(200))?);
                return Ok(q.corps);
            }
            p.console.extend(nav.ecouter(Duration::from_millis(100))?);
            if reste(echeance).is_zero() {
                return Err(format!("aucun rapport en {:?}", o.delai));
            }
        }
    });
    nav.fermer();
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
        construire(&web, &paquet, &["essais-navigateur"]).map_err(Arret::PasExecute)?;
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
