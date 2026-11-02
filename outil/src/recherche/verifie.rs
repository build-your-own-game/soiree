//! `soiree recherche verifie`: the sheets against the world, with the network.
//! Every commit they cite exists upstream, every citation is at the place its
//! link gives, and a document still has its fingerprint. The pilot nearly
//! shipped a dead link, which no offline rule can see.
//!
//! A failure and an outage are different results: a dead link is the sheet's
//! fault, a missing network says nothing about it. So this is not part of
//! `soiree check`, and an outage never reads as a pass or a failure.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use serde_json::Value as Json;
use sha2::{Digest, Sha256};

use super::Folder;
use super::fiche::{Commit, Depot, Fiche, Lecon, Lien, Reference};
use crate::report::{Level, Report};
use crate::util;

/// A citation may wrap: past its first line, how many more are read. A link
/// with a range reads exactly that range.
const WRAPPED_LINES: usize = 3;

/// What `curl` waits for one address, in seconds.
const CURL_SECONDS: &str = "30";

/// The exit code of an outage, as for a usage error: the tool could not say.
const OUTAGE_CODE: u8 = 2;

const COMMENTS_PER_PAGE: u32 = 100;

const GITHUB_API: &str = "https://api.github.com/";

/// What an address answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fetched {
    Body(Vec<u8>),
    /// It answers that this does not exist.
    Missing,
}

/// Why nothing could be said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outage {
    /// Nothing answered: every other request would fail the same way.
    Network(String),
    /// A server refused this request: rate limit, 403, 5xx.
    Refused(String),
}

/// The network as this control uses it.
pub trait Net {
    /// A GitHub API path.
    fn api(&self, path: &str) -> Result<Fetched, Outage>;
    /// A file of a repository at a commit.
    fn file(&self, depot: &Depot, sha: &Commit, path: &str) -> Result<Fetched, Outage>;
    fn web(&self, url: &str) -> Result<Fetched, Outage>;
    /// This clone has the commit, whatever upstream says.
    fn here(&self, sha: &Commit) -> bool;
}

/// `gh` when it is signed in, which also reads private repositories, else `curl`.
struct Tools<'a> {
    dir: &'a Path,
    gh: bool,
}

impl<'a> Tools<'a> {
    fn detect(dir: &'a Path) -> Self {
        Self {
            dir,
            gh: util::run(dir, "gh", &["auth", "status"])
                .is_ok_and(|output| output.status.success()),
        }
    }

    fn gh(args: &[&str]) -> Result<Fetched, Outage> {
        let output = util::run(Path::new("."), "gh", args).map_err(Outage::Network)?;
        if output.status.success() {
            return Ok(Fetched::Body(output.stdout));
        }
        match gh_failure(&String::from_utf8_lossy(&output.stderr)) {
            GhFailure::Missing => Ok(Fetched::Missing),
            GhFailure::Outage(outage) => Err(outage),
        }
    }

    fn curl(url: &str) -> Result<Fetched, Outage> {
        let output = util::run(
            Path::new("."),
            "curl",
            &[
                "-sS",
                "-L",
                "--max-time",
                CURL_SECONDS,
                "-w",
                "\n%{http_code}",
                url,
            ],
        )
        .map_err(Outage::Network)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Outage::Network(format!("curl {url} : {}", stderr.trim())));
        }
        curl_answer(url, &output.stdout)
    }
}

enum GhFailure {
    Missing,
    Outage(Outage),
}

/// `gh api` says the HTTP status in its message, when there was one.
fn gh_failure(stderr: &str) -> GhFailure {
    let text = stderr.trim();
    let status = text.split_once("HTTP ").and_then(|(_, rest)| {
        rest.chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse::<u16>()
            .ok()
    });
    match status {
        Some(404 | 410 | 422) => GhFailure::Missing,
        Some(_) => GhFailure::Outage(Outage::Refused(format!("gh api : {text}"))),
        None => GhFailure::Outage(Outage::Network(format!("gh api : {text}"))),
    }
}

/// The body, then a newline and the status that `curl -w` adds.
fn curl_answer(url: &str, output: &[u8]) -> Result<Fetched, Outage> {
    let split = output.iter().rposition(|byte| *byte == b'\n').unwrap_or(0);
    let (body, status) = output.split_at(split);
    let status = String::from_utf8_lossy(status);
    match status.trim() {
        "404" | "410" => Ok(Fetched::Missing),
        // The API's answer to a commit it does not know, as `gh_failure` reads it.
        "422" if url.starts_with(GITHUB_API) => Ok(Fetched::Missing),
        code if code.starts_with('2') => Ok(Fetched::Body(body.to_vec())),
        code => Err(Outage::Refused(format!("{url} : HTTP {code}"))),
    }
}

impl Net for Tools<'_> {
    fn api(&self, path: &str) -> Result<Fetched, Outage> {
        if self.gh {
            Self::gh(&["api", path])
        } else {
            Self::curl(&format!("{GITHUB_API}{path}"))
        }
    }

    fn file(&self, depot: &Depot, sha: &Commit, path: &str) -> Result<Fetched, Outage> {
        if self.gh {
            let path = format!("repos/{depot}/contents/{path}?ref={sha}");
            Self::gh(&[
                "api",
                "-H",
                "Accept: application/vnd.github.raw+json",
                &path,
            ])
        } else {
            Self::curl(&format!(
                "https://raw.githubusercontent.com/{depot}/{sha}/{path}"
            ))
        }
    }

    fn web(&self, url: &str) -> Result<Fetched, Outage> {
        Self::curl(url)
    }

    fn here(&self, sha: &Commit) -> bool {
        let object = format!("{sha}^{{commit}}");
        util::run(self.dir, "git", &["cat-file", "-e", &object])
            .is_ok_and(|output| output.status.success())
    }
}

/// Each address once: a commit cited ten times is one request.
struct Cached<'a> {
    net: &'a dyn Net,
    seen: RefCell<BTreeMap<String, Result<Fetched, Outage>>>,
}

impl Cached<'_> {
    fn once(
        &self,
        key: String,
        fetch: impl FnOnce() -> Result<Fetched, Outage>,
    ) -> Result<Fetched, Outage> {
        if let Some(known) = self.seen.borrow().get(&key) {
            return known.clone();
        }
        let answer = fetch();
        self.seen.borrow_mut().insert(key, answer.clone());
        answer
    }

    fn api(&self, path: &str) -> Result<Fetched, Outage> {
        self.once(format!("api {path}"), || self.net.api(path))
    }

    fn file(&self, depot: &Depot, sha: &Commit, path: &str) -> Result<Fetched, Outage> {
        self.once(format!("file {depot} {sha} {path}"), || {
            self.net.file(depot, sha, path)
        })
    }

    fn web(&self, url: &str) -> Result<Fetched, Outage> {
        self.once(format!("web {url}"), || self.net.web(url))
    }

    fn here(&self, sha: &Commit) -> bool {
        self.net.here(sha)
    }
}

/// What one control found.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    Held,
    Broken(String),
}

/// Stops the run: nothing else would answer either.
struct Offline;

struct Tally<'a> {
    report: &'a mut Report,
    held: usize,
    outages: usize,
}

impl Tally<'_> {
    fn record(&mut self, place: &str, result: Result<Verdict, Outage>) -> Result<(), Offline> {
        match result {
            Ok(Verdict::Held) => self.held = self.held.saturating_add(1),
            Ok(Verdict::Broken(why)) => self.report.error(format!("{place} : {why}")),
            Err(Outage::Refused(why)) => {
                self.outages = self.outages.saturating_add(1);
                self.report
                    .warn(format!("{place} : panne, non conclu : {why}"));
            }
            Err(Outage::Network(why)) => {
                self.outages = self.outages.saturating_add(1);
                self.report.warn(format!("{place} : panne, arrêt : {why}"));
                return Err(Offline);
            }
        }
        Ok(())
    }
}

pub fn run(dir: &Path) -> util::Result<ExitCode> {
    let dir = util::absolute(dir);
    if !dir.is_dir() {
        return Err(format!("{} : dossier absent", dir.display()));
    }
    let folder = Folder::load(&dir)?;
    let mut report = Report::new();
    report.section(&format!("recherche verifie : {}", dir.display()));
    let code = verify_folder(
        &folder,
        upstream(&dir).as_ref(),
        &Tools::detect(&dir),
        &mut report,
    );
    Ok(ExitCode::from(code))
}

/// `origin` of this folder, where `compare_a` lives.
fn upstream(dir: &Path) -> Option<Depot> {
    let url = util::stdout_of(dir, "git", &["remote", "get-url", "origin"])?;
    let (_, path) = url.rsplit_once("github.com")?;
    let path = path.trim_start_matches([':', '/']);
    Depot::try_from(path.trim_end_matches(".git").to_owned()).ok()
}

/// 0 when everything held, 1 when a sheet is wrong or there is none, 2 when the
/// tool could not tell. A wrong sheet is reported even when the network failed
/// afterwards. Nothing checked is not a pass.
fn verify_folder(folder: &Folder, local: Option<&Depot>, net: &dyn Net, report: &mut Report) -> u8 {
    if folder.sheets.is_empty() {
        report.error(
            "aucune fiche à vérifier : attendu au moins une fiche recherche/<sous_systeme>.toml",
        );
        return 1;
    }
    let cached = Cached {
        net,
        seen: RefCell::new(BTreeMap::new()),
    };
    let before = report.count(Level::Error);
    let mut tally = Tally {
        report,
        held: 0,
        outages: 0,
    };
    for sheet in &folder.sheets {
        match &sheet.parsed {
            Err(error) => tally.report.error(error.clone()),
            Ok(fiche) => {
                if verify_sheet(fiche, &sheet.file, local, &cached, &mut tally).is_err() {
                    break;
                }
            }
        }
    }
    let (held, outages) = (tally.held, tally.outages);
    let failed = report.count(Level::Error).saturating_sub(before);
    println!(
        "\n{held} contrôle(s) tenu(s), {failed} échec(s), {outages} panne(s) sur {} fiche(s)",
        folder.sheets.len()
    );
    if failed > 0 {
        1
    } else if outages > 0 {
        println!("Panne de l'outil ou du réseau : rien n'est conclu sur ce qui n'a pas répondu.");
        OUTAGE_CODE
    } else {
        0
    }
}

fn verify_sheet(
    fiche: &Fiche,
    file: &str,
    local: Option<&Depot>,
    net: &Cached<'_>,
    tally: &mut Tally<'_>,
) -> Result<(), Offline> {
    let compared = match local {
        Some(depot) => commit_exists(depot, &fiche.compare_a, net),
        None => Err(Outage::Refused(
            "le dépôt amont de ce dossier est inconnu (git remote origin)".to_owned(),
        )),
    };
    tally.record(&format!("{file} : compare_a"), compared)?;
    for reference in &fiche.references {
        let place = format!("{file} : référence {}", reference.id);
        tally.record(&place, reference_holds(reference, net))?;
    }
    for lecon in &fiche.lecons {
        let place = format!("{file} : {}", lecon.id);
        tally.record(&place, citation_holds(lecon, net))?;
    }
    Ok(())
}

fn commit_exists(depot: &Depot, sha: &Commit, net: &Cached<'_>) -> Result<Verdict, Outage> {
    Ok(match net.api(&format!("repos/{depot}/commits/{sha}"))? {
        Fetched::Body(body) => match commit_named(&body) {
            Some(named) if named == sha.to_string() => Verdict::Held,
            Some(named) => {
                return Err(Outage::Refused(format!(
                    "la réponse pour le commit {sha} nomme {named}"
                )));
            }
            // A captive portal also answers 2xx. Nothing that follows on this
            // network can be believed, in either direction.
            None => {
                return Err(Outage::Network(format!(
                    "la réponse pour le commit {sha} n'est pas celle de GitHub"
                )));
            }
        },
        Fetched::Missing if net.here(sha) => Verdict::Broken(format!(
            "le commit {sha} n'existe pas chez {depot} ; il est dans ce clone : publie-le, ou cite un commit publié"
        )),
        Fetched::Missing => Verdict::Broken(format!("le commit {sha} n'existe pas chez {depot}")),
    })
}

/// The `sha` of a GitHub API answer, or nothing if it is not one.
fn commit_named(body: &[u8]) -> Option<String> {
    let answer = serde_json::from_slice::<Json>(body).ok()?;
    answer.get("sha")?.as_str().map(str::to_owned)
}

fn reference_holds(reference: &Reference, net: &Cached<'_>) -> Result<Verdict, Outage> {
    if let (Some(depot), Some(sha)) = (&reference.depot, &reference.commit) {
        return commit_exists(depot, sha, net);
    }
    let Some(document) = &reference.document else {
        return Ok(Verdict::Held);
    };
    let body = match net.web(document)? {
        Fetched::Body(body) => body,
        Fetched::Missing => return Ok(Verdict::Broken(format!("{document} n'existe plus"))),
    };
    Ok(match &reference.empreinte {
        Some(wanted) => {
            let found = crate::certify::hex(&Sha256::digest(&body));
            if found == wanted.hex() {
                Verdict::Held
            } else {
                Verdict::Broken(format!(
                    "{document} a changé : empreinte sha256:{found}, attendu {}",
                    wanted.hex()
                ))
            }
        }
        None => Verdict::Held,
    })
}

fn citation_holds(lecon: &Lecon, net: &Cached<'_>) -> Result<Verdict, Outage> {
    let citation = &lecon.citation;
    let body = |fetched: Fetched, what: &str| match fetched {
        Fetched::Body(body) => Ok(String::from_utf8_lossy(&body).into_owned()),
        Fetched::Missing => Err(Verdict::Broken(format!("{what} n'existe pas en amont"))),
    };
    Ok(match &lecon.lien {
        Lien::Blob {
            depot,
            sha,
            chemin,
            debut,
            fin,
        } => match body(net.file(depot, sha, chemin)?, chemin) {
            Ok(text) => at_lines(&text, citation, *debut, *fin),
            Err(verdict) => verdict,
        },
        Lien::Texte { document, ligne } => match body(net.web(document)?, document) {
            Ok(text) => at_lines(&text, citation, *ligne, None),
            Err(verdict) => verdict,
        },
        Lien::Page { url } => match body(net.web(url)?, url) {
            Ok(html) => in_page(&html, citation),
            Err(verdict) => verdict,
        },
        Lien::Commit { depot, sha } => {
            let path = format!("repos/{depot}/commits/{sha}");
            match body(net.api(&path)?, &format!("le commit {sha}")) {
                Ok(json) => in_commit(&json, citation)?,
                Err(verdict) => verdict,
            }
        }
        Lien::Issue { depot, numero } => {
            let issue = format!("repos/{depot}/issues/{numero}");
            let comments = format!("{issue}/comments?per_page={COMMENTS_PER_PAGE}");
            match body(net.api(&issue)?, &format!("l'issue {numero}")) {
                Ok(json) => {
                    let replies = match net.api(&comments)? {
                        Fetched::Body(body) => String::from_utf8_lossy(&body).into_owned(),
                        Fetched::Missing => String::new(),
                    };
                    in_issue(&json, &replies, citation)?
                }
                Err(verdict) => verdict,
            }
        }
    })
}

/// Spaces collapsed and typographic quotes made plain: a citation is compared
/// on its words, not on how a page typeset them.
fn normalise(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(['’', '‘'], "'")
        .replace(['“', '”'], "\"")
}

/// The citation at this line, or starting on it and wrapping past it; with a
/// range, anywhere in the range.
fn at_lines(text: &str, citation: &str, first: u32, last: Option<u32>) -> Verdict {
    let wanted = normalise(citation);
    let lines: Vec<&str> = text.lines().collect();
    let start = usize::try_from(first)
        .unwrap_or(usize::MAX)
        .saturating_sub(1);
    let end = last.map_or(
        start.saturating_add(WRAPPED_LINES).saturating_add(1),
        |last| usize::try_from(last).unwrap_or(usize::MAX),
    );
    let window = lines
        .get(start..end.min(lines.len()))
        .unwrap_or_default()
        .join("\n");
    let window = normalise(&window);
    let opening = normalise(lines.get(start).copied().unwrap_or_default());
    let held = match window.find(&wanted) {
        Some(at) => last.is_some() || at < opening.len(),
        None => false,
    };
    if held {
        return Verdict::Held;
    }
    let elsewhere = lines
        .iter()
        .position(|line| normalise(line).contains(&wanted))
        .map(|at| at.saturating_add(1));
    Verdict::Broken(match elsewhere {
        Some(line) => format!("citation absente de la ligne {first} : elle est à la ligne {line}"),
        None => format!("citation absente de la ligne {first} et du fichier"),
    })
}

fn contains(haystack: &str, citation: &str, place: &str) -> Verdict {
    if normalise(haystack).contains(&normalise(citation)) {
        Verdict::Held
    } else {
        Verdict::Broken(format!("citation absente de {place}"))
    }
}

/// The text of a page. Tags go without a space: they often split a sentence.
fn page_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some((before, after)) = rest.split_once('<') {
        text.push_str(before);
        rest = after.split_once('>').map_or("", |(_, on)| on);
    }
    text.push_str(rest);
    [
        ("&nbsp;", " "),
        ("&#160;", " "),
        ("&quot;", "\""),
        ("&#34;", "\""),
        ("&apos;", "'"),
        ("&#39;", "'"),
        ("&#039;", "'"),
        ("&#x27;", "'"),
        ("&rsquo;", "'"),
        ("&#8217;", "'"),
        ("&lsquo;", "'"),
        ("&#8216;", "'"),
        ("&ldquo;", "\""),
        ("&#8220;", "\""),
        ("&rdquo;", "\""),
        ("&#8221;", "\""),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&amp;", "&"),
    ]
    .iter()
    .fold(text, |text, (entity, plain)| text.replace(entity, plain))
}

fn in_page(html: &str, citation: &str) -> Verdict {
    contains(&page_text(html), citation, "la page")
}

fn field<'a>(json: &'a Json, pointer: &str) -> &'a str {
    json.pointer(pointer)
        .and_then(Json::as_str)
        .unwrap_or_default()
}

fn parse(json: &str) -> Result<Json, Outage> {
    serde_json::from_str(json)
        .map_err(|error| Outage::Refused(format!("réponse illisible : {error}")))
}

/// In the message or in the patch of a commit. A patch's lines lose their
/// first column, the `+`, `-` or space of the diff.
fn in_commit(json: &str, citation: &str) -> Result<Verdict, Outage> {
    let commit = parse(json)?;
    let mut text = field(&commit, "/commit/message").to_owned();
    let files = commit.get("files").and_then(Json::as_array);
    for file in files.map_or(&[][..], Vec::as_slice) {
        for line in field(file, "/patch").lines() {
            text.push('\n');
            text.push_str(line.get(1..).unwrap_or_default());
        }
    }
    Ok(contains(
        &text,
        citation,
        "ce commit, message et patch compris",
    ))
}

/// In the title, the body, or the first page of comments.
fn in_issue(json: &str, comments: &str, citation: &str) -> Result<Verdict, Outage> {
    let issue = parse(json)?;
    let mut text = format!("{}\n{}", field(&issue, "/title"), field(&issue, "/body"));
    if !comments.is_empty() {
        for comment in parse(comments)?.as_array().map_or(&[][..], Vec::as_slice) {
            text.push('\n');
            text.push_str(field(comment, "/body"));
        }
    }
    Ok(contains(
        &text,
        citation,
        "l'issue et ses 100 premiers commentaires",
    ))
}

#[cfg(test)]
mod tests {
    use super::super::Sheet;
    use super::super::fiche::{self, Perimetre};
    use super::*;

    const SHA: &str = "2f01c3c88f361aeb9ebbf2cc9b247ee230dd815e";
    const SHA_B: &str = "6770a99f27ad6f15763b85094ea8145786dd8da5";

    /// Answers from a table; whatever it does not know does not exist, and
    /// `offline` stands for a network that never answers.
    #[derive(Default)]
    struct Fake {
        answers: BTreeMap<String, Result<Fetched, Outage>>,
        offline: bool,
        local: Option<String>,
        calls: RefCell<usize>,
    }

    impl Fake {
        fn with(mut self, key: &str, body: &str) -> Self {
            self.answers
                .insert(key.to_owned(), Ok(Fetched::Body(body.as_bytes().to_vec())));
            self
        }

        fn refusing(mut self, key: &str) -> Self {
            self.answers
                .insert(key.to_owned(), Err(Outage::Refused("HTTP 403".to_owned())));
            self
        }

        fn answer(&self, key: &str) -> Result<Fetched, Outage> {
            let calls = self.calls.borrow().saturating_add(1);
            self.calls.replace(calls);
            if self.offline {
                return Err(Outage::Network("pas de réseau".to_owned()));
            }
            self.answers
                .get(key)
                .cloned()
                .unwrap_or(Ok(Fetched::Missing))
        }
    }

    impl Net for Fake {
        fn api(&self, path: &str) -> Result<Fetched, Outage> {
            self.answer(path)
        }
        fn file(&self, depot: &Depot, sha: &Commit, path: &str) -> Result<Fetched, Outage> {
            self.answer(&format!("{depot}@{sha}/{path}"))
        }
        fn web(&self, url: &str) -> Result<Fetched, Outage> {
            self.answer(url)
        }
        fn here(&self, sha: &Commit) -> bool {
            self.local.as_deref() == Some(sha.to_string().as_str())
        }
    }

    fn sheet(references: &str, lessons: &str) -> Folder {
        let text = format!(
            "sous_systeme = \"reconnexion\"\nniveau = 3\nquestion = \"q\"\ncompare_a = \"{SHA}\"\n{references}{lessons}[saturation]\nsources = []\nderniere_lecon = \"a\"\nnote = \"n\"\n[saturation.recoupe]\n[cout]\nminutes = 1\nreferences = 1\nsondes_executees = 0\nnote = \"n\"\n"
        );
        Folder {
            registre: fiche::read(crate::embedded::recherche::SOUS_SYSTEMES, "r.toml").unwrap(),
            sheets: vec![Sheet {
                file: "recherche/reconnexion.toml".to_owned(),
                stem: "reconnexion".to_owned(),
                parsed: fiche::read(&text, "reconnexion.toml"),
            }],
            sources: None,
            perimetre: Perimetre::Commun,
        }
    }

    fn lesson(lien: &str, citation: &str) -> String {
        format!(
            "[[lecon]]\nid = \"l\"\nsource = \"a\"\nlien = \"{lien}\"\ncitation = \"{citation}\"\nenonce = \"e\"\ncategorie = \"physique\"\ndecision = \"garder\"\netat = \"couvert\"\ngarantie = [\"C2\"]\n"
        )
    }

    const REPOSITORY: &str = "[[reference]]\nid = \"a\"\nordre = 1\nsorte = \"reel\"\nconsulte = [\"x\"]\ndepot = \"o/r\"\ncommit = \"6770a99f27ad6f15763b85094ea8145786dd8da5\"\n";

    fn commit(sha: &str) -> String {
        serde_json::json!({ "sha": sha }).to_string()
    }

    fn base() -> Fake {
        Fake::default()
            .with(&format!("repos/o/soiree/commits/{SHA}"), &commit(SHA))
            .with(&format!("repos/o/r/commits/{SHA_B}"), &commit(SHA_B))
    }

    fn outcome(folder: &Folder, net: &Fake) -> (u8, Vec<String>) {
        let local = Depot::try_from("o/soiree".to_owned()).unwrap();
        let mut report = Report::quiet();
        let code = verify_folder(folder, Some(&local), net, &mut report);
        let lines = report.lines.into_iter().map(|(_, line)| line).collect();
        (code, lines)
    }

    fn blob(sha: &str, anchor: &str) -> String {
        format!("https://github.com/o/r/blob/{sha}/a/b.ts#{anchor}")
    }

    fn file_key(sha: &str) -> String {
        format!("o/r@{sha}/a/b.ts")
    }

    #[test]
    fn a_citation_at_its_line_holds() {
        let folder = sheet(REPOSITORY, &lesson(&blob(SHA, "L2"), "deux trois"));
        let net = base().with(&file_key(SHA), "un\ndeux trois\nquatre\n");
        assert_eq!(outcome(&folder, &net), (0, Vec::new()));
    }

    #[test]
    fn a_citation_on_another_line_says_where_it_went() {
        let folder = sheet(REPOSITORY, &lesson(&blob(SHA, "L1"), "deux trois"));
        let net = base().with(&file_key(SHA), "un\ndeux trois\nquatre\n");
        let (code, lines) = outcome(&folder, &net);
        assert_eq!(code, 1);
        assert!(lines[0].contains("elle est à la ligne 2"), "{lines:?}");
    }

    #[test]
    fn a_citation_that_is_nowhere_fails() {
        let folder = sheet(REPOSITORY, &lesson(&blob(SHA, "L1"), "inventée"));
        let net = base().with(&file_key(SHA), "un\n");
        let (code, lines) = outcome(&folder, &net);
        assert_eq!(code, 1);
        assert!(lines[0].contains("et du fichier"), "{lines:?}");
    }

    #[test]
    fn a_dead_link_fails_and_a_missing_commit_fails() {
        let folder = sheet(REPOSITORY, &lesson(&blob(SHA, "L1"), "un"));
        let (code, lines) = outcome(&folder, &base());
        assert_eq!(code, 1);
        assert!(lines[0].contains("n'existe pas en amont"), "{lines:?}");

        let net = Fake::default().with(&format!("repos/o/soiree/commits/{SHA}"), &commit(SHA));
        let (code, lines) = outcome(&sheet(REPOSITORY, ""), &net);
        assert_eq!(code, 1);
        assert!(lines[0].contains("référence a"), "{lines:?}");
        assert!(lines[0].contains("n'existe pas chez o/r"), "{lines:?}");

        let net = Fake::default().with(&format!("repos/o/r/commits/{SHA_B}"), &commit(SHA_B));
        let (code, lines) = outcome(&sheet(REPOSITORY, ""), &net);
        assert_eq!(code, 1);
        assert!(lines[0].contains("compare_a"), "{lines:?}");
    }

    #[test]
    fn a_commit_that_is_only_here_says_so() {
        let net = Fake {
            local: Some(SHA.to_owned()),
            ..Fake::default()
        }
        .with(&format!("repos/o/r/commits/{SHA_B}"), &commit(SHA_B));
        let (code, lines) = outcome(&sheet(REPOSITORY, ""), &net);
        assert_eq!(code, 1);
        assert!(lines[0].contains("il est dans ce clone"), "{lines:?}");
    }

    #[test]
    fn a_wrapped_citation_starts_on_its_line_and_a_range_is_read_whole() {
        let text = "un\ndeux trois\nquatre cinq\nsix\n";
        assert_eq!(at_lines(text, "trois quatre", 2, None), Verdict::Held);
        assert_ne!(at_lines(text, "quatre cinq", 2, None), Verdict::Held);
        assert_eq!(at_lines(text, "quatre cinq", 2, Some(3)), Verdict::Held);
        assert_ne!(at_lines(text, "six", 1, Some(3)), Verdict::Held);
        assert_ne!(at_lines(text, "un", 99, None), Verdict::Held);
        assert_eq!(at_lines("l’a  dit\n", "l'a dit", 1, None), Verdict::Held);
    }

    #[test]
    fn a_commit_citation_is_read_in_its_message_and_its_patch() {
        let json = serde_json::json!({
            "sha": SHA_B,
            "commit": {"message": "fix: a\n\nthe long\nsentence"},
            "files": [{"patch": "@@ -1 +1 @@\n-old line\n+new line"}]
        })
        .to_string();
        let link = format!("https://github.com/o/r/commit/{SHA_B}");
        for (citation, code) in [("the long sentence", 0), ("new line", 0), ("nowhere", 1)] {
            let folder = sheet(REPOSITORY, &lesson(&link, citation));
            let net = base().with(&format!("repos/o/r/commits/{SHA_B}"), &json);
            assert_eq!(outcome(&folder, &net).0, code, "{citation}");
        }
    }

    #[test]
    fn an_issue_citation_is_read_in_the_body_and_the_comments() {
        let issue = serde_json::json!({"title": "le titre", "body": "le corps"}).to_string();
        let comments = serde_json::json!([{"body": "une réponse"}]).to_string();
        for (citation, code) in [("le corps", 0), ("une réponse", 0), ("autre chose", 1)] {
            let folder = sheet(
                REPOSITORY,
                &lesson("https://github.com/o/r/issues/7", citation),
            );
            let net = base()
                .with("repos/o/r/issues/7", &issue)
                .with("repos/o/r/issues/7/comments?per_page=100", &comments);
            assert_eq!(outcome(&folder, &net).0, code, "{citation}");
        }
        let folder = sheet(REPOSITORY, &lesson("https://github.com/o/r/issues/7", "x"));
        assert_eq!(outcome(&folder, &base()).0, 1);
    }

    const DOCUMENT: &str = "[[reference]]\nid = \"a\"\nordre = 1\nsorte = \"normes\"\nconsulte = [\"x\"]\ndocument = \"https://x.org/d.txt\"\nvu_le = \"2026-11-01\"\nempreinte = \"sha256:HASH\"\n";

    fn document(body: &str) -> String {
        let hash = crate::certify::hex(&Sha256::digest(body.as_bytes()));
        DOCUMENT.replace("HASH", &hash)
    }

    #[test]
    fn a_document_keeps_its_fingerprint_and_its_lines() {
        let body = "un\ndeux\n";
        let folder = sheet(&document(body), &lesson("https://x.org/d.txt#L2", "deux"));
        let net = base().with("https://x.org/d.txt", body);
        assert_eq!(outcome(&folder, &net), (0, Vec::new()));
        let net = base().with("https://x.org/d.txt", "un\ndeux\nmodifié\n");
        let (code, lines) = outcome(&folder, &net);
        assert_eq!(code, 1);
        assert!(lines[0].contains("a changé"), "{lines:?}");
    }

    #[test]
    fn a_page_is_read_as_text() {
        let page = "[[reference]]\nid = \"a\"\nordre = 1\nsorte = \"retours\"\nconsulte = [\"x\"]\ndocument = \"https://x.org/p/\"\nvu_le = \"2026-11-01\"\n";
        let html = "<p>les clients <em>doivent</em> attendre l&rsquo;hôte</p>";
        let folder = sheet(
            page,
            &lesson("https://x.org/p/", "clients doivent attendre l'hôte"),
        );
        let net = base().with("https://x.org/p/", html);
        assert_eq!(outcome(&folder, &net), (0, Vec::new()));
        let folder = sheet(page, &lesson("https://x.org/p/", "autre"));
        assert_eq!(outcome(&folder, &net).0, 1);
        let net = base();
        assert_eq!(outcome(&sheet(page, ""), &net).0, 1);
    }

    #[test]
    fn a_missing_network_is_an_outage_and_stops_at_once() {
        let folder = sheet(REPOSITORY, &lesson(&blob(SHA, "L1"), "un"));
        let net = Fake {
            offline: true,
            ..Fake::default()
        };
        let (code, lines) = outcome(&folder, &net);
        assert_eq!(code, OUTAGE_CODE);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("panne"), "{lines:?}");
        assert_eq!(*net.calls.borrow(), 1);
    }

    #[test]
    fn a_refused_request_is_not_concluded_and_the_rest_goes_on() {
        let folder = sheet(REPOSITORY, &lesson(&blob(SHA, "L1"), "un"));
        let net = base().refusing(&file_key(SHA));
        let (code, lines) = outcome(&folder, &net);
        assert_eq!(code, OUTAGE_CODE);
        assert!(lines[0].contains("panne, non conclu"), "{lines:?}");
    }

    #[test]
    fn a_wrong_sheet_outranks_an_outage() {
        let body = "un\n";
        let folder = sheet(REPOSITORY, &lesson(&blob(SHA, "L9"), "un"));
        let net = base()
            .with(&file_key(SHA), body)
            .refusing(&format!("repos/o/soiree/commits/{SHA}"));
        assert_eq!(outcome(&folder, &net).0, 1);
    }

    #[test]
    fn a_2xx_that_is_not_the_commit_concludes_nothing() {
        for (body, expected) in [
            ("<html>Connectez-vous</html>", "panne, arrêt"),
            ("{}", "panne, arrêt"),
            (&commit(SHA_B), "panne, non conclu"),
        ] {
            let net = base().with(&format!("repos/o/soiree/commits/{SHA}"), body);
            let (code, lines) = outcome(&sheet(REPOSITORY, ""), &net);
            assert_eq!(code, OUTAGE_CODE, "{body} : {lines:?}");
            assert!(lines[0].contains(expected), "{lines:?}");
        }
    }

    #[test]
    fn no_sheet_fails_without_asking_anything() {
        let mut folder = sheet(REPOSITORY, "");
        folder.sheets.clear();
        let net = base();
        let (code, lines) = outcome(&folder, &net);
        assert_eq!(code, 1);
        assert!(lines[0].contains("aucune fiche"), "{lines:?}");
        assert_eq!(*net.calls.borrow(), 0);
    }

    #[test]
    fn an_unknown_upstream_cannot_conclude_on_compare_a() {
        let folder = sheet(REPOSITORY, "");
        let mut report = Report::quiet();
        let code = verify_folder(&folder, None, &base(), &mut report);
        assert_eq!(code, OUTAGE_CODE);
    }

    #[test]
    fn each_address_is_asked_once() {
        let folder = sheet(
            REPOSITORY,
            &lesson(&format!("https://github.com/o/r/commit/{SHA_B}"), "x"),
        );
        let net = base().with(
            &format!("repos/o/r/commits/{SHA_B}"),
            &format!("{{\"sha\":\"{SHA_B}\",\"commit\":{{\"message\":\"x\"}}}}"),
        );
        assert_eq!(outcome(&folder, &net).0, 0);
        assert_eq!(*net.calls.borrow(), 2);
    }

    #[test]
    fn the_tools_are_read_for_what_they_say() {
        assert!(matches!(
            gh_failure("gh: Not Found (HTTP 404)\n"),
            GhFailure::Missing
        ));
        assert!(matches!(
            gh_failure("gh: No commit found for SHA (HTTP 422)"),
            GhFailure::Missing
        ));
        assert!(matches!(
            gh_failure("gh: API rate limit exceeded (HTTP 403)"),
            GhFailure::Outage(Outage::Refused(_))
        ));
        assert!(matches!(
            gh_failure("error connecting to api.github.com"),
            GhFailure::Outage(Outage::Network(_))
        ));
        assert_eq!(
            curl_answer("u", b"corps\nsuite\n200"),
            Ok(Fetched::Body(b"corps\nsuite".to_vec()))
        );
        assert_eq!(curl_answer("u", b"perdu\n404"), Ok(Fetched::Missing));
        assert_eq!(
            curl_answer("https://api.github.com/repos/o/r/commits/a", b"{}\n422"),
            Ok(Fetched::Missing)
        );
        assert!(matches!(
            curl_answer("https://x.org/d", b"{}\n422"),
            Err(Outage::Refused(_))
        ));
        assert!(matches!(
            curl_answer("u", b"\n503"),
            Err(Outage::Refused(_))
        ));
    }

    #[test]
    fn upstream_is_read_from_either_form_of_remote() {
        let dir = crate::testdir::fresh("recherche-amont");
        util::run(&dir, "git", &["init", "-q"]).unwrap();
        assert_eq!(upstream(&dir), None);
        for url in [
            "https://github.com/o/soiree.git",
            "git@github.com:o/soiree.git",
            "https://github.com/o/soiree",
        ] {
            util::run(&dir, "git", &["remote", "remove", "origin"]).unwrap();
            util::run(&dir, "git", &["remote", "add", "origin", url]).unwrap();
            assert_eq!(upstream(&dir).unwrap().to_string(), "o/soiree", "{url}");
        }
    }
}
