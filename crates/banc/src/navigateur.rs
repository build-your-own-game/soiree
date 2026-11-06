//! Les navigateurs du banc, derrière un même pilote (PHASE-3, § 8.2, lots 3b et 3c) :
//! Chrome par CDP ([`crate::chrome`]), Firefox par `WebDriver` `BiDi`
//! ([`crate::firefox`]), Safari par `safaridriver` ([`crate::safari`]).
//!
//! Chrome et Firefox partent sans interface, sur un profil neuf sous
//! `target/banc/profils/`, effacé à la fermeture ; le banc lit l'adresse de leur
//! protocole sur leur sortie d'erreur, ouvre une page, écoute sa console et ses
//! exceptions, puis les ferme (au plus 10 s, puis il les tue). Safari ouvre une vraie
//! fenêtre, sans profil neuf, et ne dit pas sa console ; sans « Autoriser
//! l'automatisation à distance », `safaridriver` refuse la session, et Safari est « pas
//! exécuté », avec son message, jamais réussi (OUT-01).

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::chrome::Chrome;
use crate::firefox::Firefox;
use crate::horloge::{maintenant, reste};
use crate::safari::Safari;

/// Un navigateur que le banc sait nommer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Navigateur {
    /// Google Chrome, par CDP.
    Chrome,
    /// Firefox, par `WebDriver` `BiDi`.
    Firefox,
    /// Safari, par `safaridriver`.
    Safari,
}

impl Navigateur {
    /// Son nom sur la ligne de commande.
    pub const fn nom(self) -> &'static str {
        match self {
            Navigateur::Chrome => "chrome",
            Navigateur::Firefox => "firefox",
            Navigateur::Safari => "safari",
        }
    }

    /// Le navigateur de ce nom.
    pub fn depuis_nom(nom: &str) -> Option<Navigateur> {
        [Navigateur::Chrome, Navigateur::Firefox, Navigateur::Safari]
            .into_iter()
            .find(|n| n.nom() == nom.trim())
    }

    /// Des noms séparés par des virgules (`chrome,firefox`).
    pub fn liste(noms: &str) -> Result<Vec<Navigateur>, String> {
        noms.split(',')
            .filter(|n| !n.trim().is_empty())
            .map(|n| Navigateur::depuis_nom(n).ok_or_else(|| format!("navigateur inconnu : {n}")))
            .collect()
    }

    /// Son exécutable sur la machine de l'hôte (macOS).
    pub fn executable(self) -> PathBuf {
        PathBuf::from(match self {
            Navigateur::Chrome => crate::chrome::CHROME,
            Navigateur::Firefox => crate::firefox::FIREFOX,
            Navigateur::Safari => crate::safari::SAFARIDRIVER,
        })
    }
}

/// Un navigateur lancé.
pub enum Lance {
    /// Chrome.
    Chrome(Chrome),
    /// Firefox.
    Firefox(Firefox),
    /// Safari.
    Safari(Safari),
}

impl Lance {
    /// Lance `n` (son exécutable : `executable`, ou celui de la machine ; pour Safari,
    /// `safaridriver`) sur un profil neuf sous `profils` (Safari n'en a pas). `Err` : il
    /// ne part pas, ou il refuse la session (Safari sans l'automatisation à distance) ;
    /// c'est « pas exécuté ».
    pub fn lancer(
        n: Navigateur,
        executable: Option<&Path>,
        profils: &Path,
    ) -> Result<Lance, String> {
        let exe = executable.map_or_else(|| n.executable(), Path::to_path_buf);
        match n {
            Navigateur::Chrome => Chrome::lancer(&exe, profils).map(Lance::Chrome),
            Navigateur::Firefox => Firefox::lancer(&exe, profils).map(Lance::Firefox),
            Navigateur::Safari => Safari::lancer(&exe).map(Lance::Safari),
        }
    }

    /// Sa version, telle qu'il la dit.
    pub fn version(&self) -> &str {
        match self {
            Lance::Chrome(c) => &c.version,
            Lance::Firefox(f) => &f.version,
            Lance::Safari(s) => &s.version,
        }
    }

    /// Ouvre `url` dans un onglet, et écoute sa console et ses exceptions.
    pub fn ouvrir(&mut self, url: &str) -> Result<(), String> {
        match self {
            Lance::Chrome(c) => c.ouvrir(url),
            Lance::Firefox(f) => f.ouvrir(url),
            Lance::Safari(s) => s.ouvrir(url),
        }
    }

    /// Écoute la page au plus `delai`, et rend ce que sa console et ses exceptions ont
    /// dit (rien pour Safari, que `WebDriver` classique ne dit pas).
    pub fn ecouter(&mut self, delai: Duration) -> Result<Vec<String>, String> {
        match self {
            Lance::Chrome(c) => c.ecouter(delai),
            Lance::Firefox(f) => f.ecouter(delai),
            Lance::Safari(s) => Ok(s.ecouter(delai)),
        }
    }

    /// Le ferme, et efface son profil.
    pub fn fermer(self) {
        match self {
            Lance::Chrome(c) => c.fermer(),
            Lance::Firefox(f) => f.fermer(),
            Lance::Safari(s) => s.fermer(),
        }
    }
}

/// Un profil neuf : un dossier qui n'existait pas, sous `dossier`.
pub(crate) fn profil_neuf(dossier: &Path, prefixe: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dossier).map_err(|e| format!("{}: {e}", dossier.display()))?;
    for n in 0u32..1000 {
        let p = dossier.join(format!("{prefixe}-{}-{n}", std::process::id()));
        if std::fs::create_dir(&p).is_ok() {
            return Ok(p);
        }
    }
    Err(format!("aucun profil neuf sous {}", dossier.display()))
}

/// Efface un profil ; les processus fils d'un navigateur y écrivent encore un instant
/// après sa fin.
pub(crate) fn effacer(profil: &Path) {
    for _ in 0..20 {
        if std::fs::remove_dir_all(profil).is_ok() || !profil.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(250));
    }
    eprintln!("profil laissé : {}", profil.display());
}

/// Attend la fin du processus au plus 10 s, puis le tue.
pub(crate) fn arreter_processus(processus: &mut Child) {
    let echeance = maintenant().checked_add(Duration::from_secs(10));
    loop {
        match processus.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) => {}
        }
        if echeance.is_none_or(|e| reste(e).is_zero()) {
            let _: std::io::Result<()> = processus.kill();
            let _: std::io::Result<std::process::ExitStatus> = processus.wait();
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// Lit `flux` ligne à ligne sur un fil, jusqu'au bout : un tuyau plein bloquerait le
/// navigateur.
fn lire_les_lignes(flux: impl Read + Send + 'static, envoi: mpsc::Sender<String>, nom: &str) {
    let _: std::io::Result<thread::JoinHandle<()>> = thread::Builder::new()
        .name(format!("banc-{nom}"))
        .spawn(move || {
            for l in BufReader::new(flux).lines() {
                let Ok(l) = l else { break };
                let _: Result<(), mpsc::SendError<String>> = envoi.send(l);
            }
        });
}

/// Attend sur la sortie d'erreur (et la sortie standard, si elle est lue) du
/// navigateur la ligne d'où `extraire` tire l'adresse de son protocole, au plus 30 s.
/// Sans elle, le navigateur est tué. Rend l'adresse.
pub(crate) fn attendre_l_adresse(
    processus: &mut Child,
    nom: &str,
    extraire: fn(&str) -> Option<&str>,
) -> Result<String, String> {
    let (envoi, recu) = mpsc::channel::<String>();
    if let Some(e) = processus.stderr.take() {
        lire_les_lignes(e, envoi.clone(), &format!("{nom}-stderr"));
    }
    if let Some(s) = processus.stdout.take() {
        lire_les_lignes(s, envoi.clone(), &format!("{nom}-stdout"));
    }
    drop(envoi);
    let echeance = maintenant()
        .checked_add(Duration::from_secs(30))
        .ok_or_else(|| String::from("délai"))?;
    let mut vu = Vec::new();
    loop {
        let Ok(l) = recu.recv_timeout(reste(echeance)) else {
            let _: std::io::Result<()> = processus.kill();
            let _: std::io::Result<std::process::ExitStatus> = processus.wait();
            return Err(format!(
                "{nom} n'a pas annoncé l'adresse de son protocole en 30 s : {}",
                vu.join(" | ")
            ));
        };
        if let Some(a) = extraire(&l) {
            // Le fil continue de lire le reste, et le jette.
            let _: std::io::Result<thread::JoinHandle<()>> = thread::Builder::new()
                .name(format!("banc-{nom}-vidange"))
                .spawn(move || for _ in recu {});
            return Ok(String::from(a));
        }
        vu.push(l);
    }
}

#[cfg(test)]
mod tests {
    use super::Navigateur;

    #[test]
    fn les_noms_des_navigateurs() {
        assert_eq!(
            Navigateur::liste("chrome,firefox").unwrap(),
            [Navigateur::Chrome, Navigateur::Firefox]
        );
        assert_eq!(Navigateur::liste("safari").unwrap(), [Navigateur::Safari]);
        assert!(Navigateur::liste("chrome,edge").is_err());
        assert_eq!(Navigateur::liste("").unwrap(), []);
    }
}
