//! Les navigateurs du banc, derrière un même pilote (PHASE-3, § 8.2, lots 3b et 3c) :
//! Chrome par CDP ([`crate::chrome`]), Firefox par `WebDriver` `BiDi`
//! ([`crate::firefox`]), Safari par `safaridriver` ([`crate::safari`]).
//!
//! Chrome et Firefox partent sans interface (Firefox avec sa fenêtre sur demande,
//! [`Interface`]), sur un profil neuf sous
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

use crate::chrome::{Chrome, GesteSouris};
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

/// Où s'ouvre un onglet de plus (correction du lot 13b) : devant les autres, ou derrière
/// celui de l'écran, qui reste alors devant, visible et en plein écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Devant : l'onglet ouvert passe au premier plan (le défaut des navigateurs).
    Devant,
    /// Derrière : l'onglet ouvert reste à l'arrière-plan.
    Derriere,
}

impl Place {
    /// La valeur de `background`, dans `Target.createTarget` comme dans
    /// `browsingContext.create`.
    pub const fn en_arriere_plan(self) -> bool {
        matches!(self, Self::Derriere)
    }
}

/// Avec ou sans fenêtre (lot 10b, après la vérification) : Firefox sans interface refuse
/// le plein écran à un vrai clic du banc ; avec sa fenêtre, il le donne. Chrome reste
/// toujours sans interface (Keyboard Lock y suffit), Safari a toujours la sienne.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Interface {
    /// Sans fenêtre (`--headless`) : le défaut de chaque scénario.
    #[default]
    Sans,
    /// Firefox avec sa fenêtre, sur l'écran de l'hôte (`soiree-banc aide --avec-interface`) ;
    /// Chrome aussi depuis le lot 13 (`soiree-banc arriere-plan --avec-interface`, M6).
    Avec,
}

/// Le caractère d'une touche de chiffre ou de lettre (`Digit5` → `5`, `KeyD` → `d`),
/// celui que le banc tape (lot 13) ; `None` pour une autre touche.
pub fn caractere_de(code: &str) -> Option<char> {
    let c = code
        .strip_prefix("Digit")
        .or_else(|| code.strip_prefix("Key"))?;
    let mut lettres = c.chars();
    match (lettres.next(), lettres.next()) {
        (Some(x), None) if x.is_ascii_alphanumeric() => Some(x.to_ascii_lowercase()),
        _ => None,
    }
}

/// La session de l'écran de macOS est-elle verrouillée ? (lot 13b) Session verrouillée,
/// une fenêtre ne peut pas entrer dans le plein écran : Chrome le quitte aussitôt entré, et
/// le voile du commun couvre le jeu. Lu sans rien changer, dans `IOConsoleUsers` du
/// registre d'`IOKit` (`ioreg -n Root -d1`) ; `None` si l'outil ne répond pas ou ne le dit
/// pas.
pub fn session_verrouillee() -> Option<bool> {
    let sortie = std::process::Command::new("/usr/sbin/ioreg")
        .args(["-n", "Root", "-d1"])
        .output()
        .ok()?;
    verrou_dans(&String::from_utf8_lossy(&sortie.stdout))
}

/// Le verrou de la session à la console, dans le texte de `ioreg -n Root -d1` :
/// `"CGSSessionScreenIsLocked"=Yes` dans l'entrée de `IOConsoleUsers` qui porte
/// `"kCGSSessionOnConsoleKey"=Yes` (l'entrée n'a pas la clé tant que l'écran n'a jamais
/// été verrouillé : `false`).
pub fn verrou_dans(texte: &str) -> Option<bool> {
    let ligne = texte.lines().find(|l| l.contains("\"IOConsoleUsers\""))?;
    let a_la_console = ligne
        .split('}')
        .find(|e| e.contains("\"kCGSSessionOnConsoleKey\"=Yes"))?;
    Some(a_la_console.contains("\"CGSSessionScreenIsLocked\"=Yes"))
}

/// La session de l'écran, en mots, pour un rapport avec interface (lot 13b).
pub fn note_de_la_session() -> String {
    match session_verrouillee() {
        Some(true) => String::from(
            "la session de l'écran de macOS est VERROUILLÉE (ioreg : CGSSessionScreenIsLocked) : une fenêtre n'y entre pas dans le plein écran",
        ),
        Some(false) => String::from("la session de l'écran de macOS est déverrouillée"),
        None => String::from("la session de l'écran de macOS : verrou non lu"),
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
        Lance::lancer_avec(n, executable, profils, Interface::Sans)
    }

    /// Comme [`Lance::lancer`], Firefox et Chrome (depuis le lot 13) avec leur fenêtre si
    /// `interface` le demande ; Safari n'en change pas ([`Interface`]).
    pub fn lancer_avec(
        n: Navigateur,
        executable: Option<&Path>,
        profils: &Path,
        interface: Interface,
    ) -> Result<Lance, String> {
        let exe = executable.map_or_else(|| n.executable(), Path::to_path_buf);
        match n {
            Navigateur::Chrome => Chrome::lancer_avec(&exe, profils, interface).map(Lance::Chrome),
            Navigateur::Firefox => {
                Firefox::lancer_avec(&exe, profils, interface).map(Lance::Firefox)
            }
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

    /// Ouvre un onglet de plus à `url` (lot 10a) ; rend son rang.
    pub fn nouvel_onglet(&mut self, url: &str) -> Result<usize, String> {
        match self {
            Lance::Chrome(c) => c.nouvel_onglet(url),
            Lance::Firefox(f) => f.nouvel_onglet(url),
            Lance::Safari(_) => Err(String::from("Safari : un seul onglet par le banc")),
        }
    }

    /// Ouvre un onglet de plus à `url`, **derrière** celui de l'écran, qui reste devant,
    /// visible et en plein écran (l'écho de M14 avec un autre onglet, correction du lot
    /// 13b) ; rend son rang.
    pub fn nouvel_onglet_derriere(&mut self, url: &str) -> Result<usize, String> {
        match self {
            Lance::Chrome(c) => c.nouvel_onglet_derriere(url),
            Lance::Firefox(f) => f.nouvel_onglet_derriere(url),
            Lance::Safari(_) => Err(String::from("Safari : un seul onglet par le banc")),
        }
    }

    /// Évalue `expression` dans l'onglet, sa promesse attendue ; rend sa valeur.
    pub fn evaluer(
        &mut self,
        onglet: usize,
        expression: &str,
    ) -> Result<serde_json::Value, String> {
        match self {
            Lance::Chrome(c) => c.evaluer(onglet, expression),
            Lance::Firefox(f) => f.evaluer(onglet, expression),
            Lance::Safari(_) => Err(String::from("Safari : pas d'évaluation par le banc")),
        }
    }

    /// Recharge l'onglet.
    pub fn recharger(&mut self, onglet: usize) -> Result<(), String> {
        match self {
            Lance::Chrome(c) => c.recharger(onglet),
            Lance::Firefox(f) => f.recharger(onglet),
            Lance::Safari(_) => Err(String::from("Safari : pas de rechargement par le banc")),
        }
    }

    /// Ferme l'onglet.
    pub fn fermer_onglet(&mut self, onglet: usize) -> Result<(), String> {
        match self {
            Lance::Chrome(c) => c.fermer_onglet(onglet),
            Lance::Firefox(f) => f.fermer_onglet(onglet),
            Lance::Safari(_) => Err(String::from("Safari : un seul onglet par le banc")),
        }
    }

    /// Passe un onglet vierge devant : les autres sont cachés.
    pub fn cacher(&mut self) -> Result<(), String> {
        match self {
            Lance::Chrome(c) => c.cacher(),
            Lance::Firefox(f) => f.cacher(),
            Lance::Safari(_) => Err(String::from("Safari : un seul onglet par le banc")),
        }
    }

    /// Remet l'onglet devant.
    pub fn montrer(&mut self, onglet: usize) -> Result<(), String> {
        match self {
            Lance::Chrome(c) => c.montrer(onglet),
            Lance::Firefox(f) => f.montrer(onglet),
            Lance::Safari(_) => Err(String::from("Safari : un seul onglet par le banc")),
        }
    }

    /// Ouvre une autre fenêtre du navigateur par-dessus celle de l'écran (M6, lot 13 ;
    /// avec interface).
    pub fn recouvrir(&mut self) -> Result<(), String> {
        match self {
            Lance::Chrome(c) => c.recouvrir(),
            Lance::Firefox(f) => f.recouvrir(),
            Lance::Safari(_) => Err(String::from("Safari : une seule fenêtre par le banc")),
        }
    }

    /// Réduit la fenêtre de l'onglet dans le Dock (M6, lot 13 ; avec interface). Rend ce
    /// qui a été fait, pour le rapport : dans Chrome, l'état relu de la fenêtre, et une
    /// faute s'il n'est pas `minimized` (lot 13b).
    pub fn reduire(&mut self, onglet: usize) -> Result<String, String> {
        match self {
            Lance::Chrome(c) => c.etat_de_la_fenetre(onglet, "minimized"),
            Lance::Firefox(f) => f
                .etat_de_la_fenetre("minimized")
                .map(|()| String::from("browser.setClientWindowState minimized, non relu")),
            Lance::Safari(_) => Err(String::from("Safari : pas de fenêtre réduite par le banc")),
        }
    }

    /// Rend la fenêtre réduite ; comme [`Lance::reduire`].
    pub fn restaurer(&mut self, onglet: usize) -> Result<String, String> {
        match self {
            Lance::Chrome(c) => c.etat_de_la_fenetre(onglet, "normal"),
            Lance::Firefox(f) => f
                .etat_de_la_fenetre("normal")
                .map(|()| String::from("browser.setClientWindowState normal, non relu")),
            Lance::Safari(_) => Err(String::from("Safari : pas de fenêtre réduite par le banc")),
        }
    }

    /// Appuie sur F8 dans l'onglet, par la chaîne d'entrées du navigateur.
    pub fn touche_f8(&mut self, onglet: usize) -> Result<(), String> {
        match self {
            Lance::Chrome(c) => c.touche_f8(onglet),
            Lance::Firefox(f) => f.touche_f8(onglet),
            Lance::Safari(_) => Err(String::from("Safari : pas de clavier par le banc")),
        }
    }

    /// Appuie sur Échap dans l'onglet, par la chaîne d'entrées du navigateur (lot 10b).
    pub fn touche_echap(&mut self, onglet: usize) -> Result<(), String> {
        match self {
            Lance::Chrome(c) => c.touche_echap(onglet),
            Lance::Firefox(f) => f.touche_echap(onglet),
            Lance::Safari(_) => Err(String::from("Safari : pas de clavier par le banc")),
        }
    }

    /// Clique en `(x, y)`, pixels de la fenêtre, par la chaîne d'entrées (lot 10b).
    pub fn cliquer(&mut self, onglet: usize, x: f64, y: f64) -> Result<(), String> {
        match self {
            Lance::Chrome(c) => c.cliquer(onglet, x, y),
            Lance::Firefox(f) => f.cliquer(onglet, x, y),
            Lance::Safari(_) => Err(String::from("Safari : pas de pointeur par le banc")),
        }
    }

    /// Un écran de téléphone dans l'onglet (lot 12) : Chrome l'émule, tactile compris ;
    /// Firefox ne règle que la taille de sa fenêtre (ses pointeurs tactiles viennent de
    /// `input.performActions`).
    pub fn telephone(&mut self, onglet: usize, largeur: u32, hauteur: u32) -> Result<(), String> {
        match self {
            Lance::Chrome(c) => c.emuler_un_telephone(onglet, largeur, hauteur),
            Lance::Firefox(f) => f.dimensionner(onglet, largeur, hauteur),
            Lance::Safari(_) => Err(String::from("Safari : pas d'émulation par le banc")),
        }
    }

    /// Un doigt (lot 12) : `touchStart`, `touchMove` ou `touchEnd` en `(x, y)`, par la
    /// chaîne d'entrées du navigateur.
    pub fn doigt(&mut self, onglet: usize, sorte: &str, x: f64, y: f64) -> Result<(), String> {
        match self {
            Lance::Chrome(c) => c.doigt(onglet, sorte, x, y),
            Lance::Firefox(f) => f.doigt(onglet, sorte, x, y),
            Lance::Safari(_) => Err(String::from("Safari : pas de pointeur par le banc")),
        }
    }

    /// Un toucher bref en `(x, y)` (lot 12).
    pub fn toucher(&mut self, onglet: usize, x: f64, y: f64) -> Result<(), String> {
        self.doigt(onglet, "touchStart", x, y)?;
        self.doigt(onglet, "touchEnd", x, y)
    }

    /// Tape `texte` dans l'élément qui a le focus (lot 12).
    pub fn taper(&mut self, onglet: usize, texte: &str) -> Result<(), String> {
        match self {
            Lance::Chrome(c) => c.taper(onglet, texte),
            Lance::Firefox(f) => f.taper(onglet, texte),
            Lance::Safari(_) => Err(String::from("Safari : pas de clavier par le banc")),
        }
    }

    /// Donne à l'onglet une fenêtre de `largeur` × `hauteur` pixels CSS, au rapport de
    /// pixels `dpr` (lot 11 ; `dpr` depuis la remontée du lot 10b).
    pub fn fenetre(
        &mut self,
        onglet: usize,
        largeur: u32,
        hauteur: u32,
        dpr: f64,
    ) -> Result<(), String> {
        match self {
            Lance::Chrome(c) => c.fenetre(onglet, largeur, hauteur, dpr),
            Lance::Firefox(f) => f.fenetre(onglet, largeur, hauteur, dpr),
            Lance::Safari(_) => Err(String::from("Safari : pas de taille par le banc")),
        }
    }

    /// Le dossier où vont ses téléchargements (lot 11).
    pub fn telechargements(&self) -> Result<PathBuf, String> {
        match self {
            Lance::Chrome(c) => Ok(c.telechargements()),
            Lance::Firefox(f) => Ok(f.telechargements()),
            Lance::Safari(_) => Err(String::from("Safari : pas de téléchargement par le banc")),
        }
    }

    /// Donne des fichiers à un `<input type=file>` de l'onglet (lot 11).
    pub fn fichiers(
        &mut self,
        onglet: usize,
        selecteur: &str,
        fichiers: &[PathBuf],
    ) -> Result<(), String> {
        match self {
            Lance::Chrome(c) => c.fichiers(onglet, selecteur, fichiers),
            Lance::Firefox(f) => f.fichiers(onglet, selecteur, fichiers),
            Lance::Safari(_) => Err(String::from("Safari : pas de fichiers par le banc")),
        }
    }

    /// Joue des gestes de la souris dans l'onglet, en pixels de la fenêtre (lot 11).
    pub fn souris(&mut self, onglet: usize, gestes: &[GesteSouris]) -> Result<(), String> {
        match self {
            Lance::Chrome(c) => c.souris(onglet, gestes),
            Lance::Firefox(f) => f.souris(onglet, gestes),
            Lance::Safari(_) => Err(String::from("Safari : pas de souris par le banc")),
        }
    }

    /// Tape un texte dans l'élément qui a le focus (lot 11).
    pub fn texte(&mut self, onglet: usize, texte: &str) -> Result<(), String> {
        match self {
            Lance::Chrome(c) => c.texte(onglet, texte),
            Lance::Firefox(f) => f.texte(onglet, texte),
            Lance::Safari(_) => Err(String::from("Safari : pas de clavier par le banc")),
        }
    }

    /// Appuie sur une touche nommée (`Enter`, `Escape`, `Tab`, lot 11).
    pub fn touche(&mut self, onglet: usize, code: &str) -> Result<(), String> {
        match self {
            Lance::Chrome(c) => c.touche(onglet, code),
            Lance::Firefox(f) => f.touche(onglet, code),
            Lance::Safari(_) => Err(String::from("Safari : pas de clavier par le banc")),
        }
    }

    /// Chrome, s'il l'est : ce que CDP seul permet (la trace).
    pub fn chrome(&mut self) -> Option<&mut Chrome> {
        match self {
            Lance::Chrome(c) => Some(c),
            Lance::Firefox(_) | Lance::Safari(_) => None,
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
    use super::{Navigateur, verrou_dans};

    /// Lot 13b : le verrou de la session à la console se lit dans `ioreg -n Root -d1`
    /// (les deux entrées relevées le 2026-11-08 sur la machine de l'hôte, abrégées), et
    /// pas celui d'une autre session.
    #[test]
    fn le_verrou_de_la_session_a_la_console_se_lit() {
        let verrouillee = r#"  | {
      "IOConsoleUsers" = ({"kCGSSessionOnConsoleKey"=No,"kCGSSessionUserNameKey"="axel","kCGSSessionUserIDKey"=501},{"kCGSSessionOnConsoleKey"=Yes,"kCGSSessionUserNameKey"="axel2","CGSSessionScreenLockedTime"=1794176644,"CGSSessionScreenIsLocked"=Yes,"kCGSSessionUserIDKey"=502})
    }"#;
        assert_eq!(verrou_dans(verrouillee), Some(true));
        let ailleurs = r#"      "IOConsoleUsers" = ({"kCGSSessionOnConsoleKey"=No,"CGSSessionScreenIsLocked"=Yes},{"kCGSSessionOnConsoleKey"=Yes,"kCGSSessionUserIDKey"=502})"#;
        assert_eq!(verrou_dans(ailleurs), Some(false));
        let deverrouillee = r#"      "IOConsoleUsers" = ({"kCGSSessionOnConsoleKey"=Yes,"CGSSessionScreenIsLocked"=No})"#;
        assert_eq!(verrou_dans(deverrouillee), Some(false));
        assert_eq!(verrou_dans("rien"), None);
        assert_eq!(
            verrou_dans(r#""IOConsoleUsers" = ({"kCGSSessionOnConsoleKey"=No})"#),
            None
        );
    }

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
