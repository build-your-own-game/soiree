//! The rules of a sheet, offline. A rule that fails says what it found and
//! what it expected; nothing here touches the network (`verifie.rs` does).
//!
//! Whether a cited test, item or file exists goes through [`World`]: the rules
//! stay pure, and the negative cases can break one rule at a time.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::fiche::{
    Decision, Etat, EtatSource, Fiche, Garantie, Lecon, Lieu, Nature, Niveau, Reference, Registre,
    Sorte, Sources, SousSysteme,
};
use crate::util::{self, Result};

/// A citation stays an excerpt: fewer words than this.
const CITATION_WORDS: usize = 15;

/// Where a subsystem's client lives, in the common repository and in a game.
/// `sous-systemes.toml` does not say it yet, so its paths do.
const CLIENT_CODE: [&str; 6] = [
    "salon/crates/core/src/client.rs",
    "salon/crates/web/",
    "remote/",
    "crates/controller/",
    "crates/web/",
    "web/",
];

/// What the sheets cite outside themselves.
pub trait World {
    fn guarantee(&self, cited: &Garantie) -> Result<()>;
    fn place(&self, cited: &Lieu) -> Result<()>;
}

/// The files of a repository, and the items of its `CERTIFICATION.md`.
pub struct Disk {
    root: PathBuf,
    items: BTreeSet<String>,
}

impl Disk {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            items: util::read(&root.join("CERTIFICATION.md"))
                .map(|text| items_in(&text))
                .unwrap_or_default(),
        }
    }
}

impl World for Disk {
    fn guarantee(&self, cited: &Garantie) -> Result<()> {
        match cited {
            Garantie::Item(id) => self
                .items
                .contains(id)
                .then_some(())
                .ok_or_else(|| format!("{id} n'est pas un item de CERTIFICATION.md")),
            Garantie::Test { chemin, nom } => {
                let text = util::read(&self.root.join(chemin))
                    .map_err(|_| format!("{chemin} n'existe pas"))?;
                test_in(&text, nom).map_err(|why| format!("{chemin} : {why}"))
            }
            Garantie::Code { chemin, ligne } => {
                let text = util::read(&self.root.join(chemin))
                    .map_err(|_| format!("{chemin} n'existe pas"))?;
                let lines = text.lines().count();
                let inside = usize::try_from(*ligne).is_ok_and(|ligne| ligne <= lines);
                inside
                    .then_some(())
                    .ok_or_else(|| format!("{chemin} n'a que {lines} lignes, pas {ligne}"))
            }
            Garantie::Chemin(chemin) => self
                .root
                .join(chemin)
                .exists()
                .then_some(())
                .ok_or_else(|| format!("{chemin} n'existe pas")),
        }
    }

    fn place(&self, cited: &Lieu) -> Result<()> {
        self.root
            .join(&cited.chemin)
            .exists()
            .then_some(())
            .ok_or_else(|| format!("{} n'existe pas", cited.chemin))
    }
}

/// The identifiers of the requirement tables.
fn items_in(certification: &str) -> BTreeSet<String> {
    certification
        .lines()
        .filter_map(|line| {
            let mut cells = line.split('|').map(str::trim).skip(1);
            let id = cells.next()?;
            (super::fiche::is_item(id) && cells.count() >= 3).then(|| id.to_owned())
        })
        .collect()
}

/// `name` is a function of `source`, with a test attribute and not ignored.
/// This reads the text: it says the test is there, not that it runs.
fn test_in(source: &str, name: &str) -> Result<()> {
    let wanted = format!("fn {name}");
    let lines: Vec<&str> = source.lines().collect();
    let found = lines.iter().position(|line| {
        let line = line.trim_start();
        let line = line.strip_prefix("pub ").unwrap_or(line);
        let line = line.strip_prefix("async ").unwrap_or(line);
        line.strip_prefix(&wanted)
            .is_some_and(|rest| rest.starts_with(['(', '<']))
    });
    let Some(found) = found else {
        return Err(format!("pas de fonction {name}"));
    };
    let above = lines.get(..found).unwrap_or_default();
    let mut tested = false;
    for line in above.iter().rev().map(|line| line.trim()) {
        if line.is_empty() || line.ends_with(['{', '}', ';']) {
            break;
        }
        if line.starts_with("#[ignore") {
            return Err(format!("{name} est ignoré : il ne garantit rien"));
        }
        if line.starts_with("#[") && line.contains("test") {
            tested = true;
        }
    }
    tested
        .then_some(())
        .ok_or_else(|| format!("{name} n'a pas d'attribut de test"))
}

fn has_client(sous_systeme: &SousSysteme) -> bool {
    sous_systeme
        .chemins
        .iter()
        .any(|chemin| CLIENT_CODE.iter().any(|code| chemin.starts_with(code)))
}

fn filled(errors: &mut Vec<String>, place: &str, field: &str, text: &str) {
    if text.trim().is_empty() {
        errors.push(format!("{place} : `{field}` vide ; attendu un texte"));
    }
}

/// Every rule of one sheet, in reading order. `stem` is its file name without
/// `.toml`.
pub fn rules(fiche: &Fiche, stem: &str, registre: &Registre, world: &dyn World) -> Vec<String> {
    let mut errors = Vec::new();
    if fiche.sous_systeme != stem {
        errors.push(format!(
            "`sous_systeme` vaut « {} » ; attendu le nom du fichier, « {stem} »",
            fiche.sous_systeme
        ));
    }
    let Some(sous_systeme) = registre.sous_systeme(&fiche.sous_systeme) else {
        errors.push(format!(
            "`sous_systeme` « {} » absent de sous-systemes.toml",
            fiche.sous_systeme
        ));
        return errors;
    };
    filled(&mut errors, "fiche", "question", &fiche.question);
    level(fiche, sous_systeme, &mut errors);
    references(fiche, &mut errors);
    lessons(fiche, &mut errors);
    holes(fiche, &mut errors);
    match registre.niveau(fiche.niveau) {
        Some(niveau) => {
            saturation(fiche, sous_systeme, niveau, &mut errors);
            quotas(fiche, sous_systeme, niveau, &mut errors);
        }
        None => errors.push(format!(
            "`niveau` {} absent de sous-systemes.toml ; attendu un `numero` de [[niveau]]",
            fiche.niveau
        )),
    }
    cost(fiche, &mut errors);
    cited(fiche, world, &mut errors);
    errors
}

/// The floor comes from the registry. Above it, nothing to say. Below it, a
/// written reason, which is refused when it has nothing left to excuse.
fn level(fiche: &Fiche, sous_systeme: &SousSysteme, errors: &mut Vec<String>) {
    let floor = sous_systeme.niveau;
    let reason = fiche.raison_niveau.as_deref().map(str::trim);
    if fiche.niveau < floor {
        if reason.is_none_or(str::is_empty) {
            errors.push(format!(
                "`niveau` {} sous le plancher {floor} de {} ; attendu le plancher, ou `raison_niveau`",
                fiche.niveau, sous_systeme.id
            ));
        }
    } else if reason.is_some() {
        errors.push(format!(
            "`raison_niveau` sans objet : le niveau {} n'est pas sous le plancher {floor}",
            fiche.niveau
        ));
    }
}

fn references(fiche: &Fiche, errors: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    for reference in &fiche.references {
        let id = &reference.id;
        if !seen.insert(id.as_str()) {
            errors.push(format!(
                "référence {id} en double ; attendu des `id` uniques"
            ));
        }
        match (
            &reference.depot,
            &reference.commit,
            &reference.document,
            &reference.vu_le,
        ) {
            (Some(_), Some(_), None, _) | (None, None, Some(_), Some(_)) => {}
            _ => errors.push(format!(
                "référence {id} : attendu `depot` et `commit`, ou `document` et `vu_le`"
            )),
        }
        if reference.empreinte.is_some() && reference.document.is_none() {
            errors.push(format!(
                "référence {id} : `empreinte` sans `document` ; un dépôt se fige par `commit`"
            ));
        }
        if reference.consulte.is_empty() {
            errors.push(format!(
                "référence {id} : `consulte` vide ; attendu ce qui a réellement été lu"
            ));
        }
        for line in &reference.consulte {
            filled(errors, &format!("référence {id}"), "consulte", line);
        }
    }
    let mut orders: Vec<u32> = fiche.references.iter().map(|r| r.ordre).collect();
    orders.sort_unstable();
    let contiguous = orders
        .iter()
        .zip(1_u32..)
        .all(|(order, expected)| *order == expected);
    if !contiguous {
        errors.push(format!(
            "`ordre` des références : {orders:?} ; attendu 1, 2, 3… sans trou ni doublon"
        ));
    }
}

fn lessons(fiche: &Fiche, errors: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    let mut documents = BTreeSet::new();
    let known_holes: BTreeSet<&str> = fiche.trous.iter().map(|t| t.id.as_str()).collect();
    for lecon in &fiche.lecons {
        let id = &lecon.id;
        if !seen.insert(id.as_str()) {
            errors.push(format!("leçon {id} en double ; attendu des `id` uniques"));
        }
        filled(errors, id, "enonce", &lecon.enonce);
        filled(errors, id, "citation", &lecon.citation);
        if let Some(note) = &lecon.note {
            filled(errors, id, "note", note);
        }
        let words = lecon.citation.split_whitespace().count();
        if words >= CITATION_WORDS {
            errors.push(format!(
                "{id} : `citation` de {words} mots ; attendu moins de {CITATION_WORDS}"
            ));
        }
        if !documents.insert(lecon.lien.document()) {
            errors.push(format!(
                "{id} : deuxième extrait de {} ; attendu un seul extrait par document",
                lecon.lien.document()
            ));
        }
        match fiche.references.iter().find(|r| r.id == lecon.source) {
            Some(reference) => source_of(lecon, reference, errors),
            None => errors.push(format!(
                "{id} : `source` « {} » n'est pas une référence de la fiche",
                lecon.source
            )),
        }
        state(lecon, &known_holes, errors);
    }
}

/// The link must come from the reference it claims, and a link that can
/// change must say when it was seen.
fn source_of(lecon: &Lecon, reference: &Reference, errors: &mut Vec<String>) {
    let id = &lecon.id;
    match (lecon.lien.depot(), &reference.depot) {
        (Some(link), Some(depot)) if link != depot => errors.push(format!(
            "{id} : `lien` sur {link}, alors que la source {} est {depot}",
            reference.id
        )),
        (Some(link), None) => errors.push(format!(
            "{id} : `lien` sur {link}, alors que la source {} n'a pas de dépôt",
            reference.id
        )),
        (Some(_), Some(_)) | (None, _) => {}
    }
    if lecon.lien.depot().is_none() {
        let document = lecon.lien.document();
        let page = document.split('#').next().unwrap_or_default();
        if reference.document.as_deref() != Some(page) {
            errors.push(format!(
                "{id} : `lien` sur {page}, qui n'est pas le `document` de la source {}",
                reference.id
            ));
        }
    }
    if lecon.lien.mutable() && reference.vu_le.is_none() {
        errors.push(format!(
            "{id} : `lien` vers une issue ou une page ; attendu `vu_le` sur la source {}",
            reference.id
        ));
    }
}

fn state(lecon: &Lecon, known_holes: &BTreeSet<&str>, errors: &mut Vec<String>) {
    let id = &lecon.id;
    if lecon.decision == Decision::Jeter && lecon.etat != Etat::NonApplicable {
        errors.push(format!(
            "{id} : `decision = \"jeter\"` ; attendu `etat = \"non-applicable\"`"
        ));
    }
    let reason = lecon.raison.as_deref().map_or("", str::trim);
    if lecon.etat == Etat::NonApplicable && reason.is_empty() {
        errors.push(format!(
            "{id} : `etat = \"non-applicable\"` sans `raison` ; attendu pourquoi"
        ));
    }
    if lecon.etat == Etat::Couvert && lecon.garantie.is_empty() {
        errors.push(format!(
            "{id} : `etat = \"couvert\"` sans `garantie` ; attendu le test ou l'item qui le couvre"
        ));
    }
    match (&lecon.trou, lecon.etat) {
        (None, Etat::Trou) => errors.push(format!(
            "{id} : `etat = \"trou\"` sans `trou` ; attendu l'id d'un [[trou]]"
        )),
        (Some(trou), Etat::NonApplicable) => errors.push(format!(
            "{id} : `trou = \"{trou}\"` sur une leçon non applicable"
        )),
        (Some(trou), Etat::Trou | Etat::Couvert) if !known_holes.contains(trou.as_str()) => errors
            .push(format!(
                "{id} : `trou = \"{trou}\"` ; attendu un [[trou]] de cette fiche"
            )),
        (Some(_), Etat::Trou | Etat::Couvert) | (None, Etat::Couvert | Etat::NonApplicable) => {}
    }
}

fn holes(fiche: &Fiche, errors: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    for trou in &fiche.trous {
        let id = &trou.id;
        if !seen.insert(id.as_str()) {
            errors.push(format!("trou {id} en double ; attendu des `id` uniques"));
        }
        for (field, text) in [
            ("titre", &trou.titre),
            ("cas", &trou.cas),
            ("preuve", &trou.preuve),
            ("verifie", &trou.verifie),
            ("suppose", &trou.suppose),
        ] {
            filled(errors, id, field, text);
        }
        if trou.lieux.is_empty() {
            errors.push(format!(
                "{id} : `lieux` vide ; attendu au moins un chemin:ligne"
            ));
        }
        let lesson = fiche
            .lecons
            .iter()
            .any(|l| l.trou.as_deref() == Some(id.as_str()));
        if !lesson {
            errors.push(format!(
                "{id} : aucune leçon ne le porte ; attendu au moins une leçon avec `trou = \"{id}\"`"
            ));
        }
    }
}

fn saturation(
    fiche: &Fiche,
    sous_systeme: &SousSysteme,
    niveau: &Niveau,
    errors: &mut Vec<String>,
) {
    let mut ordered: Vec<&Reference> = fiche.references.iter().collect();
    ordered.sort_by_key(|r| r.ordre);
    let last: BTreeSet<&str> = ordered
        .iter()
        .rev()
        .take(niveau.saturation)
        .map(|r| r.id.as_str())
        .collect();
    let cited = &fiche.saturation.sources;
    let listed: BTreeSet<&str> = cited.iter().map(String::as_str).collect();
    if listed != last || cited.len() != niveau.saturation {
        errors.push(format!(
            "[saturation] `sources` : {} ; attendu les {} derniers `ordre` : {}",
            cited.join(", "),
            niveau.saturation,
            last.iter().copied().collect::<Vec<_>>().join(", ")
        ));
    }
    for id in cited {
        if fiche.lecons.iter().any(|l| &l.source == id) {
            errors.push(format!(
                "[saturation] {id} a donné une leçon ; attendu des sources sans aucune leçon nouvelle"
            ));
        }
    }
    let recoupe: BTreeSet<&str> = fiche
        .saturation
        .recoupe
        .keys()
        .map(String::as_str)
        .collect();
    if recoupe != listed {
        errors.push(format!(
            "[saturation.recoupe] : {} ; attendu une entrée pour chaque source : {}",
            recoupe.iter().copied().collect::<Vec<_>>().join(", "),
            listed.iter().copied().collect::<Vec<_>>().join(", ")
        ));
    }
    for (id, text) in &fiche.saturation.recoupe {
        filled(errors, "[saturation.recoupe]", id, text);
    }
    filled(errors, "[saturation]", "note", &fiche.saturation.note);
    let latest = ordered
        .iter()
        .rev()
        .find(|r| fiche.lecons.iter().any(|l| l.source == r.id));
    if let Some(latest) = latest
        && latest.id != fiche.saturation.derniere_lecon
    {
        errors.push(format!(
            "[saturation] `derniere_lecon` vaut « {} » ; attendu {}, la dernière source à avoir donné une leçon",
            fiche.saturation.derniere_lecon, latest.id
        ));
    }
    coverage(fiche, sous_systeme, errors);
}

/// Three norms in a row say nothing of the code, and three servers nothing of
/// the client: the pilot stopped too early by counting sources alone.
fn coverage(fiche: &Fiche, sous_systeme: &SousSysteme, errors: &mut Vec<String>) {
    let sources: Vec<&Reference> = fiche
        .saturation
        .sources
        .iter()
        .filter_map(|id| fiche.references.iter().find(|r| &r.id == id))
        .collect();
    let code = sources.iter().any(|r| r.sorte == Sorte::Reel);
    if sous_systeme.marches.contains(&Sorte::Reel) && !code {
        errors.push(
            "[saturation] aucune source de code réel ; attendu au moins une référence `reel` parmi les sources de saturation".to_owned(),
        );
    }
    let client = sources.iter().any(|r| r.nature == Some(Nature::CodeClient));
    if has_client(sous_systeme) && !client {
        errors.push(format!(
            "[saturation] {} a un client et aucune source de saturation ne le lit ; attendu au moins une référence avec `nature = \"code-client\"`",
            sous_systeme.id
        ));
    }
}

/// The level's quota applies to the steps this subsystem requires, and each
/// required step needs at least one reference.
fn quotas(fiche: &Fiche, sous_systeme: &SousSysteme, niveau: &Niveau, errors: &mut Vec<String>) {
    let total = fiche.references.len();
    if total < niveau.references {
        errors.push(format!(
            "{total} référence(s) ; le niveau {} en demande au moins {}",
            niveau.numero, niveau.references
        ));
    }
    for sorte in &sous_systeme.marches {
        let wanted = niveau.quota.get(sorte).copied().unwrap_or(1).max(1);
        let found = fiche
            .references
            .iter()
            .filter(|r| r.sorte == *sorte)
            .count();
        if found < wanted {
            errors.push(format!(
                "marche {sorte} : {found} référence(s) ; le niveau {} en demande au moins {wanted}",
                niveau.numero
            ));
        }
    }
}

fn cost(fiche: &Fiche, errors: &mut Vec<String>) {
    if fiche.cout.references != fiche.references.len() {
        errors.push(format!(
            "[cout] `references` vaut {} ; la fiche en a {}",
            fiche.cout.references,
            fiche.references.len()
        ));
    }
    if fiche.cout.minutes == 0 {
        errors.push("[cout] `minutes` vaut 0 ; attendu le temps réel".to_owned());
    }
    filled(errors, "[cout]", "note", &fiche.cout.note);
}

fn cited(fiche: &Fiche, world: &dyn World, errors: &mut Vec<String>) {
    for lecon in &fiche.lecons {
        for garantie in &lecon.garantie {
            if let Err(why) = world.guarantee(garantie) {
                errors.push(format!("{} : `garantie` {garantie} : {why}", lecon.id));
            }
        }
    }
    for trou in &fiche.trous {
        for lieu in &trou.lieux {
            if let Err(why) = world.place(lieu) {
                errors.push(format!("{} : `lieux` {lieu} : {why}", trou.id));
            }
        }
    }
}

/// The registry's own coherence: a sheet is judged against it.
pub fn registry_rules(registre: &Registre) -> Vec<String> {
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();
    for niveau in &registre.niveaux {
        filled(&mut errors, "niveau", "intention", &niveau.intention);
        for sorte in niveau.quota.keys() {
            if !registre.marches.contains(sorte) {
                errors.push(format!(
                    "niveau {} : quota sur {sorte}, absente de `marches`",
                    niveau.numero
                ));
            }
        }
    }
    for sous_systeme in &registre.sous_systemes {
        let id = &sous_systeme.id;
        if !seen.insert(id.as_str()) {
            errors.push(format!("sous-système {id} en double"));
        }
        if registre.niveau(sous_systeme.niveau).is_none() {
            errors.push(format!("{id} : niveau {} inconnu", sous_systeme.niveau));
        }
        if sous_systeme.chemins.is_empty() {
            errors.push(format!(
                "{id} : `chemins` vide ; attendu ce qui le déclenche"
            ));
        }
        if sous_systeme.marches.is_empty() {
            errors.push(format!("{id} : `marches` vide"));
        }
        for sorte in &sous_systeme.marches {
            if !registre.marches.contains(sorte) {
                errors.push(format!("{id} : marche {sorte}, absente de `marches`"));
            }
        }
    }
    errors
}

/// `sources.toml` against the registry and the sheets: a source said to be
/// read is a reference of its sheet.
pub fn sources_rules(sources: &Sources, registre: &Registre, fiches: &[&Fiche]) -> Vec<String> {
    let mut errors = Vec::new();
    for source in &sources.sources {
        let id = &source.sous_systeme;
        if registre.sous_systeme(id).is_none() {
            errors.push(format!("sous-système {id} absent de sous-systemes.toml"));
        }
        if source.ids.is_empty() {
            errors.push(format!("{id} : `ids` vide"));
        }
        if let Some(why) = &source.pourquoi {
            filled(&mut errors, id, "pourquoi", why);
        }
        if source.etat == EtatSource::Candidate {
            continue;
        }
        let fiche = fiches.iter().find(|f| &f.sous_systeme == id);
        for cited in &source.ids {
            let read = fiche.is_some_and(|fiche| {
                fiche.references.iter().any(|r| {
                    r.document.as_deref() == Some(cited)
                        || r.depot.as_ref().is_some_and(|d| &d.to_string() == cited)
                })
            });
            if !read {
                errors.push(format!(
                    "{id} : {cited} dit `consultee` ; attendu une référence de sa fiche, sinon `candidate`"
                ));
            }
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::arithmetic_side_effects,
        reason = "des positions dans un texte de test, sur la fiche réelle"
    )]

    use super::super::fiche::{self, Sources};
    use super::*;

    const SHEET: &str = include_str!("../../../recherche/reconnexion.toml");
    const REGISTRY: &str = include_str!("../../../recherche/sous-systemes.toml");
    const SOURCES: &str = include_str!("../../../recherche/sources.toml");

    /// Believes everything: these tests break one rule at a time.
    struct Trusting;

    impl World for Trusting {
        fn guarantee(&self, _: &Garantie) -> Result<()> {
            Ok(())
        }
        fn place(&self, _: &Lieu) -> Result<()> {
            Ok(())
        }
    }

    fn registre() -> Registre {
        fiche::read(REGISTRY, "sous-systemes.toml").unwrap()
    }

    fn errors_of(text: &str) -> Vec<String> {
        let parsed: Fiche = fiche::read(text, "reconnexion.toml").unwrap();
        rules(&parsed, "reconnexion", &registre(), &Trusting)
    }

    /// Replaces text that must be there: a mutation that no longer applies
    /// would otherwise test nothing.
    fn with(text: &str, from: &str, to: &str) -> String {
        assert!(text.contains(from), "absent de la fiche : {from}");
        text.replacen(from, to, 1)
    }

    fn lesson(id: &str) -> String {
        let start = SHEET.find(&format!("id = \"{id}\"")).unwrap();
        let end = SHEET[start..].find("\n\n").unwrap();
        SHEET[start..start + end].to_owned()
    }

    /// The first lesson still open, with its hole: the sheet closes them as
    /// the fixes land, and these tests must not depend on which one is left.
    fn open_lesson() -> (String, String) {
        let parsed: Fiche = fiche::read(SHEET, "reconnexion.toml").unwrap();
        let hole = parsed.open().next().unwrap().id.clone();
        let id = parsed
            .lecons
            .iter()
            .find(|l| l.etat == Etat::Trou && l.trou.as_deref() == Some(&hole))
            .unwrap()
            .id
            .clone();
        (lesson(&id), hole)
    }

    fn some_error(text: &str, expected: &str) {
        let found = errors_of(text);
        assert!(
            found.iter().any(|e| e.contains(expected)),
            "{expected} ∉ {found:?}"
        );
    }

    fn one_error(text: &str, expected: &str) {
        some_error(text, expected);
        let found = errors_of(text);
        assert_eq!(found.len(), 1, "{expected} : {found:?}");
    }

    #[test]
    fn the_pilot_sheet_follows_every_rule() {
        assert_eq!(errors_of(SHEET), Vec::<String>::new());
    }

    #[test]
    fn the_pilot_sheet_and_the_registry_agree_on_the_real_tree() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let parsed: Fiche = fiche::read(SHEET, "reconnexion.toml").unwrap();
        let disk = Disk::new(&root);
        let found: Vec<String> = cited_errors(&parsed, &disk);
        assert_eq!(found, Vec::<String>::new());
        assert_eq!(registry_rules(&registre()), Vec::<String>::new());
        let sources: Sources = fiche::read(SOURCES, "sources.toml").unwrap();
        assert_eq!(
            sources_rules(&sources, &registre(), &[&parsed]),
            Vec::<String>::new()
        );
    }

    fn cited_errors(parsed: &Fiche, world: &dyn World) -> Vec<String> {
        let mut errors = Vec::new();
        cited(parsed, world, &mut errors);
        errors
    }

    #[test]
    fn a_wrong_format_stops_at_the_reading() {
        let cases = [
            (
                with(SHEET, "niveau = 3\n", "niveau = 3\nauteur = \"x\"\n"),
                "auteur",
            ),
            (
                with(SHEET, "sorte = \"chez-nous\"", "sorte = \"blog\""),
                "blog",
            ),
            (
                with(
                    SHEET,
                    "categorie = \"usage-reel\"",
                    "categorie = \"folklore\"",
                ),
                "folklore",
            ),
            (
                with(SHEET, "decision = \"garder\"", "decision = \"peut-etre\""),
                "peut-etre",
            ),
            (
                with(SHEET, "etat = \"couvert\"", "etat = \"plutot\""),
                "plutot",
            ),
            (
                with(SHEET, "gravite = \"haute\"", "gravite = \"enorme\""),
                "enorme",
            ),
            (
                with(SHEET, "compare_a = \"3194971", "compare_a = \"ZZ94971"),
                "commit",
            ),
            (
                with(
                    SHEET,
                    "commit = \"319497130bd9d4b4dcaffd4133dd529ada46aa41\"",
                    "commit = \"3194971\"",
                ),
                "commit",
            ),
            (
                with(SHEET, "vu_le = \"2026-11-01\"", "vu_le = \"hier\""),
                "date",
            ),
            (
                with(SHEET, "empreinte = \"sha256:7657", "empreinte = \"md5:7657"),
                "empreinte",
            ),
            (
                with(
                    SHEET,
                    "/blob/2f01c3c88f361aeb9ebbf2cc9b247ee230dd815e/",
                    "/blob/main/",
                ),
                "lien",
            ),
            (with(SHEET, "\"C2\"]", "\"tout va bien\"]"), "garantie"),
            (
                with(
                    SHEET,
                    "\"salon/crates/core/src/host.rs:430\"",
                    "\"salon/crates/core/src/host.rs\"",
                ),
                "lieu",
            ),
        ];
        for (text, expected) in cases {
            let error = fiche::read::<Fiche>(&text, "x.toml").unwrap_err();
            assert!(error.contains(expected), "{expected} ∉ {error}");
            assert!(error.starts_with("x.toml:"), "{error}");
        }
        let no_saturation = SHEET.split("[saturation]").next().unwrap();
        assert!(fiche::read::<Fiche>(no_saturation, "x.toml").is_err());
    }

    #[test]
    fn the_sheet_is_named_after_a_known_subsystem() {
        let text = with(
            SHEET,
            "sous_systeme = \"reconnexion\"",
            "sous_systeme = \"autre\"",
        );
        let found = errors_of(&text);
        assert!(
            found
                .iter()
                .any(|e| e.contains("attendu le nom du fichier"))
        );
        assert!(
            found
                .iter()
                .any(|e| e.contains("absent de sous-systemes.toml"))
        );
        let parsed: Fiche = fiche::read(SHEET, "x.toml").unwrap();
        let found = rules(&parsed, "autre", &registre(), &Trusting);
        assert!(found.first().unwrap().contains("attendu le nom du fichier"));
    }

    #[test]
    fn the_level_is_the_floor_unless_a_reason_lowers_it() {
        let lowered = with(SHEET, "niveau = 3\n", "niveau = 2\n");
        one_error(&lowered, "sous le plancher 3");
        let excused = with(
            &lowered,
            "niveau = 2\n",
            "niveau = 2\nraison_niveau = \"banc seul\"\n",
        );
        assert_eq!(errors_of(&excused), Vec::<String>::new());
        let empty = with(
            &lowered,
            "niveau = 2\n",
            "niveau = 2\nraison_niveau = \" \"\n",
        );
        one_error(&empty, "sous le plancher 3");
        let raised = with(
            SHEET,
            "niveau = 3\n",
            "niveau = 3\nraison_niveau = \"pour rien\"\n",
        );
        one_error(&raised, "`raison_niveau` sans objet");
    }

    #[test]
    fn an_unknown_level_is_refused_rather_than_free_of_quotas() {
        one_error(
            &with(SHEET, "niveau = 3\n", "niveau = 9\n"),
            "`niveau` 9 absent",
        );
    }

    #[test]
    fn a_free_text_field_is_never_empty() {
        let question = SHEET
            .lines()
            .find(|l| l.starts_with("question = "))
            .unwrap();
        one_error(&with(SHEET, question, "question = \"\""), "`question` vide");
        let note = SHEET.rfind("\nnote = ").unwrap() + 1;
        let end = note + SHEET[note..].find('\n').unwrap_or(SHEET.len() - note);
        one_error(
            &with(SHEET, &SHEET[note..end], "note = \"\""),
            "[cout] : `note` vide",
        );
        let saturation = SHEET.split("[saturation]").nth(1).unwrap();
        let line = saturation
            .lines()
            .find(|l| l.starts_with("note = "))
            .unwrap();
        one_error(
            &with(SHEET, line, "note = \"\""),
            "[saturation] : `note` vide",
        );
        let lesson = lesson("reconnexion-07");
        let line = lesson.lines().find(|l| l.starts_with("note = ")).unwrap();
        one_error(
            &with(SHEET, line, "note = \"\""),
            "reconnexion-07 : `note` vide",
        );
    }

    #[test]
    fn a_reference_has_one_frozen_origin() {
        some_error(
            &with(SHEET, "id = \"punchline\"", "id = \"salon\""),
            "référence salon en double",
        );
        one_error(
            &with(SHEET, "ordre = 2\n", "ordre = 3\n"),
            "`ordre` des références",
        );
        one_error(
            &with(
                SHEET,
                "commit = \"6770a99f27ad6f15763b85094ea8145786dd8da5\"\n",
                "",
            ),
            "attendu `depot` et `commit`",
        );
        some_error(
            &with(
                SHEET,
                "document = \"https://slack.engineering/slacks-incident-on-2-22-22/\"\nvu_le = \"2026-11-01\"\n",
                "document = \"https://slack.engineering/slacks-incident-on-2-22-22/\"\n",
            ),
            "attendu `depot` et `commit`",
        );
        one_error(
            &with(
                SHEET,
                "depot = \"build-your-own-game/punchline\"\n",
                "depot = \"build-your-own-game/punchline\"\ndocument = \"https://x.org/a\"\n",
            ),
            "attendu `depot` et `commit`",
        );
        one_error(
            &with(
                SHEET,
                "depot = \"build-your-own-game/punchline\"\n",
                "depot = \"build-your-own-game/punchline\"\nempreinte = \"sha256:765775326aee0ecca9b04bde3fd1f52932d498e33e34e428bd61b8a24da0fa3b\"\n",
            ),
            "`empreinte` sans `document`",
        );
        let consulte = "consulte = [\n  \"postmortem entier\",\n]";
        one_error(&with(SHEET, consulte, "consulte = []"), "`consulte` vide");
        one_error(
            &with(SHEET, consulte, "consulte = [\"\"]"),
            "`consulte` vide",
        );
    }

    #[test]
    fn a_lesson_is_a_short_quote_from_its_own_source() {
        let long =
            "un deux trois quatre cinq six sept huit neuf dix onze douze treize quatorze quinze";
        one_error(
            &with(
                SHEET,
                "citation = \"so one bad-token attempt\"",
                &format!("citation = \"{long}\""),
            ),
            "`citation` de 15 mots",
        );
        one_error(
            &with(
                SHEET,
                "citation = \"so one bad-token attempt\"",
                "citation = \"\"",
            ),
            "`citation` vide",
        );
        one_error(
            &with(
                SHEET,
                "enonce = \"Valider le jeton avant tout effet de bord : un essai faux ne doit rien changer à la place tenue.\"",
                "enonce = \" \"",
            ),
            "`enonce` vide",
        );
        one_error(
            &with(SHEET, "id = \"reconnexion-02\"", "id = \"reconnexion-01\""),
            "leçon reconnexion-01 en double",
        );
        one_error(
            &with(SHEET, "source = \"punchline\"", "source = \"inconnue\""),
            "n'est pas une référence",
        );
        one_error(
            &with(SHEET, "source = \"punchline\"", "source = \"sidequest\""),
            "alors que la source sidequest est",
        );
        one_error(
            &with(SHEET, "source = \"punchline\"", "source = \"slack\""),
            "n'a pas de dépôt",
        );
        one_error(
            &with(SHEET, "source = \"figma\"", "source = \"slack\""),
            "qui n'est pas le `document`",
        );
        let colyseus =
            "https://github.com/colyseus/colyseus/commit/d6c554f37a188824befec57a04b8e0854e54836f";
        one_error(
            &with(
                SHEET,
                colyseus,
                "https://github.com/colyseus/colyseus/commit/a22c6c586a6957c49681de318f395ee0683df60c",
            ),
            "deuxième extrait",
        );
    }

    #[test]
    fn an_issue_or_a_page_is_dated_by_its_source() {
        let socketio = "depot = \"socketio/socket.io\"\ncommit = \"1eaa582d3b453e3e6f522300ed05b10da0a0799b\"\nvu_le = \"2026-11-02\"\n";
        one_error(
            &with(
                SHEET,
                socketio,
                "depot = \"socketio/socket.io\"\ncommit = \"1eaa582d3b453e3e6f522300ed05b10da0a0799b\"\n",
            ),
            "attendu `vu_le` sur la source socketio",
        );
    }

    #[test]
    fn a_state_needs_what_proves_it() {
        let reference_08 = lesson("reconnexion-08");
        one_error(
            &with(
                SHEET,
                &reference_08,
                &reference_08.replace(
                    "etat = \"non-applicable\"",
                    "etat = \"couvert\"\ngarantie = [\"C2\"]",
                ),
            ),
            "`decision = \"jeter\"`",
        );
        let reason = reference_08
            .lines()
            .find(|l| l.starts_with("raison"))
            .unwrap();
        one_error(
            &with(
                SHEET,
                &reference_08,
                &reference_08.replace(&format!("\n{reason}"), ""),
            ),
            "sans `raison`",
        );
        let reference_01 = lesson("reconnexion-01");
        let guarantee = reference_01
            .lines()
            .find(|l| l.starts_with("garantie"))
            .unwrap();
        one_error(
            &with(
                SHEET,
                &reference_01,
                &reference_01.replace(&format!("\n{guarantee}"), ""),
            ),
            "sans `garantie`",
        );
        one_error(
            &with(
                SHEET,
                &reference_01,
                &reference_01.replace(guarantee, "garantie = []"),
            ),
            "sans `garantie`",
        );
        let (open, hole) = open_lesson();
        some_error(
            &with(
                SHEET,
                &open,
                &open.replace(&format!("trou = \"{hole}\""), "trou = \"T99\""),
            ),
            "attendu un [[trou]] de cette fiche",
        );
        some_error(
            &with(
                SHEET,
                &open,
                &open.replace(&format!("\ntrou = \"{hole}\""), ""),
            ),
            "sans `trou`",
        );
        one_error(
            &with(
                SHEET,
                &reference_08,
                &format!("{reference_08}\ntrou = \"T1\""),
            ),
            "sur une leçon non applicable",
        );
    }

    #[test]
    fn a_covered_lesson_may_keep_its_hole_and_closes_it() {
        let (open, hole) = open_lesson();
        let parsed: Fiche = fiche::read(SHEET, "reconnexion.toml").unwrap();
        let before = parsed.open().count();
        let text = SHEET.replace(
            &format!("etat = \"trou\"\ntrou = \"{hole}\""),
            &format!("etat = \"couvert\"\ngarantie = [\"C2\"]\ntrou = \"{hole}\""),
        );
        assert_ne!(text, SHEET, "{open}");
        assert_eq!(errors_of(&text), Vec::<String>::new());
        let parsed: Fiche = fiche::read(&text, "reconnexion.toml").unwrap();
        assert_eq!(parsed.open().count(), before - 1);
        assert!(!parsed.open().any(|t| t.id == hole));
    }

    #[test]
    fn a_hole_needs_a_lesson_a_place_and_its_texts() {
        let orphan = "[[trou]]\nid = \"T14\"\ntitre = \"x\"\ngravite = \"faible\"\nlieux = [\"a.rs:1\"]\ncas = \"x\"\npreuve = \"x\"\nverifie = \"x\"\nsuppose = \"x\"\n\n[saturation]";
        one_error(&with(SHEET, "[saturation]", orphan), "T14 : aucune leçon");
        some_error(
            &with(SHEET, "id = \"T2\"", "id = \"T1\""),
            "trou T1 en double",
        );
        one_error(
            &with(
                SHEET,
                "lieux = [\n  \"salon/crates/core/src/host.rs:764\",\n]",
                "lieux = []",
            ),
            "`lieux` vide",
        );
        for field in ["titre", "cas", "preuve", "verifie", "suppose"] {
            let start = SHEET.rfind(&format!("\n{field} = ")).unwrap() + 1;
            let end = start + SHEET[start..].find('\n').unwrap();
            one_error(
                &with(SHEET, &SHEET[start..end], &format!("{field} = \"\"")),
                &format!("`{field}` vide"),
            );
        }
    }

    #[test]
    fn saturation_is_the_last_sources_without_a_lesson() {
        some_error(
            &with(
                SHEET,
                "sources = [\"signalr-client\", \"livekit\", \"graphql-ws\"]",
                "sources = [\"signalr-client\", \"livekit\"]",
            ),
            "attendu les 3 derniers `ordre`",
        );
        let found = errors_of(&with(
            SHEET,
            "sources = [\"signalr-client\", \"livekit\", \"graphql-ws\"]",
            "sources = [\"centrifuge\", \"livekit\", \"graphql-ws\"]",
        ));
        assert!(found.iter().any(|e| e.contains("attendu les 3 derniers")));
        assert!(
            found
                .iter()
                .any(|e| e.contains("attendu une entrée pour chaque source"))
        );
        let moved = SHEET.replace(
            "source = \"centrifuge-js\"",
            "source = \"signalr-client\"",
        ).replace(
            "https://github.com/centrifugal/centrifuge-js/commit/fe060f378461c9dc3c042a6759af8c4555be9720",
            "https://github.com/dotnet/aspnetcore/commit/fe060f378461c9dc3c042a6759af8c4555be9720",
        ).replace(
            "https://github.com/centrifugal/centrifuge-js/blob/bd9f60da92764314903ba6c8998843158fef4171/src/centrifuge.ts#L836",
            "https://github.com/dotnet/aspnetcore/blob/bd9f60da92764314903ba6c8998843158fef4171/src/centrifuge.ts#L836",
        );
        let found = errors_of(&moved);
        assert!(
            found
                .iter()
                .any(|e| e.contains("[saturation] signalr-client a donné une leçon")),
            "{found:?}"
        );
        assert!(
            found.iter().any(|e| e.contains("`derniere_lecon`")),
            "{found:?}"
        );
        one_error(
            &with(
                SHEET,
                "derniere_lecon = \"centrifuge-js\"",
                "derniere_lecon = \"nakama\"",
            ),
            "`derniere_lecon` vaut « nakama »",
        );
        let recoupe = SHEET.lines().find(|l| l.starts_with("livekit = ")).unwrap();
        one_error(
            &with(SHEET, &format!("{recoupe}\n"), ""),
            "attendu une entrée pour chaque source",
        );
        one_error(&with(SHEET, recoupe, "livekit = \"\""), "`livekit` vide");
    }

    #[test]
    fn saturation_must_reach_the_client_when_there_is_one() {
        // The pilot's first stop: a server and two specifications.
        let client = "nature = \"code-client\"\n";
        let found = errors_of(&SHEET.replace(client, ""));
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("reconnexion a un client"), "{found:?}");
        let one = SHEET.replacen(client, "", 2);
        assert_eq!(errors_of(&one), Vec::<String>::new());
        let server = SHEET.replace(client, "nature = \"code-serveur\"\n");
        assert!(errors_of(&server)[0].contains("reconnexion a un client"));
    }

    #[test]
    fn saturation_on_norms_alone_is_refused() {
        let found = errors_of(
            &SHEET
                .replace(
                    "sorte = \"reel\"\ndepot = \"enisdenjo/graphql-ws\"",
                    "sorte = \"normes\"\ndepot = \"enisdenjo/graphql-ws\"",
                )
                .replace(
                    "sorte = \"reel\"\ndepot = \"livekit/client-sdk-js\"",
                    "sorte = \"normes\"\ndepot = \"livekit/client-sdk-js\"",
                )
                .replace(
                    "sorte = \"reel\"\ndepot = \"dotnet/aspnetcore\"",
                    "sorte = \"normes\"\ndepot = \"dotnet/aspnetcore\"",
                ),
        );
        assert!(
            found
                .iter()
                .any(|e| e.contains("aucune source de code réel")),
            "{found:?}"
        );
    }

    #[test]
    fn a_subsystem_without_a_client_owes_no_client_source() {
        let registre = registre();
        assert!(has_client(registre.sous_systeme("reconnexion").unwrap()));
        assert!(has_client(registre.sous_systeme("telecommande").unwrap()));
        assert!(has_client(registre.sous_systeme("entrees").unwrap()));
        assert!(!has_client(
            registre.sous_systeme("transport-websocket").unwrap()
        ));
        assert!(!has_client(registre.sous_systeme("verification").unwrap()));
    }

    #[test]
    fn quotas_count_each_required_step() {
        let few: String = SHEET.replace("sorte = \"retours\"", "sorte = \"reel\"");
        let found = errors_of(&few);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found[0].contains("marche retours : 0 référence(s)"),
            "{found:?}"
        );
        let found = errors_of(&SHEET.replace("sorte = \"bancs\"", "sorte = \"reel\""));
        assert!(found[0].contains("marche bancs : 0"), "{found:?}");
        let found = errors_of(&SHEET.replacen("sorte = \"retours\"", "sorte = \"reel\"", 2));
        assert!(
            found[0]
                .contains("marche retours : 1 référence(s) ; le niveau 3 en demande au moins 2"),
            "{found:?}"
        );
        let registre = registre();
        let mut parsed: Fiche = fiche::read(SHEET, "reconnexion.toml").unwrap();
        parsed.references.truncate(11);
        let mut errors = Vec::new();
        quotas(
            &parsed,
            registre.sous_systeme("reconnexion").unwrap(),
            registre.niveau(3).unwrap(),
            &mut errors,
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("11 référence(s) ; le niveau 3 en demande au moins 12")),
            "{errors:?}"
        );
    }

    #[test]
    fn a_step_outside_the_level_quota_still_needs_one_reference() {
        let mut registre = registre();
        for niveau in &mut registre.niveaux {
            niveau.quota.remove(&Sorte::Retours);
        }
        let text = SHEET.replace("sorte = \"retours\"", "sorte = \"reel\"");
        let parsed: Fiche = fiche::read(&text, "reconnexion.toml").unwrap();
        let sous_systeme = registre.sous_systeme("reconnexion").unwrap();
        let mut errors = Vec::new();
        quotas(
            &parsed,
            sous_systeme,
            registre.niveau(3).unwrap(),
            &mut errors,
        );
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            errors[0]
                .contains("marche retours : 0 référence(s) ; le niveau 3 en demande au moins 1")
        );
    }

    #[test]
    fn the_cost_counts_what_the_sheet_holds() {
        one_error(
            &with(SHEET, "references = 28\n", "references = 27\n"),
            "[cout] `references` vaut 27",
        );
        one_error(
            &with(SHEET, "minutes = 33\n", "minutes = 0\n"),
            "`minutes` vaut 0",
        );
    }

    fn fixture(files: &[(&str, &str)]) -> PathBuf {
        let dir = crate::testdir::fresh("recherche-garanties");
        for (path, text) in files {
            util::write(&dir.join(path), text).unwrap();
        }
        dir
    }

    fn guarantee(dir: &Path, text: &str) -> Result<()> {
        Disk::new(dir).guarantee(&Garantie::try_from(text.to_owned()).unwrap())
    }

    #[test]
    fn a_cited_test_item_or_file_must_exist() {
        let dir = fixture(&[
            (
                "CERTIFICATION.md",
                "| C2 | Exigence | origine | salon |\n| Id | Pas un item | x | y |\n",
            ),
            (
                "t/a.rs",
                "#[test]\nfn real() {}\n\n/// Documented.\n#[tokio::test]\nasync fn asynchronous() {}\n\nfn helper() {}\n\n#[test]\n#[ignore = \"plus tard\"]\nfn ignored() {}\n\nproptest! {\n    #[test]\n    fn chaos(seed in any::<u64>()) {}\n}\n",
            ),
        ]);
        for held in [
            "C2",
            "t/a.rs::real",
            "t/a.rs::asynchronous",
            "t/a.rs::chaos",
            "t/a.rs#L2",
            "t/a.rs",
        ] {
            assert_eq!(guarantee(&dir, held), Ok(()), "{held}");
        }
        for (cited, expected) in [
            ("C9", "pas un item"),
            ("t/a.rs::absent", "pas de fonction"),
            ("t/b.rs::real", "n'existe pas"),
            ("t/a.rs::helper", "pas d'attribut de test"),
            ("t/a.rs::ignored", "ignoré"),
            ("t/a.rs#L99", "n'a que"),
            ("t/b.rs#L1", "n'existe pas"),
            ("t/b.rs", "n'existe pas"),
        ] {
            let error = guarantee(&dir, cited).unwrap_err();
            assert!(error.contains(expected), "{cited} : {error}");
        }
        let place = Lieu::try_from("t/b.rs:3".to_owned()).unwrap();
        assert!(Disk::new(&dir).place(&place).is_err());
    }

    #[test]
    fn the_real_sheet_fails_when_a_guarantee_moves() {
        let dir = fixture(&[("CERTIFICATION.md", "| C2 | Exigence | origine | salon |\n")]);
        let parsed: Fiche = fiche::read(SHEET, "reconnexion.toml").unwrap();
        let found = cited_errors(&parsed, &Disk::new(&dir));
        assert!(
            found
                .iter()
                .any(|e| e
                    .contains("reconnexion-01 : `garantie` salon/crates/core/tests/salon.rs::")),
            "{found:?}"
        );
        assert!(
            found
                .iter()
                .any(|e| e.contains("reconnexion-02 : `garantie` C5")),
            "{found:?}"
        );
        assert!(
            found.iter().any(|e| e.contains("T1 : `lieux`")),
            "{found:?}"
        );
    }

    #[test]
    fn the_registry_is_coherent() {
        let broken = REGISTRY
            .replacen("id = \"fiabilite\"", "id = \"reconnexion\"", 1)
            .replacen(
                "niveau = 3\nmarches = [\"chez-nous\", \"reel\", \"normes\", \"bancs\"]",
                "niveau = 9\nmarches = []",
                1,
            )
            .replacen(
                "chemins = [\"remote/crates/**\", \"remote/ios/**\", \"remote/CONTRAT.md\"]",
                "chemins = []",
                1,
            )
            .replacen(
                "marches = [\"chez-nous\", \"reel\", \"normes\", \"bancs\", \"retours\"]\n\n#",
                "marches = [\"chez-nous\"]\n\n#",
                1,
            );
        let registre: Registre = fiche::read(&broken, "r.toml").unwrap();
        let found = registry_rules(&registre);
        for expected in [
            "sous-système reconnexion en double",
            "niveau 9 inconnu",
            "`chemins` vide",
            "`marches` vide",
            "absente de `marches`",
        ] {
            assert!(
                found.iter().any(|e| e.contains(expected)),
                "{expected} ∉ {found:?}"
            );
        }
    }

    #[test]
    fn a_source_said_to_be_read_is_a_reference_of_its_sheet() {
        let parsed: Fiche = fiche::read(SHEET, "reconnexion.toml").unwrap();
        let sources = |text: &str| fiche::read::<Sources>(text, "s.toml").unwrap();
        let read = sources(
            "[[source]]\nsous_systeme = \"reconnexion\"\nids = [\"colyseus/colyseus\", \"https://slack.engineering/slacks-incident-on-2-22-22/\"]\nmarche = \"reel\"\netat = \"consultee\"\n",
        );
        assert_eq!(
            sources_rules(&read, &registre(), &[&parsed]),
            Vec::<String>::new()
        );
        let unread = sources(
            "[[source]]\nsous_systeme = \"reconnexion\"\nids = [\"geckosio/geckos.io\"]\nmarche = \"reel\"\netat = \"consultee\"\n",
        );
        assert_eq!(sources_rules(&unread, &registre(), &[&parsed]).len(), 1);
        assert_eq!(sources_rules(&read, &registre(), &[]).len(), 2);
        let unknown = sources(
            "[[source]]\nsous_systeme = \"inconnu\"\nids = []\nmarche = \"reel\"\netat = \"candidate\"\npourquoi = \"\"\n",
        );
        assert_eq!(sources_rules(&unknown, &registre(), &[]).len(), 3);
        assert!(
            fiche::read::<Sources>(
                "[[source]]\nsous_systeme = \"a\"\nids = []\nmarche = \"reel\"\netat = \"lue\"\n",
                "s.toml"
            )
            .is_err()
        );
    }
}
