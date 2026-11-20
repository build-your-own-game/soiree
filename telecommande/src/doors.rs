//! Les portes vers le monde extérieur : la console fait ce que la machine
//! demande ([`cartouche::Ask`]) et lui rend la réponse.
//!
//! - **La date** : celle de la montre de l'hôte.
//! - **Internet** : une adresse en `https://`, ou `http://localhost`. Un
//!   secret ne quitte jamais la console : `{NOM}`, dans un en-tête, devient
//!   la valeur de `SOIREE_SECRET_NOM`, et seulement vers l'adresse que
//!   `SOIREE_SECRET_NOM_POUR` nomme.
//! - **Un contenu** : les octets que la console garde sous ce ticket.
//! - **Les sauvegardes** : un fichier par nom, dans `.soiree/sauvegardes/`.
//!
//! Tout se fait sur place, dans l'ordre des demandes, sauf internet, qui
//! répond quand il peut. La réponse sort de la salle comme ce qui vient des
//! pages : [`Event::Answer`].

use std::collections::VecDeque;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use cartouche::{
    Answer, Ask, Date, Method, SAVE_MAX, SAVE_NAME_MAX, SAVES_MAX, WEB_MAX, Want, Web,
};
use ureq::http::Uri;

use crate::host::Event;
use crate::store::Store;

/// Internet a tant de temps pour répondre.
const WEB_PATIENCE: Duration = Duration::from_secs(30);
/// Au plus tant de questions à internet par minute : un jeu qui s'emballe ne
/// vide pas le compte de l'hôte.
const WEB_PER_MINUTE: usize = 30;
const MINUTE: Duration = Duration::from_secs(60);
/// La variable qui porte le secret `NOM` : `SOIREE_SECRET_NOM`. Celle qui
/// nomme l'adresse où il a le droit d'aller : `SOIREE_SECRET_NOM_POUR`.
const SECRET: &str = "SOIREE_SECRET_";
const SECRET_FOR: &str = "_POUR";

/// Les portes de la console.
pub(crate) struct Doors {
    answers: mpsc::Sender<Event>,
    store: Arc<Store>,
    saves: Saves,
    agent: ureq::Agent,
    /// Quand les dernières questions à internet sont parties.
    sent: VecDeque<Instant>,
}

impl Doors {
    /// Les réponses partent par `answers` ; les sauvegardes vont dans `saves`.
    pub(crate) fn new(answers: mpsc::Sender<Event>, store: Arc<Store>, saves: PathBuf) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(WEB_PATIENCE))
            // Une redirection emmènerait les en-têtes, secrets compris, à une
            // autre adresse : elle revient telle quelle à la machine.
            .max_redirects(0)
            // Un code d'erreur est une réponse : la machine le lit.
            .http_status_as_error(false)
            .build()
            .into();
        Self {
            answers,
            store,
            saves: Saves { folder: saves },
            agent,
            sent: VecDeque::new(),
        }
    }

    /// Fait ce que la machine demande. La réponse porte `serial`, le numéro
    /// que la console a donné à cette demande.
    pub(crate) fn open(&mut self, serial: u64, ask: Ask, now: Instant) {
        let id = ask.id;
        let done = match ask.want {
            Want::Date => Ok(Answer::date(today())),
            Want::Content(content) => Ok(self
                .store
                .get(&content.name())
                .map_or_else(Answer::nothing, |(bytes, _)| Answer::bytes(bytes))),
            Want::Save { name, bytes } => self
                .saves
                .save(&name, &bytes)
                .map(|()| Answer::done())
                .map_err(|why| format!("« {name} » n'est pas gardé : {why}")),
            Want::Load(name) => self
                .saves
                .load(&name)
                .map(|bytes| bytes.map_or_else(Answer::nothing, Answer::bytes))
                .map_err(|why| format!("« {name} » ne se relit pas : {why}")),
            Want::Erase(name) => self
                .saves
                .erase(&name)
                .map(|()| Answer::done())
                .map_err(|why| format!("« {name} » ne s'oublie pas : {why}")),
            Want::Saves => self
                .saves
                .names()
                .map(Answer::names)
                .map_err(|why| format!("les sauvegardes ne se listent pas : {why}")),
            Want::Web(web) => {
                if self.may_send(now) {
                    let (agent, answers) = (self.agent.clone(), self.answers.clone());
                    let _: std::thread::JoinHandle<()> = std::thread::spawn(move || {
                        let answer = settle(id, fetch(&agent, &web, &secret));
                        let _: Result<(), mpsc::SendError<Event>> =
                            answers.send(Event::Answer(serial, answer));
                    });
                    return;
                }
                Err(format!(
                    "plus de {WEB_PER_MINUTE} questions à internet en une minute"
                ))
            }
        };
        let _: Result<(), mpsc::SendError<Event>> =
            self.answers.send(Event::Answer(serial, settle(id, done)));
    }

    /// Une question de plus peut-elle partir vers internet maintenant ?
    fn may_send(&mut self, now: Instant) -> bool {
        while self
            .sent
            .front()
            .is_some_and(|sent| now.saturating_duration_since(*sent) >= MINUTE)
        {
            let _: Option<Instant> = self.sent.pop_front();
        }
        if self.sent.len() >= WEB_PER_MINUTE {
            return false;
        }
        self.sent.push_back(now);
        true
    }
}

/// La réponse à remettre à la machine. Une demande que la console n'a pas pu
/// faire reste sans réponse, et l'hôte lit pourquoi.
fn settle(id: u32, done: Result<Answer, String>) -> Answer {
    done.unwrap_or_else(|why| {
        eprintln!("Demande {id} de la machine, sans réponse : {why}.");
        Answer::nothing()
    })
}

/// La date et l'heure, à la montre de l'hôte.
fn today() -> Date {
    let seconds = chrono::Local::now().naive_local().and_utc().timestamp();
    Date::at(u64::try_from(seconds).unwrap_or(0))
}

/// Un secret que l'hôte a confié à la console, et l'adresse où il a le droit
/// d'aller.
struct Secret {
    value: String,
    host: String,
}

/// Le secret de ce nom, dans les variables que l'hôte a données.
fn secret(name: &str) -> Option<Secret> {
    Some(Secret {
        value: std::env::var(format!("{SECRET}{name}")).ok()?,
        host: std::env::var(format!("{SECRET}{name}{SECRET_FOR}")).ok()?,
    })
}

/// Pose la question à internet et lit sa réponse.
fn fetch(
    agent: &ureq::Agent,
    web: &Web,
    secret: &dyn Fn(&str) -> Option<Secret>,
) -> Result<Answer, String> {
    if web.body().len() > WEB_MAX {
        return Err(format!("l'envoi dépasse {WEB_MAX} octets"));
    }
    let address = Address::read(web.url())?;
    let headers = web
        .headers()
        .iter()
        .map(|(name, value)| Ok((name.as_str(), fill(value, &address, secret)?)))
        .collect::<Result<Vec<(&str, String)>, String>>()?;
    let response = match web.method() {
        Method::Get => headers
            .iter()
            .fold(agent.get(web.url()), |request, (name, value)| {
                request.header(*name, value)
            })
            .call(),
        Method::Post => headers
            .iter()
            .fold(agent.post(web.url()), |request, (name, value)| {
                request.header(*name, value)
            })
            .send(web.body()),
    }
    .map_err(|error| format!("{} ne répond pas ({error})", address.host))?;
    let status = response.status().as_u16();
    let body = response
        .into_body()
        .with_config()
        .limit(u64::try_from(WEB_MAX).unwrap_or(u64::MAX))
        .read_to_vec()
        .map_err(|error| format!("la réponse de {} ne se lit pas ({error})", address.host))?;
    Ok(Answer::web(status, body))
}

/// L'adresse d'une question à internet, une fois acceptée.
struct Address {
    host: String,
    /// En `https://` : un secret peut y aller.
    sealed: bool,
}

impl Address {
    /// `https://` partout ; `http://` seulement vers l'ordinateur de l'hôte.
    fn read(url: &str) -> Result<Self, String> {
        let uri: Uri = url
            .parse()
            .map_err(|_| format!("`{url}` n'est pas une adresse"))?;
        let host = uri
            .host()
            .ok_or_else(|| format!("`{url}` ne nomme personne"))?
            .to_ascii_lowercase();
        let local = ["localhost", "127.0.0.1", "[::1]", "::1"].contains(&host.as_str());
        match uri.scheme_str() {
            Some("https") => Ok(Self { host, sealed: true }),
            Some("http") if local => Ok(Self {
                host,
                sealed: false,
            }),
            Some(_) | None => Err(format!(
                "`{url}` : la console n'interroge qu'une adresse en `https://`, ou \
                 `http://localhost`"
            )),
        }
    }
}

/// La valeur d'un en-tête, où chaque `{NOM}` est remplacé par le secret de ce
/// nom. Un secret ne part que chiffré, et vers l'adresse pour laquelle l'hôte
/// l'a confié.
fn fill(
    value: &str,
    address: &Address,
    secret: &dyn Fn(&str) -> Option<Secret>,
) -> Result<String, String> {
    let mut filled = String::new();
    let mut rest = value;
    while let Some((before, after)) = rest.split_once('{') {
        filled.push_str(before);
        let name = after.split_once('}').filter(|(name, _)| is_name(name));
        let Some((name, after)) = name else {
            // Une accolade qui ne nomme pas un secret reste une accolade.
            filled.push('{');
            rest = after;
            continue;
        };
        let Some(secret) = secret(name) else {
            return Err(format!(
                "le jeu demande le secret `{name}`, que la console n'a pas. Pour le lui confier, \
                 dans le dossier du jeu : `fnox set {SECRET}{name}` (sa valeur) et `fnox set \
                 {SECRET}{name}{SECRET_FOR}` (l'adresse où il a le droit d'aller, par exemple \
                 `{}`)",
                address.host
            ));
        };
        if !address.sealed || !secret.host.trim().eq_ignore_ascii_case(&address.host) {
            return Err(format!(
                "le secret `{name}` est confié pour `{}`, en `https://` : il ne part pas vers \
                 `{}`",
                secret.host.trim(),
                address.host
            ));
        }
        filled.push_str(&secret.value);
        rest = after;
    }
    filled.push_str(rest);
    Ok(filled)
}

/// Un nom de secret : des majuscules, des chiffres et des tirets bas.
fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Les sauvegardes d'un jeu : un fichier par nom.
struct Saves {
    folder: PathBuf,
}

impl Saves {
    fn save(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let path = self.path(name)?;
        if bytes.len() > SAVE_MAX {
            return Err(format!(
                "{} octets, plus que les {SAVE_MAX} permis",
                bytes.len()
            ));
        }
        if !path.is_file() && self.names()?.len() >= SAVES_MAX {
            return Err(format!("le jeu garde déjà {SAVES_MAX} sauvegardes"));
        }
        crate::console::write_whole(&path, bytes).map_err(|error| error.to_string())
    }

    /// Les octets gardés sous ce nom. `None` : ce nom ne garde rien.
    fn load(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        match fs::read(self.path(name)?) {
            Ok(bytes) if bytes.len() <= SAVE_MAX => Ok(Some(bytes)),
            Ok(bytes) => Err(format!(
                "{} octets, plus que les {SAVE_MAX} permis",
                bytes.len()
            )),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn erase(&self, name: &str) -> Result<(), String> {
        match fs::remove_file(self.path(name)?) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }

    /// Les noms de tout ce qui est gardé, dans l'ordre.
    fn names(&self) -> Result<Vec<String>, String> {
        let entries = match fs::read_dir(&self.folder) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.to_string()),
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| named(entry.file_name().to_str()?))
            .collect();
        names.sort();
        Ok(names)
    }

    fn path(&self, name: &str) -> Result<PathBuf, String> {
        if name.is_empty() {
            return Err("un nom vide".to_owned());
        }
        if name.len() > SAVE_NAME_MAX {
            return Err(format!("un nom de plus de {SAVE_NAME_MAX} octets"));
        }
        Ok(self.folder.join(file_name(name)))
    }
}

/// Le nom du fichier d'une sauvegarde : les minuscules, les chiffres, `-` et
/// `_` tels quels, tout autre octet écrit `%xx`. Deux noms différents ne
/// donnent jamais le même fichier, même là où « Score » et « score » sont le
/// même fichier.
fn file_name(name: &str) -> String {
    let mut file = String::new();
    for byte in name.bytes() {
        let plain = byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte);
        if plain {
            file.push(char::from(byte));
        } else {
            file.push('%');
            file.push_str(&cartouche::journal::hex(&[byte]));
        }
    }
    file
}

/// Le nom de la sauvegarde que ce fichier garde. `None` si ce fichier n'en
/// est pas une.
fn named(file: &str) -> Option<String> {
    let mut bytes = Vec::new();
    let mut rest = file.chars();
    while let Some(one) = rest.next() {
        if one == '%' {
            let high = rest.next()?.to_digit(16)?;
            let low = rest.next()?.to_digit(16)?;
            bytes.push(u8::try_from(high.saturating_mul(16).saturating_add(low)).ok()?);
        } else {
            bytes.push(u8::try_from(one).ok()?);
        }
    }
    let name = String::from_utf8(bytes).ok()?;
    // Un seul fichier par nom : celui que `file_name` écrit.
    (file_name(&name) == file && !name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;
    use std::time::{SystemTime, UNIX_EPOCH};

    use cartouche::{Content, Kind};

    use super::*;

    fn folder(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "telecommande-portes-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn doors(name: &str) -> (Doors, mpsc::Receiver<Event>, Arc<Store>) {
        let (answers, events) = mpsc::channel();
        let folder = folder(name);
        let store = Arc::new(Store::open(folder.join("contenus")));
        let doors = Doors::new(answers, Arc::clone(&store), folder.join("sauvegardes"));
        (doors, events, store)
    }

    fn next(events: &mpsc::Receiver<Event>) -> (u64, Answer) {
        match events.recv_timeout(Duration::from_secs(10)).unwrap() {
            Event::Answer(serial, answer) => (serial, answer),
            other @ (Event::Joined(..) | Event::Text(..) | Event::Left(_) | Event::Order(_)) => {
                panic!("{other:?}")
            }
        }
    }

    fn no_secret(_: &str) -> Option<Secret> {
        None
    }

    fn judge(name: &str) -> Option<Secret> {
        (name == "JUGE").then(|| Secret {
            value: "s3cr3t".to_owned(),
            host: "Juge.Test".to_owned(),
        })
    }

    #[test]
    fn what_is_kept_comes_back_until_it_is_forgotten() {
        let (mut doors, events, _) = doors("sauvegardes");
        let now = Instant::now();
        doors.open(1, Ask::saves(1), now);
        assert_eq!(next(&events), (1, Answer::names(Vec::new())));
        doors.open(2, Ask::load(2, "Soirée du 12"), now);
        assert_eq!(next(&events), (2, Answer::nothing()));
        doors.open(3, Ask::save(3, "Soirée du 12", vec![1, 2, 3]), now);
        assert_eq!(next(&events), (3, Answer::done()));
        doors.open(4, Ask::save(4, "score", vec![9]), now);
        doors.open(5, Ask::save(5, "Score", vec![8]), now);
        doors.open(6, Ask::load(6, "Soirée du 12"), now);
        assert_eq!(next(&events).1, Answer::done());
        assert_eq!(next(&events).1, Answer::done());
        assert_eq!(next(&events), (6, Answer::bytes(vec![1, 2, 3])));
        // « Score » et « score » sont deux sauvegardes.
        doors.open(7, Ask::load(7, "score"), now);
        assert_eq!(next(&events).1, Answer::bytes(vec![9]));
        doors.open(8, Ask::saves(8), now);
        assert_eq!(
            next(&events).1,
            Answer::names(vec![
                "Score".to_owned(),
                "Soirée du 12".to_owned(),
                "score".to_owned()
            ])
        );
        doors.open(9, Ask::erase(9, "score"), now);
        doors.open(10, Ask::erase(10, "score"), now);
        doors.open(11, Ask::load(11, "score"), now);
        assert_eq!(next(&events).1, Answer::done());
        assert_eq!(next(&events).1, Answer::done());
        assert_eq!(next(&events).1, Answer::nothing());
    }

    #[test]
    fn a_save_that_does_not_fit_is_not_kept() {
        let (mut doors, events, _) = doors("refus");
        let now = Instant::now();
        doors.open(1, Ask::save(1, "", vec![1]), now);
        doors.open(
            2,
            Ask::save(2, &"n".repeat(SAVE_NAME_MAX + 1), vec![1]),
            now,
        );
        doors.open(3, Ask::save(3, "gros", vec![0; SAVE_MAX + 1]), now);
        doors.open(4, Ask::load(4, "../etat"), now);
        for _ in 0..4 {
            assert_eq!(next(&events).1, Answer::nothing());
        }
        for rank in 0..SAVES_MAX {
            doors.saves.save(&format!("s{rank}"), &[1]).unwrap();
        }
        assert!(doors.saves.save("une de trop", &[1]).is_err());
        // Remplacer une sauvegarde qui existe reste possible.
        doors.saves.save("s0", &[2]).unwrap();
        assert_eq!(doors.saves.load("s0").unwrap(), Some(vec![2]));
    }

    #[test]
    fn a_save_keeps_its_name_in_its_file() {
        assert_eq!(file_name("score-2_a"), "score-2_a");
        assert_eq!(file_name("Soirée/.."), "%53oir%c3%a9e%2f%2e%2e");
        for name in ["score", "Soirée du 12", "a%2fb", "..", "🎉"] {
            assert_eq!(named(&file_name(name)).as_deref(), Some(name));
        }
        // Un fichier qui n'est pas écrit comme `file_name` l'écrit n'en est pas une.
        for file in ["", "Score", "%2F", "a%2", "score.tmp", "%ff"] {
            assert_eq!(named(file), None, "{file}");
        }
    }

    #[test]
    fn the_date_and_a_content_come_at_once() {
        let (mut doors, events, store) = doors("date");
        let now = Instant::now();
        doors.open(1, Ask::date(1), now);
        let date = next(&events).1.as_date().unwrap();
        assert!(date.year >= 2025 && (1..=12).contains(&date.month));
        let level = store.put(b"un niveau", "application/json").unwrap();
        doors.open(2, Ask::content(2, level), now);
        assert_eq!(next(&events), (2, Answer::bytes(b"un niveau".to_vec())));
        let gone = Content::of(b"jamais vu", Kind::Other);
        doors.open(3, Ask::content(3, gone), now);
        assert_eq!(next(&events), (3, Answer::nothing()));
    }

    #[test]
    fn the_web_answers_through_the_console() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = server.local_addr().unwrap().port();
        let asked = std::thread::spawn(move || {
            let (mut stream, _) = server.accept().unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            // L'en-tête, puis les quatre octets envoyés.
            while !request.ends_with(b"ping") {
                let read = stream.read(&mut chunk).unwrap();
                assert!(read > 0);
                request.extend_from_slice(&chunk[..read]);
            }
            stream
                .write_all(
                    b"HTTP/1.1 418 Teapot\r\ncontent-length: 4\r\nconnection: close\r\n\r\npong",
                )
                .unwrap();
            String::from_utf8(request).unwrap()
        });
        let (mut doors, events, _) = doors("internet");
        let web = Web::post(&format!("http://127.0.0.1:{port}/juge"), b"ping".to_vec())
            .header("x-jeu", "essai {pas un secret}");
        doors.open(7, Ask::web(1, web), Instant::now());
        // Un code d'erreur est une réponse.
        assert_eq!(next(&events), (7, Answer::web(418, b"pong".to_vec())));
        let request = asked.join().unwrap().to_ascii_lowercase();
        assert!(request.starts_with("post /juge http/1.1\r\n"), "{request}");
        assert!(
            request.contains("x-jeu: essai {pas un secret}\r\n"),
            "{request}"
        );
    }

    #[test]
    fn an_address_the_console_refuses_gets_no_answer() {
        let (mut doors, events, _) = doors("adresses");
        let now = Instant::now();
        for url in [
            "http://exemple.test/",
            "ftp://exemple.test/",
            "pas une adresse",
            "https://",
        ] {
            doors.open(1, Ask::web(1, Web::get(url)), now);
            assert_eq!(next(&events).1, Answer::nothing(), "{url}");
        }
        assert!(Address::read("https://Exemple.Test/a?b=c").unwrap().sealed);
        assert!(!Address::read("http://localhost:11434/api").unwrap().sealed);
        assert!(!Address::read("http://[::1]:8080/").unwrap().sealed);
    }

    #[test]
    fn a_secret_only_goes_where_the_host_sent_it() {
        let to = |url| Address::read(url).unwrap();
        assert_eq!(
            fill("Bearer {JUGE}", &to("https://juge.test/v1"), &judge).unwrap(),
            "Bearer s3cr3t"
        );
        // Ni vers une autre adresse, ni en clair, ni sans que l'hôte l'ait confié.
        assert!(fill("{JUGE}", &to("https://ailleurs.test/"), &judge).is_err());
        assert!(fill("{JUGE}", &to("http://localhost/"), &judge).is_err());
        let missing = fill("{JUGE}", &to("https://juge.test/"), &no_secret).unwrap_err();
        assert!(missing.contains("SOIREE_SECRET_JUGE_POUR"), "{missing}");
        // Ce qui ne nomme pas un secret reste tel quel.
        for value in ["{\"a\":1}", "{juge}", "{", "a}b", "{}"] {
            assert_eq!(
                fill(value, &to("https://juge.test/"), &no_secret).unwrap(),
                value
            );
        }
    }

    #[test]
    fn a_game_that_runs_away_is_held_back() {
        let (mut doors, _, _) = doors("emballement");
        let start = Instant::now();
        for _ in 0..WEB_PER_MINUTE {
            assert!(doors.may_send(start));
        }
        assert!(!doors.may_send(start));
        assert!(doors.may_send(start.checked_add(MINUTE).unwrap()));
    }
}
