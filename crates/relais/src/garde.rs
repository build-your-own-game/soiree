//! La garde sur disque (PHASE-3, § 2.4, P9) : les cases chiffrées de chaque salle, la
//! preuve de son autorité, et le code de la salle que le relais garde (P41).
//!
//! ```text
//! <dossier>/salle                      le code de la dernière salle ouverte (GET /salle)
//! <dossier>/<SALLE>/preuve             la preuve de son autorité, 32 octets
//! <dossier>/<SALLE>/cases/<n>          la case de ce numéro : sa sorte, un octet (`p`, un
//!                                      point ; `j`, un morceau du journal), puis ses
//!                                      octets
//! ```
//!
//! **Un numéro, un fichier.** Une case qui en remplace une autre du même numéro, de
//! l'autre sorte, prend sa place par le seul renommage : il n'y a jamais d'instant où le
//! numéro n'a plus de case, ni où il en a deux (un fichier par sorte demandait d'effacer
//! l'ancienne avant que la nouvelle soit sur disque ; vérification du lot 5).
//!
//! **Écrire une case** (SAL-26, SAL-27) : dans un fichier temporaire du même dossier,
//! synchronisé par `File::sync_all` (sous macOS, `std` y fait `fcntl(F_FULLFSYNC)`), puis
//! renommé, puis le dossier ouvert et synchronisé de même ; seulement alors le relais
//! répond `Ecrit`. Un arrêt brutal laisse l'ancien fichier entier, ou le nouveau ; un
//! fichier temporaire resté se jette à la lecture. Ni `libc` ni code `unsafe`.
//!
//! Le relais ne lit rien de ce qu'il garde : les cases sont chiffrées par l'autorité, sous
//! une clé qu'il n'a pas.

use std::fs::{self, File};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use liaison::{Case, CodeSalle, RefusRelais};

use crate::coeur::{Fait, OpGarde, Preuves};

/// Ce qu'une salle garde au plus, toutes cases comprises : 256 Mio (choisi). Une garde en
/// tient au plus deux points et une centaine de cases (§ 2.4) ; les deux points de Tier
/// list au pire mesuré font 33,6 Mio (M3).
pub const GARDE_MAX: u64 = 256 * 1024 * 1024;

/// Écrit `octets` dans `dossier/nom`, durablement : fichier temporaire synchronisé,
/// renommé, dossier synchronisé (SAL-26, SAL-27).
pub fn ecrire_durable(dossier: &Path, nom: &str, octets: &[u8]) -> io::Result<()> {
    ecrire_durable_en_morceaux(dossier, nom, &[octets])
}

/// [`ecrire_durable`], le fichier fait de plusieurs morceaux à la suite : la sorte d'une
/// case, puis ses octets, sans recopier une case de 16,8 Mio pour lui ajouter un octet.
pub fn ecrire_durable_en_morceaux(dossier: &Path, nom: &str, morceaux: &[&[u8]]) -> io::Result<()> {
    let temporaire = dossier.join(format!(".{nom}.tmp"));
    {
        let mut f = File::create(&temporaire)?;
        for m in morceaux {
            f.write_all(m)?;
        }
        f.sync_all()?;
    }
    fs::rename(&temporaire, dossier.join(nom))?;
    File::open(dossier)?.sync_all()
}

/// Le nom du fichier d'un numéro de case : vingt chiffres, pour que l'ordre des noms
/// soit celui des numéros. La sorte de la case est dans le fichier, pas dans son nom.
pub fn nom_de_numero(n: u64) -> String {
    format!("{n:020}")
}

/// Le numéro d'un nom de fichier, s'il en est un.
pub fn numero_d_un_nom(nom: &str) -> Option<u64> {
    if nom.len() != 20 || !nom.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    nom.parse().ok()
}

/// L'octet qui dit la sorte d'une case, en tête de son fichier.
pub const fn octet_de_sorte(c: Case) -> u8 {
    match c {
        Case::Point(_) => b'p',
        Case::Journal(_) => b'j',
    }
}

/// La case de ce numéro, d'après l'octet de sorte de son fichier.
pub const fn case_de_sorte(n: u64, sorte: u8) -> Option<Case> {
    match sorte {
        b'p' => Some(Case::Point(n)),
        b'j' => Some(Case::Journal(n)),
        _ => None,
    }
}

/// Le numéro d'une case.
pub const fn numero(c: Case) -> u64 {
    match c {
        Case::Point(n) | Case::Journal(n) => n,
    }
}

/// Le dossier de la garde.
#[derive(Debug, Clone)]
pub struct Dossier {
    racine: PathBuf,
}

impl Dossier {
    /// Le dossier, créé s'il manque.
    pub fn ouvrir(racine: &Path) -> io::Result<Dossier> {
        fs::create_dir_all(racine)?;
        Ok(Dossier {
            racine: racine.to_path_buf(),
        })
    }

    /// Sa racine.
    pub fn racine(&self) -> &Path {
        &self.racine
    }

    fn salle_dir(&self, code: CodeSalle) -> PathBuf {
        self.racine.join(code.to_string())
    }

    fn cases_dir(&self, code: CodeSalle) -> PathBuf {
        self.salle_dir(code).join("cases")
    }

    /// La preuve enregistrée d'une salle.
    pub fn preuve(&self, code: CodeSalle) -> Option<[u8; 32]> {
        let o = fs::read(self.salle_dir(code).join("preuve")).ok()?;
        <[u8; 32]>::try_from(o.as_slice()).ok()
    }

    /// Enregistre la preuve d'une salle, et retient son code comme celui de la salle du
    /// relais (P41).
    pub fn enregistrer_preuve(&self, code: CodeSalle, preuve: [u8; 32]) -> io::Result<()> {
        let s = self.salle_dir(code);
        fs::create_dir_all(s.join("cases"))?;
        ecrire_durable(&s, "preuve", &preuve)?;
        ecrire_durable(&self.racine, "salle", code.to_string().as_bytes())
    }

    /// Le code de la salle que le relais garde (`GET /salle`, P41).
    pub fn salle(&self) -> Option<CodeSalle> {
        let o = fs::read(self.racine.join("salle")).ok()?;
        CodeSalle::lire(core::str::from_utf8(&o).ok()?.trim())
    }

    /// `soiree-relais oublier <salle>` : efface la preuve et la garde de cette salle.
    /// Le code reste celui du relais : l'autorité suivante enregistre sa preuve sous le
    /// même code (P41). Rend `false` si la salle n'avait rien.
    pub fn oublier(&self, code: CodeSalle) -> io::Result<bool> {
        let s = self.salle_dir(code);
        if !s.exists() {
            return Ok(false);
        }
        fs::remove_dir_all(&s)?;
        File::open(&self.racine)?.sync_all()?;
        Ok(true)
    }

    fn fichier(&self, code: CodeSalle, n: u64) -> PathBuf {
        self.cases_dir(code).join(nom_de_numero(n))
    }

    /// Les cases d'une salle, par numéro : la sorte de chacune se lit dans le premier
    /// octet de son fichier. Un fichier temporaire resté d'un arrêt brutal est jeté ; un
    /// fichier sans sorte lisible n'est pas une case (aucune écriture du relais n'en
    /// laisse : le renommage pose un fichier entier).
    pub fn lister(&self, code: CodeSalle) -> io::Result<Vec<Case>> {
        let d = self.cases_dir(code);
        let mut cases = Vec::new();
        let lecture = match fs::read_dir(&d) {
            Ok(l) => l,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(cases),
            Err(e) => return Err(e),
        };
        for e in lecture {
            let e = e?;
            let nom = e.file_name();
            let Some(nom) = nom.to_str() else {
                continue;
            };
            if let Some(n) = numero_d_un_nom(nom) {
                let mut sorte = [0u8; 1];
                match File::open(e.path()).and_then(|mut f| f.read(&mut sorte)) {
                    Ok(1) => {
                        let [s] = sorte;
                        if let Some(c) = case_de_sorte(n, s) {
                            cases.push(c);
                        }
                    }
                    // Effacé entre la liste et la lecture (`oublier`), ou vide.
                    Ok(_) => {}
                    Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                    Err(err) => return Err(err),
                }
            } else if nom.starts_with('.') && Path::new(nom).extension().is_some_and(|x| x == "tmp")
            {
                let _: io::Result<()> = fs::remove_file(e.path());
            }
        }
        cases.sort_by_key(|c| numero(*c));
        Ok(cases)
    }

    /// Les octets d'une case : rien si son numéro n'a pas de case, ou une case de
    /// l'autre sorte.
    pub fn lire(&self, code: CodeSalle, case: Case) -> io::Result<Option<Vec<u8>>> {
        let mut o = match fs::read(self.fichier(code, numero(case))) {
            Ok(o) => o,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        if o.first() != Some(&octet_de_sorte(case)) {
            return Ok(None);
        }
        let _: std::vec::Drain<'_, u8> = o.drain(..1);
        Ok(Some(o))
    }

    /// Les octets que tient la case de ce numéro, quelle que soit sa sorte, lus dans les
    /// métadonnées de son fichier (sans le lire) ; 0 s'il n'y en a pas.
    fn octets_du_numero(&self, code: CodeSalle, n: u64) -> io::Result<u64> {
        match fs::metadata(self.fichier(code, n)) {
            Ok(m) => Ok(m.len().saturating_sub(1)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(0),
            Err(e) => Err(e),
        }
    }

    /// Les octets gardés par une salle (l'octet de sorte de chaque case non compris).
    pub fn taille(&self, code: CodeSalle) -> io::Result<u64> {
        let mut total = 0u64;
        for c in self.lister(code)? {
            total = total.saturating_add(self.octets_du_numero(code, numero(c))?);
        }
        Ok(total)
    }

    /// Écrit une case, durablement. Elle remplace la case de son numéro, quelle que soit
    /// sa sorte, par le seul renommage : un numéro nomme une case, à chaque instant.
    pub fn ecrire(&self, code: CodeSalle, case: Case, octets: &[u8]) -> io::Result<()> {
        let d = self.cases_dir(code);
        fs::create_dir_all(&d)?;
        ecrire_durable_en_morceaux(
            &d,
            &nom_de_numero(numero(case)),
            &[&[octet_de_sorte(case)], octets],
        )
    }

    /// Efface les cases d'avant `avant`.
    pub fn effacer(&self, code: CodeSalle, avant: u64) -> io::Result<()> {
        let d = self.cases_dir(code);
        for c in self.lister(code)? {
            if numero(c) < avant {
                fs::remove_file(self.fichier(code, numero(c)))?;
            }
        }
        if d.exists() {
            File::open(&d)?.sync_all()?;
        }
        Ok(())
    }

    /// Fait une opération de la garde, et dit ce qui en est. Une écriture au-delà de
    /// [`GARDE_MAX`] est refusée (`GardePleine`), comme une écriture que le disque refuse :
    /// une file pleine se dit, elle ne jette rien en silence (SAL-07).
    pub fn faire(&self, code: CodeSalle, op: &OpGarde) -> Fait {
        self.faire_sous(code, op, GARDE_MAX)
    }

    /// [`Dossier::faire`], sous une autre borne que [`GARDE_MAX`] : pour les tests.
    pub fn faire_sous(&self, code: CodeSalle, op: &OpGarde, borne: u64) -> Fait {
        match op {
            OpGarde::Ecrire { case, octets } => {
                let deja = self.taille(code).unwrap_or(u64::MAX);
                // La case qu'elle remplace, de l'une ou l'autre sorte : sa taille seule,
                // sans relire ses octets (jusqu'à 16,8 Mio).
                let ancienne = self.octets_du_numero(code, numero(*case)).unwrap_or(0);
                let apres = deja
                    .saturating_sub(ancienne)
                    .saturating_add(octets.len() as u64);
                if apres > borne {
                    return Fait::Refus(RefusRelais::GardePleine);
                }
                match self.ecrire(code, *case, octets) {
                    Ok(()) => Fait::Ecrit(*case),
                    Err(_) => Fait::Refus(RefusRelais::GardePleine),
                }
            }
            OpGarde::Lister => match self.lister(code) {
                Ok(c) => Fait::Cases(c),
                Err(_) => Fait::Refus(RefusRelais::GardePleine),
            },
            OpGarde::Lire { case } => match self.lire(code, *case) {
                Ok(Some(o)) => Fait::Lu(*case, o),
                Ok(None) => Fait::Absente(*case),
                Err(_) => Fait::Refus(RefusRelais::GardePleine),
            },
            OpGarde::Effacer { avant } => match self.effacer(code, *avant) {
                Ok(()) => Fait::Efface,
                Err(_) => Fait::Refus(RefusRelais::GardePleine),
            },
        }
    }
}

/// Les preuves d'autorité, lues sur disque à chaque bonjour d'autorité : ce que
/// `soiree-relais oublier` efface, le relais qui tourne le voit (P41).
#[derive(Debug, Clone)]
pub struct PreuvesSurDisque(pub Dossier);

impl Preuves for PreuvesSurDisque {
    fn preuve(&mut self, salle: CodeSalle) -> Option<[u8; 32]> {
        self.0.preuve(salle)
    }

    fn enregistrer(&mut self, salle: CodeSalle, preuve: [u8; 32]) -> bool {
        self.0.enregistrer_preuve(salle, preuve).is_ok()
    }
}
