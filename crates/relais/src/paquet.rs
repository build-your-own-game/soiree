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
//!   lui-même avant de compiler (§ 2.8) ; le relais ne fait que les nommer. *Au lot 10b* :
//!   le rapport d'épreuve `<empreinte>.json` qui l'accompagne (`soiree-epreuve
//!   --catalogue`, sur un code 0 du plan standard), servi à `GET /jeux/<empreinte>.json` ;
//!   le catalogue dit pour chaque module s'il est éprouvé (P17).
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

/// Un module livré, et son rapport d'épreuve s'il en a un.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Module {
    octets: Vec<u8>,
    /// `<empreinte>.json`, que `soiree-epreuve --catalogue` dépose sur un code 0 du plan
    /// standard seulement (P17, lot 10b) ; `None` : un module non éprouvé, que seul
    /// `soiree-relais jouer --non-eprouve` ajoute.
    rapport: Option<Vec<u8>>,
}

/// Les modules livrés, en mémoire, par empreinte.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Jeux {
    modules: BTreeMap<String, Module>,
}

/// Les valeurs qui suivent la clé `"cle"` dans un texte JSON (des blancs permis autour du
/// deux-points), chacune jusqu'à la fin du texte.
fn valeurs<'t>(t: &'t str, cle: &str) -> Vec<&'t str> {
    let cle = format!("\"{cle}\"");
    t.split(cle.as_str())
        .skip(1)
        .filter_map(|reste| reste.trim_start().strip_prefix(':').map(str::trim_start))
        .collect()
}

/// Un rapport d'épreuve plausible pour ce module : un objet JSON en UTF-8 qui nomme son
/// empreinte (`"empreinte": "<empreinte>"`), un code 0 (`"code": 0`) et le plan standard
/// (`"plan": {"nom": "standard", …}`), des blancs permis autour des deux-points. Le relais
/// ne lit pas le JSON (il ne tire aucune crate pour cela) : la page le lit ; il vérifie
/// seulement qu'un rapport n'est pas celui d'un autre module, ni d'un module qui n'a pas
/// passé l'épreuve (P17 : *après la vérification du lot 10b*, un code 0 du plan court ou
/// d'un plan réglé ne vaut pas épreuve, et `soiree-epreuve --catalogue` ne le dépose pas).
fn rapport_plausible(empreinte: &str, octets: &[u8]) -> bool {
    let Ok(t) = core::str::from_utf8(octets) else {
        return false;
    };
    let t = t.trim();
    let valeur = format!("\"{empreinte}\"");
    let code_0 = |v: &str| {
        v.strip_prefix('0')
            .is_some_and(|r| !r.starts_with(|c: char| c.is_ascii_digit() || c == '.'))
    };
    // L'objet du plan n'a pas d'objet dedans : il va jusqu'à sa première accolade fermante.
    let plan_standard = |v: &str| {
        v.strip_prefix('{')
            .and_then(|r| r.split_once('}'))
            .is_some_and(|(plan, _)| {
                valeurs(plan, "nom")
                    .iter()
                    .any(|n| n.starts_with("\"standard\""))
            })
    };
    t.starts_with('{')
        && t.ends_with('}')
        && valeurs(t, "empreinte")
            .iter()
            .any(|v| v.starts_with(&valeur))
        && valeurs(t, "code").iter().any(|v| code_0(v))
        && valeurs(t, "plan").iter().any(|v| plan_standard(v))
}

impl Jeux {
    /// Lit chaque `<empreinte>.wasm` d'un dossier, et le rapport `<empreinte>.json` qui
    /// l'accompagne s'il y en a un (lot 10b) ; un autre nom, un rapport sans module, ou un
    /// rapport qui ne nomme pas son module, est refusé, haut et clair, plutôt que laissé.
    pub fn lire(dossier: &Path) -> Result<Jeux, String> {
        let mut modules = BTreeMap::new();
        let mut rapports = BTreeMap::new();
        for (nom, octets) in lire_le_dossier(dossier)? {
            let (empreinte, rapport) = match nom.rsplit_once('.') {
                Some((e, "wasm")) => (e, false),
                Some((e, "json")) => (e, true),
                _ => ("", false),
            };
            if !(empreinte.len() == 64 && empreinte_permise(empreinte)) {
                return Err(format!(
                    "{} : « {nom} » n'est ni <empreinte BLAKE3 en 64 chiffres>.wasm, ni son rapport <empreinte>.json",
                    dossier.display()
                ));
            }
            if rapport {
                if !rapport_plausible(empreinte, &octets) {
                    return Err(format!(
                        "{} : « {nom} » n'est pas le rapport d'une épreuve réussie de ce module (un objet JSON qui nomme \"empreinte\":\"{empreinte}\", \"code\":0 et le plan standard, \"plan\":{{\"nom\":\"standard\",…}})",
                        dossier.display()
                    ));
                }
                rapports.insert(empreinte.to_owned(), octets);
            } else {
                modules.insert(
                    empreinte.to_owned(),
                    Module {
                        octets,
                        rapport: None,
                    },
                );
            }
        }
        for (empreinte, rapport) in rapports {
            let Some(m) = modules.get_mut(&empreinte) else {
                return Err(format!(
                    "{} : « {empreinte}.json » n'a pas son module {empreinte}.wasm",
                    dossier.display()
                ));
            };
            m.rapport = Some(rapport);
        }
        Ok(Jeux { modules })
    }

    /// Ajoute un module **non éprouvé** (`soiree-relais jouer --non-eprouve`, P17) : sans
    /// rapport, l'écran l'affiche « NON ÉPROUVÉ » en permanence. Un module déjà là, avec
    /// son rapport, le garde.
    pub fn ajouter_non_eprouve(&mut self, empreinte: &str, octets: Vec<u8>) -> Result<(), String> {
        if !(empreinte.len() == 64 && empreinte_permise(empreinte)) {
            return Err(format!(
                "« {empreinte} » n'est pas une empreinte BLAKE3 en 64 chiffres"
            ));
        }
        self.modules.entry(empreinte.to_owned()).or_insert(Module {
            octets,
            rapport: None,
        });
        Ok(())
    }

    /// Ce module a-t-il un rapport d'épreuve ?
    pub fn eprouve(&self, empreinte: &str) -> bool {
        self.modules
            .get(empreinte)
            .is_some_and(|m| m.rapport.is_some())
    }

    /// Ce que sert `/jeux/<reste>` : le catalogue, un module, ou son rapport.
    pub fn servir(&self, reste: &str) -> Option<(&'static str, Vec<u8>)> {
        if reste == "catalogue" {
            return Some(("application/json", self.catalogue().into_bytes()));
        }
        if let Some(empreinte) = reste.strip_suffix(".json") {
            return self
                .modules
                .get(empreinte)
                .and_then(|m| m.rapport.clone())
                .map(|r| ("application/json", r));
        }
        let empreinte = reste.strip_suffix(".wasm")?;
        self.modules
            .get(empreinte)
            .map(|m| ("application/wasm", m.octets.clone()))
    }

    /// Le catalogue, en JSON : `{"jeux":[{"empreinte":"…","taille":N,"eprouve":true},…]}`,
    /// par empreinte. Le rapport d'un module éprouvé se lit à `/jeux/<empreinte>.json`.
    pub fn catalogue(&self) -> String {
        let mut s = String::from("{\"jeux\":[");
        for (k, (e, m)) in self.modules.iter().enumerate() {
            if k > 0 {
                s.push(',');
            }
            let _: core::fmt::Result = write!(
                s,
                "{{\"empreinte\":\"{e}\",\"taille\":{},\"eprouve\":{}}}",
                m.octets.len(),
                m.rapport.is_some()
            );
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
