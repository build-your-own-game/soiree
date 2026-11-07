//! Le paquet de la page et les modules livrés (PHASE-3, § 2.5, § 2.8, P16 ; lot 10a) :
//! lus une fois, au lancement, puis servis d'une table en mémoire, nommée par empreinte ;
//! rien d'autre ne se lit du disque pour être servi (SAL-150, SAL-41).
//!
//! - **Le paquet** (`GET /web/<empreinte>/<fichier>`, tous) : la colle `wasm-bindgen` et
//!   les `.wasm` de l'écran et du Worker, et `demarrage.js`. Son empreinte est le nom de
//!   son dossier, que l'outil qui le construit écrit (`<cible>/soiree-web/<empreinte>/`) :
//!   le relais ne hache rien, il ne tire aucune crate de hachage ni de chiffrement
//!   (`tests/relais/graphe.rs`). Un fichier se sert `immutable` : son chemin porte
//!   l'empreinte du paquet.
//! - **Les modules livrés** (`GET /jeux/<empreinte>.wasm`, `GET /jeux/catalogue`, boucle
//!   locale) : chaque fichier `<empreinte>.wasm` d'un dossier, l'empreinte étant le
//!   BLAKE3 du module en 64 chiffres hexadécimaux minuscules, que le Worker vérifie
//!   lui-même avant de compiler (§ 2.8) ; le relais ne fait que les nommer.
//! - **La page de démarrage** (`GET /`, `GET /s/<salle>`) : quelques lignes de HTML qui
//!   chargent `demarrage.js` du paquet, sans script en ligne (la politique de contenu
//!   n'en permet pas, ni de style en ligne : la page se met en forme par le CSSOM). Avec `--banc`, elle le dit à la page (`<meta name="soiree-banc">`),
//!   qui le dit au Worker (le mode banc, § 8.1) ; jamais en soirée.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use crate::http::Banc;

/// Le plus grand fichier lu dans un paquet ou un catalogue : 64 Mio (choisi : les `.wasm`
/// du paquet font quelques Mio ; un module livré, au plus 4 Mio, `TAILLE_MODULE_MAX`).
pub const FICHIER_MAX: u64 = 64 * 1024 * 1024;

/// Un nom de fichier servi : lettres, chiffres, `.`, `_`, `-`, sans `..` ni séparateur.
fn nom_permis(nom: &str) -> bool {
    !nom.is_empty()
        && nom.len() <= 128
        && !nom.starts_with('.')
        && nom
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// Une empreinte en chiffres hexadécimaux minuscules, de 16 à 64 chiffres.
fn empreinte_permise(e: &str) -> bool {
    (16..=64).contains(&e.len())
        && e.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Les fichiers ordinaires d'un dossier, au premier niveau, par nom.
fn lire_le_dossier(dossier: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut fichiers = BTreeMap::new();
    let entrees = std::fs::read_dir(dossier).map_err(|e| format!("{} : {e}", dossier.display()))?;
    for entree in entrees {
        let entree = entree.map_err(|e| format!("{} : {e}", dossier.display()))?;
        let chemin = entree.path();
        let meta = std::fs::symlink_metadata(&chemin)
            .map_err(|e| format!("{} : {e}", chemin.display()))?;
        if !meta.is_file() {
            continue;
        }
        let Some(nom) = entree.file_name().to_str().map(str::to_owned) else {
            return Err(format!("{} : un nom illisible", chemin.display()));
        };
        if !nom_permis(&nom) {
            return Err(format!(
                "{} : un nom que le relais ne sert pas",
                chemin.display()
            ));
        }
        if meta.len() > FICHIER_MAX {
            return Err(format!(
                "{} : plus de {FICHIER_MAX} octets",
                chemin.display()
            ));
        }
        let octets = std::fs::read(&chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
        fichiers.insert(nom, octets);
    }
    Ok(fichiers)
}

/// Le paquet de la page, en mémoire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paquet {
    empreinte: String,
    fichiers: BTreeMap<String, Vec<u8>>,
}

impl Paquet {
    /// Lit le paquet d'un dossier `<…>/<empreinte>/` : son nom est son empreinte, et il
    /// doit tenir `demarrage.js`.
    pub fn lire(dossier: &Path) -> Result<Paquet, String> {
        let empreinte = dossier
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|n| empreinte_permise(n))
            .ok_or_else(|| {
                format!(
                    "{} : le dossier d'un paquet est nommé par son empreinte (16 à 64 chiffres hexadécimaux minuscules)",
                    dossier.display()
                )
            })?
            .to_owned();
        let fichiers = lire_le_dossier(dossier)?;
        if !fichiers.contains_key("demarrage.js") {
            return Err(format!("{} : pas de demarrage.js", dossier.display()));
        }
        Ok(Paquet {
            empreinte,
            fichiers,
        })
    }

    /// Son empreinte.
    pub fn empreinte(&self) -> &str {
        &self.empreinte
    }

    /// Le fichier de `/web/<reste>`, s'il est dans la table : `reste` est
    /// `<empreinte>/<nom>`, comparé à des noms, jamais au disque.
    pub fn fichier(&self, reste: &str) -> Option<(&'static str, &[u8])> {
        let (empreinte, nom) = reste.split_once('/')?;
        if empreinte != self.empreinte {
            return None;
        }
        let octets = self.fichiers.get(nom)?;
        Some((type_de(nom), octets))
    }
}

/// Le type d'un fichier servi, par son extension.
pub fn type_de(nom: &str) -> &'static str {
    match nom.rsplit_once('.').map(|(_, e)| e) {
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("html") => "text/html; charset=utf-8",
        Some("json") => "application/json",
        Some("ttf") => "font/ttf",
        Some(_) | None => "application/octet-stream",
    }
}

/// Les modules livrés, en mémoire, par empreinte.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Jeux {
    modules: BTreeMap<String, Vec<u8>>,
}

impl Jeux {
    /// Lit chaque `<empreinte>.wasm` d'un dossier ; un autre nom est refusé, haut et
    /// clair, plutôt que laissé.
    pub fn lire(dossier: &Path) -> Result<Jeux, String> {
        let mut modules = BTreeMap::new();
        for (nom, octets) in lire_le_dossier(dossier)? {
            let Some(empreinte) = nom
                .strip_suffix(".wasm")
                .filter(|e| e.len() == 64 && empreinte_permise(e))
            else {
                return Err(format!(
                    "{} : « {nom} » n'est pas <empreinte BLAKE3 en 64 chiffres>.wasm",
                    dossier.display()
                ));
            };
            modules.insert(empreinte.to_owned(), octets);
        }
        Ok(Jeux { modules })
    }

    /// Ce que sert `/jeux/<reste>` : le catalogue, ou un module.
    pub fn servir(&self, reste: &str) -> Option<(&'static str, Vec<u8>)> {
        if reste == "catalogue" {
            return Some(("application/json", self.catalogue().into_bytes()));
        }
        let empreinte = reste.strip_suffix(".wasm")?;
        self.modules
            .get(empreinte)
            .map(|o| ("application/wasm", o.clone()))
    }

    /// Le catalogue, en JSON : `{"jeux":[{"empreinte":"…","taille":N},…]}`, par empreinte.
    pub fn catalogue(&self) -> String {
        let mut s = String::from("{\"jeux\":[");
        for (k, (e, o)) in self.modules.iter().enumerate() {
            if k > 0 {
                s.push(',');
            }
            let _: core::fmt::Result =
                write!(s, "{{\"empreinte\":\"{e}\",\"taille\":{}}}", o.len());
        }
        s.push_str("]}");
        s
    }
}

/// La page de démarrage : elle charge `demarrage.js` du paquet. Avec le banc, elle le dit.
pub fn page_de_demarrage(paquet: &Paquet, banc: Banc) -> Vec<u8> {
    let meta = match banc {
        Banc::Oui => "<meta name=\"soiree-banc\" content=\"oui\">",
        Banc::Non => "",
    };
    format!(
        "<!doctype html><html lang=\"fr\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">{meta}<title>Soirée</title><body><script type=\"module\" src=\"/web/{}/demarrage.js\"></script></body></html>\n",
        paquet.empreinte()
    )
    .into_bytes()
}
