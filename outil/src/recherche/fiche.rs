//! The research files as types: `sous-systemes.toml`, `sources.toml` and the
//! sheets. Nothing here is a free string that a rule compares by hand: a kind,
//! a state, a link or a guarantee that is not one of the known forms stops at
//! the reading, with its line.

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::util::Result;

/// A TOML file into `T`, or one line saying where and why it cannot be read.
pub fn read<T: DeserializeOwned>(text: &str, file: &str) -> Result<T> {
    toml::from_str(text).map_err(|error: toml::de::Error| {
        let line = error.span().map_or(0, |span| {
            text.get(..span.start)
                .map_or(0, |before| before.matches('\n').count())
                .saturating_add(1)
        });
        format!("{file}:{line} : {}", error.message().trim())
    })
}

/// A step of the ladder, in reading order.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Sorte {
    ChezNous,
    Reel,
    Normes,
    Bancs,
    Retours,
}

impl fmt::Display for Sorte {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ChezNous => "chez-nous",
            Self::Reel => "reel",
            Self::Normes => "normes",
            Self::Bancs => "bancs",
            Self::Retours => "retours",
        })
    }
}

/// What a source's code is, when its side matters. A norm or a postmortem has
/// no side: its `sorte` says enough.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Nature {
    CodeClient,
    CodeServeur,
}

/// The four origins of the manifesto's table.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Categorie {
    Physique,
    UsageReel,
    Accident,
    Heritage,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Decision {
    Garder,
    Jeter,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Etat {
    Couvert,
    Trou,
    NonApplicable,
}

/// In the order of the debt list: the worst first.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Gravite {
    Haute,
    Moyenne,
    Faible,
}

impl fmt::Display for Gravite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Haute => "haute",
            Self::Moyenne => "moyenne",
            Self::Faible => "faible",
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Perimetre {
    Commun,
    Jeu,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EtatSource {
    Candidate,
    Consultee,
}

/// What counts as new under a triggering path.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Nouveau {
    Fichier,
    ApiPublique,
    Dependance,
    Delai,
    MessageDeProtocole,
}

impl fmt::Display for Nouveau {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Fichier => "fichier",
            Self::ApiPublique => "api-publique",
            Self::Dependance => "dependance",
            Self::Delai => "delai",
            Self::MessageDeProtocole => "message-de-protocole",
        })
    }
}

fn is_hex(text: &str, length: usize) -> bool {
    text.len() == length
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// A full commit hash: a short one stops meaning anything as a repository grows.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(try_from = "String")]
pub struct Commit(String);

impl TryFrom<String> for Commit {
    type Error = String;

    fn try_from(text: String) -> Result<Self> {
        if is_hex(&text, 40) {
            Ok(Self(text))
        } else {
            Err(format!(
                "commit « {text} » : attendu 40 caractères hexadécimaux"
            ))
        }
    }
}

impl fmt::Display for Commit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `owner/name` on GitHub.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(try_from = "String")]
pub struct Depot(String);

impl TryFrom<String> for Depot {
    type Error = String;

    fn try_from(text: String) -> Result<Self> {
        let named = |part: &str| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        };
        match text.split_once('/') {
            Some((owner, name)) if named(owner) && named(name) => Ok(Self(text)),
            _ => Err(format!("depot « {text} » : attendu propriétaire/nom")),
        }
    }
}

impl fmt::Display for Depot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `AAAA-MM-JJ`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(try_from = "String")]
pub struct Date(String);

impl TryFrom<String> for Date {
    type Error = String;

    fn try_from(text: String) -> Result<Self> {
        let mut parts = text.split('-');
        let number = |part: Option<&str>, width: usize, top: u32| {
            part.filter(|p| p.len() == width && p.chars().all(|c| c.is_ascii_digit()))
                .and_then(|p| p.parse::<u32>().ok())
                .is_some_and(|n| (1..=top).contains(&n))
        };
        let valid = number(parts.next(), 4, 9_999)
            && number(parts.next(), 2, 12)
            && number(parts.next(), 2, 31)
            && parts.next().is_none();
        if valid {
            Ok(Self(text))
        } else {
            Err(format!("date « {text} » : attendu AAAA-MM-JJ"))
        }
    }
}

/// `sha256:<64 hex>`, the identity of a document that has no commit.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(try_from = "String")]
pub struct Empreinte(String);

impl TryFrom<String> for Empreinte {
    type Error = String;

    fn try_from(text: String) -> Result<Self> {
        if text.strip_prefix("sha256:").is_some_and(|h| is_hex(h, 64)) {
            Ok(Self(text))
        } else {
            Err(format!(
                "empreinte « {text} » : attendu sha256: et 64 caractères hexadécimaux"
            ))
        }
    }
}

impl Empreinte {
    pub fn hex(&self) -> &str {
        self.0.strip_prefix("sha256:").unwrap_or(&self.0)
    }
}

/// `chemin:ligne`: where a hole lives.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(try_from = "String")]
pub struct Lieu {
    pub chemin: String,
    pub ligne: u32,
}

impl TryFrom<String> for Lieu {
    type Error = String;

    fn try_from(text: String) -> Result<Self> {
        text.rsplit_once(':')
            .and_then(|(chemin, ligne)| {
                let ligne = ligne.parse::<u32>().ok().filter(|n| *n > 0)?;
                is_path(chemin).then(|| Self {
                    chemin: chemin.to_owned(),
                    ligne,
                })
            })
            .ok_or_else(|| format!("lieu « {text} » : attendu chemin:ligne"))
    }
}

impl fmt::Display for Lieu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.chemin, self.ligne)
    }
}

fn is_path(text: &str) -> bool {
    !text.is_empty() && !text.contains(char::is_whitespace) && text.contains(['/', '.'])
}

/// `L12`.
fn line_number(text: &str) -> Option<u32> {
    text.strip_prefix('L')?.parse().ok().filter(|n| *n > 0)
}

const LINK_FORMS: &str = "attendu https://github.com/<dépôt>/blob/<sha>/chemin#L1(-L2), …/commit/<sha>, …/issues/N, un document .txt#L1, ou une page web";

/// Where a lesson was read. Every form but a web page is frozen: it says the
/// same thing tomorrow.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(try_from = "String")]
pub enum Lien {
    Blob {
        depot: Depot,
        sha: Commit,
        chemin: String,
        debut: u32,
        fin: Option<u32>,
    },
    Commit {
        depot: Depot,
        sha: Commit,
    },
    Issue {
        depot: Depot,
        numero: u32,
    },
    Texte {
        document: String,
        ligne: u32,
    },
    Page {
        url: String,
    },
}

impl TryFrom<String> for Lien {
    type Error = String;

    fn try_from(text: String) -> Result<Self> {
        let refused = || format!("lien « {text} » : {LINK_FORMS}");
        let rest = text.strip_prefix("https://").ok_or_else(refused)?;
        if let Some(github) = rest.strip_prefix("github.com/") {
            return github_link(github).ok_or_else(refused);
        }
        let (document, anchor) = rest
            .split_once('#')
            .map_or((rest, None), |(document, anchor)| (document, Some(anchor)));
        let text_file = std::path::Path::new(document)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"));
        if text_file {
            let ligne = anchor.and_then(line_number).ok_or_else(refused)?;
            return Ok(Self::Texte {
                document: format!("https://{document}"),
                ligne,
            });
        }
        Ok(Self::Page { url: text })
    }
}

fn github_link(rest: &str) -> Option<Lien> {
    let mut parts = rest.splitn(4, '/');
    let (owner, name, kind, tail) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    let depot = Depot::try_from(format!("{owner}/{name}")).ok()?;
    match kind {
        "blob" => {
            let (sha, path) = tail.split_once('/')?;
            let (chemin, anchor) = path.split_once('#')?;
            let (first, last) = anchor
                .split_once('-')
                .map_or((anchor, None), |(first, last)| (first, Some(last)));
            let debut = line_number(first)?;
            let fin = match last {
                Some(last) => Some(line_number(last).filter(|last| *last >= debut)?),
                None => None,
            };
            Some(Lien::Blob {
                depot,
                sha: Commit::try_from(sha.to_owned()).ok()?,
                chemin: chemin.to_owned(),
                debut,
                fin,
            })
        }
        "commit" => Some(Lien::Commit {
            depot,
            sha: Commit::try_from(tail.to_owned()).ok()?,
        }),
        "issues" => Some(Lien::Issue {
            depot,
            numero: tail.parse().ok().filter(|n| *n > 0)?,
        }),
        _ => None,
    }
}

impl Lien {
    /// What two excerpts must not share: one excerpt per document.
    pub fn document(&self) -> String {
        match self {
            Self::Blob {
                depot, sha, chemin, ..
            } => format!("{depot}@{sha}/{chemin}"),
            Self::Commit { depot, sha } => format!("{depot}@{sha}"),
            Self::Issue { depot, numero } => format!("{depot}#{numero}"),
            Self::Texte { document, .. } => document.clone(),
            Self::Page { url } => url.clone(),
        }
    }

    pub fn depot(&self) -> Option<&Depot> {
        match self {
            Self::Blob { depot, .. } | Self::Commit { depot, .. } | Self::Issue { depot, .. } => {
                Some(depot)
            }
            Self::Texte { .. } | Self::Page { .. } => None,
        }
    }

    /// A page or an issue can change after reading: its reference dates it.
    pub fn mutable(&self) -> bool {
        match self {
            Self::Issue { .. } | Self::Page { .. } => true,
            Self::Blob { .. } | Self::Commit { .. } | Self::Texte { .. } => false,
        }
    }
}

const GUARANTEE_FORMS: &str =
    "attendu chemin::fonction pour un test, C2 pour un item, chemin#L1 pour du code, ou un chemin";

/// What covers a lesson.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(try_from = "String")]
pub enum Garantie {
    Test { chemin: String, nom: String },
    Item(String),
    Code { chemin: String, ligne: u32 },
    Chemin(String),
}

impl TryFrom<String> for Garantie {
    type Error = String;

    fn try_from(text: String) -> Result<Self> {
        if is_item(&text) {
            return Ok(Self::Item(text));
        }
        if let Some((chemin, nom)) = text.rsplit_once("::") {
            let named = !nom.is_empty() && nom.chars().all(|c| c.is_alphanumeric() || c == '_');
            if is_path(chemin) && named {
                return Ok(Self::Test {
                    chemin: chemin.to_owned(),
                    nom: nom.to_owned(),
                });
            }
        } else if let Some((chemin, anchor)) = text.rsplit_once('#') {
            if let Some(ligne) = line_number(anchor).filter(|_| is_path(chemin)) {
                return Ok(Self::Code {
                    chemin: chemin.to_owned(),
                    ligne,
                });
            }
        } else if is_path(&text) {
            return Ok(Self::Chemin(text));
        }
        Err(format!("garantie « {text} » : {GUARANTEE_FORMS}"))
    }
}

impl fmt::Display for Garantie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Test { chemin, nom } => write!(f, "{chemin}::{nom}"),
            Self::Item(id) | Self::Chemin(id) => f.write_str(id),
            Self::Code { chemin, ligne } => write!(f, "{chemin}#L{ligne}"),
        }
    }
}

/// An item of `CERTIFICATION.md`: a capital and digits.
pub fn is_item(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && text.len() >= 2
        && chars.all(|c| c.is_ascii_digit())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub id: String,
    pub ordre: u32,
    pub sorte: Sorte,
    pub consulte: Vec<String>,
    pub depot: Option<Depot>,
    pub commit: Option<Commit>,
    pub document: Option<String>,
    pub vu_le: Option<Date>,
    pub empreinte: Option<Empreinte>,
    pub nature: Option<Nature>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lecon {
    pub id: String,
    pub source: String,
    pub lien: Lien,
    pub citation: String,
    pub enonce: String,
    #[expect(
        dead_code,
        reason = "lu pour refuser une catégorie inconnue ; aucune règle ne l'exige encore"
    )]
    pub categorie: Categorie,
    pub decision: Decision,
    pub etat: Etat,
    #[serde(default)]
    pub garantie: Vec<Garantie>,
    pub trou: Option<String>,
    pub raison: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trou {
    pub id: String,
    pub titre: String,
    pub gravite: Gravite,
    pub lieux: Vec<Lieu>,
    pub cas: String,
    pub preuve: String,
    pub verifie: String,
    pub suppose: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saturation {
    pub sources: Vec<String>,
    pub derniere_lecon: String,
    pub note: String,
    pub recoupe: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cout {
    pub minutes: u32,
    pub references: usize,
    #[expect(
        dead_code,
        reason = "lu pour refuser un format faux ; une sonde ne se compte pas depuis la fiche"
    )]
    pub sondes_executees: u32,
    pub note: String,
}

/// One research sheet, `recherche/<sous_systeme>.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fiche {
    pub sous_systeme: String,
    pub niveau: u8,
    /// Why this sheet stands below the floor, like a `#[expect]`'s reason.
    pub raison_niveau: Option<String>,
    pub question: String,
    pub compare_a: Commit,
    #[serde(rename = "reference", default)]
    pub references: Vec<Reference>,
    #[serde(rename = "lecon", default)]
    pub lecons: Vec<Lecon>,
    #[serde(rename = "trou", default)]
    pub trous: Vec<Trou>,
    pub saturation: Saturation,
    pub cout: Cout,
}

impl Fiche {
    /// A hole stays open while one of its lessons still says `trou`. A fix
    /// turns the lesson to `couvert`, names the guarantee and keeps `trou`.
    pub fn open(&self) -> impl Iterator<Item = &Trou> {
        self.trous.iter().filter(|trou| {
            self.lecons
                .iter()
                .any(|l| l.etat == Etat::Trou && l.trou.as_deref() == Some(trou.id.as_str()))
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Niveau {
    pub numero: u8,
    pub intention: String,
    pub references: usize,
    pub saturation: usize,
    pub quota: BTreeMap<Sorte, usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SousSysteme {
    pub id: String,
    pub perimetre: Perimetre,
    pub niveau: u8,
    pub marches: Vec<Sorte>,
    pub chemins: Vec<String>,
}

impl SousSysteme {
    pub fn triggered_by(&self, path: &str) -> bool {
        self.chemins.iter().any(|motif| glob(motif, path))
    }
}

/// `sous-systemes.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registre {
    pub marches: Vec<Sorte>,
    pub nouveau: Vec<Nouveau>,
    #[serde(rename = "niveau")]
    pub niveaux: Vec<Niveau>,
    #[serde(rename = "sous_systeme")]
    pub sous_systemes: Vec<SousSysteme>,
}

impl Registre {
    pub fn sous_systeme(&self, id: &str) -> Option<&SousSysteme> {
        self.sous_systemes.iter().find(|s| s.id == id)
    }

    pub fn niveau(&self, numero: u8) -> Option<&Niveau> {
        self.niveaux.iter().find(|n| n.numero == numero)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub sous_systeme: String,
    pub ids: Vec<String>,
    pub marche: Sorte,
    pub etat: EtatSource,
    pub pourquoi: Option<String>,
}

/// `sources.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sources {
    #[serde(rename = "source")]
    pub sources: Vec<Source>,
}

/// `*` within a segment, `**` across any number of them.
pub fn glob(motif: &str, path: &str) -> bool {
    let motif: Vec<&str> = motif.split('/').collect();
    let path: Vec<&str> = path.split('/').collect();
    segments(&motif, &path)
}

fn segments(motif: &[&str], path: &[&str]) -> bool {
    match (motif.split_first(), path.split_first()) {
        (None, None) => true,
        (Some((&"**", rest)), _) => {
            segments(rest, path)
                || path
                    .split_first()
                    .is_some_and(|(_, on)| segments(motif, on))
        }
        (Some((first, rest)), Some((name, on))) => segment(first, name) && segments(rest, on),
        (Some(_), None) | (None, Some(_)) => false,
    }
}

fn segment(motif: &str, name: &str) -> bool {
    let Some((before, after)) = motif.split_once('*') else {
        return motif == name;
    };
    let Some(rest) = name.strip_prefix(before) else {
        return false;
    };
    rest.char_indices()
        .map(|(at, _)| at)
        .chain(std::iter::once(rest.len()))
        .any(|at| rest.get(at..).is_some_and(|tail| segment(after, tail)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs_match_like_the_registry_expects() {
        assert!(glob(
            "salon/crates/core/src/client.rs",
            "salon/crates/core/src/client.rs"
        ));
        assert!(!glob(
            "salon/crates/core/src/client.rs",
            "salon/crates/core/src/host.rs"
        ));
        assert!(glob(
            "salon/crates/crypto/**",
            "salon/crates/crypto/src/a/b.rs"
        ));
        assert!(glob(
            "crates/*/src/physics*",
            "crates/core/src/physics_world.rs"
        ));
        assert!(!glob(
            "crates/*/src/physics*",
            "crates/core/src/a/physics.rs"
        ));
        assert!(glob(
            "crates/*/src/**/physics*",
            "crates/core/src/a/b/physics.rs"
        ));
        assert!(glob(
            "crates/*/src/**/physics*",
            "crates/core/src/physics.rs"
        ));
        assert!(!glob(
            "crates/*/src/**/physics*",
            "crates/core/src/metaphysics.rs"
        ));
    }

    #[test]
    fn each_link_form_is_read_and_a_moving_link_is_refused() {
        let link = |text: &str| Lien::try_from(text.to_owned());
        let sha = "2f01c3c88f361aeb9ebbf2cc9b247ee230dd815e";
        assert!(matches!(
            link(&format!("https://github.com/a/b/blob/{sha}/x/y.rs#L3-L9")),
            Ok(Lien::Blob {
                debut: 3,
                fin: Some(9),
                ..
            })
        ));
        assert!(matches!(
            link(&format!("https://github.com/a/b/commit/{sha}")),
            Ok(Lien::Commit { .. })
        ));
        assert!(matches!(
            link("https://github.com/a/b/issues/12"),
            Ok(Lien::Issue { numero: 12, .. })
        ));
        assert!(matches!(
            link("https://www.rfc-editor.org/rfc/rfc6455.txt#L2453"),
            Ok(Lien::Texte { ligne: 2453, .. })
        ));
        assert!(matches!(
            link("https://slack.engineering/x/"),
            Ok(Lien::Page { .. })
        ));
        for refused in [
            "http://slack.engineering/x/",
            "https://github.com/a/b/blob/main/x.rs#L1",
            "https://github.com/a/b/blob/2f01c3c/x.rs#L1",
            "https://github.com/a/b",
            "https://github.com/a/b/pull/3",
            "https://github.com/a/b/issues/x",
            "https://www.rfc-editor.org/rfc/rfc6455.txt",
        ] {
            assert!(link(refused).is_err(), "{refused}");
        }
        let unanchored = format!("https://github.com/a/b/blob/{sha}/x.rs");
        let reversed = format!("https://github.com/a/b/blob/{sha}/x.rs#L9-L3");
        assert!(link(&unanchored).is_err());
        assert!(link(&reversed).is_err());
    }

    #[test]
    fn each_guarantee_form_is_read() {
        let guarantee = |text: &str| Garantie::try_from(text.to_owned());
        assert!(matches!(
            guarantee("salon/tests/a.rs::un_test"),
            Ok(Garantie::Test { .. })
        ));
        assert!(matches!(guarantee("C10"), Ok(Garantie::Item(_))));
        assert!(matches!(
            guarantee("salon/src/lib.rs#L385"),
            Ok(Garantie::Code { ligne: 385, .. })
        ));
        assert!(matches!(
            guarantee("tools/cert/a.mjs"),
            Ok(Garantie::Chemin(_))
        ));
        for refused in ["", "couvert", "a.rs::", "a.rs#385", "un test"] {
            assert!(guarantee(refused).is_err(), "{refused}");
        }
    }

    #[test]
    fn dates_hashes_and_places_are_refused_when_malformed() {
        assert!(Date::try_from("2026-11-02".to_owned()).is_ok());
        assert!(Date::try_from("02/11/2026".to_owned()).is_err());
        assert!(Date::try_from("2026-13-02".to_owned()).is_err());
        assert!(Commit::try_from("ABC".to_owned()).is_err());
        assert!(Empreinte::try_from("sha256:abc".to_owned()).is_err());
        assert!(Lieu::try_from("salon/a.rs:12".to_owned()).is_ok());
        assert!(Lieu::try_from("salon/a.rs".to_owned()).is_err());
        assert!(Lieu::try_from("salon/a.rs:0".to_owned()).is_err());
    }

    #[test]
    fn a_reading_error_says_its_line() {
        let error = read::<Sources>("\n\n[[source]]\nsous_systeme = 3\n", "s.toml").unwrap_err();
        assert!(error.starts_with("s.toml:4 : "), "{error}");
    }
}
