//! Les portes vers le monde extérieur : ce que la machine demande à la
//! console, et ce que la console lui répond.
//!
//! La machine ne fait rien elle-même. Elle liste ce qu'elle veut
//! ([`crate::Machine::asks`]) ; la console le fait, note la réponse au
//! journal, et la remet à la machine ([`crate::Machine::answer`]). Une partie
//! se rejoue donc à l'identique sans internet ni fichier : la réponse est
//! dans le journal.
//!
//! ```ignore
//! fn asks(&self) -> Vec<Ask> {
//!     match &self.phase {
//!         Phase::Judging { round, text } => vec![Ask::web(*round, judge(text))],
//!         Phase::Lobby | Phase::Writing => Vec::new(),
//!     }
//! }
//!
//! fn answer(&mut self, id: u32, answer: Answer) -> Vec<(To, Effect)> {
//!     // Sans réponse (pas d'internet ce soir), les joueurs votent.
//!     self.phase = match answer.as_web() {
//!         Some((200, body)) => Phase::Judged(verdict(body)),
//!         Some(_) | None => Phase::Voting,
//!     };
//!     Vec::new()
//! }
//! ```

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::Content;

/// La console écoute au plus tant de demandes à la fois : les premières de
/// la liste.
pub const ASKS_MAX: usize = 16;
/// Ce qui part vers internet, et ce qui en revient, pèse au plus tant d'octets.
pub const WEB_MAX: usize = 1_048_576;
/// Une sauvegarde pèse au plus tant d'octets.
pub const SAVE_MAX: usize = 1_048_576;
/// Le nom d'une sauvegarde tient en tant d'octets, au plus.
pub const SAVE_NAME_MAX: usize = 64;
/// Un jeu garde au plus tant de sauvegardes.
pub const SAVES_MAX: usize = 100;

/// Une demande de la machine au monde extérieur.
///
/// `id` est le numéro que la machine lui donne : la réponse revient avec lui.
/// La console fait une demande une fois, tant qu'elle reste dans la liste ;
/// pour la refaire, la machine lui donne un autre numéro.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ask {
    pub id: u32,
    pub want: Want,
}

/// Ce qu'une demande veut. La machine l'écrit avec [`Ask::date`],
/// [`Ask::web`]… ; c'est la console qui le lit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Want {
    /// La date et l'heure, à la montre de l'hôte.
    Date,
    /// Une question à internet.
    Web(Web),
    /// Les octets d'un contenu que la console garde.
    Content(Content),
    /// Garder ces octets sous ce nom.
    Save { name: String, bytes: Vec<u8> },
    /// Les octets gardés sous ce nom.
    Load(String),
    /// Oublier ce qui est gardé sous ce nom.
    Erase(String),
    /// Les noms de tout ce qui est gardé.
    Saves,
}

impl Ask {
    /// Quel jour et quelle heure est-il chez l'hôte ? La réponse se lit avec
    /// [`Answer::as_date`].
    pub fn date(id: u32) -> Self {
        Self {
            id,
            want: Want::Date,
        }
    }

    /// Une question à internet. La réponse se lit avec [`Answer::as_web`].
    pub fn web(id: u32, web: Web) -> Self {
        Self {
            id,
            want: Want::Web(web),
        }
    }

    /// Les octets d'un contenu qu'une page a confié à la console : un niveau
    /// fabriqué ailleurs, une liste de questions. La réponse se lit avec
    /// [`Answer::as_bytes`]. Ce qu'il y a dedans vient d'un joueur : n'importe
    /// quoi.
    pub fn content(id: u32, content: Content) -> Self {
        Self {
            id,
            want: Want::Content(content),
        }
    }

    /// Garde ces octets sous ce nom, d'un lancement à l'autre ; ce qui portait
    /// ce nom est remplacé. [`Answer::is_done`] dit si c'est fait. Un nom vide ou
    /// de plus de [`SAVE_NAME_MAX`] octets, plus de [`SAVE_MAX`] octets, ou
    /// une sauvegarde de trop ([`SAVES_MAX`]) : ce n'est pas fait.
    pub fn save(id: u32, name: &str, bytes: Vec<u8>) -> Self {
        Self {
            id,
            want: Want::Save {
                name: name.to_owned(),
                bytes,
            },
        }
    }

    /// Les octets gardés sous ce nom. La réponse se lit avec
    /// [`Answer::as_bytes`] : rien si ce nom ne garde rien. Ils peuvent venir
    /// d'une autre version du jeu : relis-les sans leur faire confiance.
    pub fn load(id: u32, name: &str) -> Self {
        Self {
            id,
            want: Want::Load(name.to_owned()),
        }
    }

    /// Oublie ce qui est gardé sous ce nom. [`Answer::is_done`] dit que ce nom
    /// ne garde plus rien.
    pub fn erase(id: u32, name: &str) -> Self {
        Self {
            id,
            want: Want::Erase(name.to_owned()),
        }
    }

    /// Les noms de tout ce qui est gardé, dans l'ordre. La réponse se lit
    /// avec [`Answer::as_names`].
    pub fn saves(id: u32) -> Self {
        Self {
            id,
            want: Want::Saves,
        }
    }
}

/// Lire ou envoyer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// Une question à internet : une adresse en `https://` (ou `http://localhost`,
/// pour un service qui tourne sur l'ordinateur de l'hôte).
///
/// Un secret (une clé) n'est jamais dans la machine. Dans la valeur d'un
/// en-tête, `{NOM}` est remplacé par le secret que l'hôte a confié à la
/// console sous ce nom, et seulement vers l'adresse pour laquelle il l'a
/// confié.
///
/// ```ignore
/// Web::post("https://api.example.com/v1/juge", body)
///     .header("content-type", "application/json")
///     .header("authorization", "Bearer {JUGE}")
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Web {
    method: Method,
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Web {
    /// Lire cette adresse.
    pub fn get(url: &str) -> Self {
        Self {
            method: Method::Get,
            url: url.to_owned(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// Envoyer ces octets à cette adresse.
    pub fn post(url: &str, body: Vec<u8>) -> Self {
        Self {
            method: Method::Post,
            url: url.to_owned(),
            headers: Vec::new(),
            body,
        }
    }

    /// Avec cet en-tête en plus.
    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    pub fn method(&self) -> Method {
        self.method
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

/// La date et l'heure, à la montre de l'hôte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Date {
    pub year: u16,
    /// De 1 (janvier) à 12.
    pub month: u8,
    /// De 1 à 31.
    pub day: u8,
    /// De 1 (lundi) à 7 (dimanche).
    pub weekday: u8,
    /// De 0 à 23.
    pub hour: u8,
    /// De 0 à 59.
    pub minute: u8,
}

impl Date {
    /// La date qu'il est `seconds` secondes après le 1er janvier 1970 à
    /// minuit, à la même montre.
    pub fn at(seconds: u64) -> Self {
        let days = seconds / 86_400;
        let time = seconds % 86_400;
        let (year, month, day) = civil(days);
        Self {
            year: u16::try_from(year).unwrap_or(u16::MAX),
            month: small(month),
            day: small(day),
            // Le 1er janvier 1970 était un jeudi.
            weekday: small((days.saturating_add(3) % 7).saturating_add(1)),
            hour: small(time / 3600),
            minute: small(time % 3600 / 60),
        }
    }
}

fn small(number: u64) -> u8 {
    u8::try_from(number).unwrap_or(u8::MAX)
}

/// L'année, le mois et le jour d'un nombre de jours depuis le 1er janvier
/// 1970 (l'algorithme `civil_from_days` de Howard Hinnant, sur des entiers
/// positifs : l'année commence en mars, le jour bissextile tombe à la fin).
fn civil(days: u64) -> (u64, u64, u64) {
    let shifted = days.saturating_add(719_468);
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era = day_of_era
        .saturating_sub(day_of_era / 1460)
        .saturating_add(day_of_era / 36_524)
        .saturating_sub(day_of_era / 146_096)
        / 365;
    let day_of_year = day_of_era.saturating_sub(
        year_of_era
            .saturating_mul(365)
            .saturating_add(year_of_era / 4)
            .saturating_sub(year_of_era / 100),
    );
    let month_from_march = day_of_year.saturating_mul(5).saturating_add(2) / 153;
    let day = day_of_year
        .saturating_sub(month_from_march.saturating_mul(153).saturating_add(2) / 5)
        .saturating_add(1);
    let month = if month_from_march < 10 {
        month_from_march.saturating_add(3)
    } else {
        month_from_march.saturating_sub(9)
    };
    let year = year_of_era
        .saturating_add(era.saturating_mul(400))
        .saturating_add(u64::from(month <= 2));
    (year, month, day)
}

/// La réponse de la console à une demande.
///
/// Elle se lit par la question qu'on a posée : [`Answer::as_date`] après
/// [`Ask::date`], [`Answer::as_web`] après [`Ask::web`]… Sans réponse (pas
/// d'internet, un contenu effacé, un nom qui ne garde rien), ces lectures ne
/// rendent rien : la machine continue sans.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer(pub(crate) Got);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Got {
    Nothing,
    Done,
    Date(Date),
    Web { status: u16, body: Vec<u8> },
    Bytes(Vec<u8>),
    Names(Vec<String>),
}

impl Answer {
    /// Pas de réponse : la console n'a pas pu, ou n'a rien trouvé.
    pub fn nothing() -> Self {
        Self(Got::Nothing)
    }

    /// C'est fait (garder, oublier).
    pub fn done() -> Self {
        Self(Got::Done)
    }

    /// Une date.
    pub fn date(date: Date) -> Self {
        Self(Got::Date(date))
    }

    /// Ce qu'internet a répondu : son code (200, 404…) et ses octets.
    pub fn web(status: u16, body: Vec<u8>) -> Self {
        Self(Got::Web { status, body })
    }

    /// Des octets : ceux d'un contenu, ou d'une sauvegarde.
    pub fn bytes(bytes: Vec<u8>) -> Self {
        Self(Got::Bytes(bytes))
    }

    /// Les noms des sauvegardes.
    pub fn names(names: Vec<String>) -> Self {
        Self(Got::Names(names))
    }

    /// La date, après [`Ask::date`].
    pub fn as_date(&self) -> Option<Date> {
        match &self.0 {
            Got::Date(date) => Some(*date),
            Got::Nothing | Got::Done | Got::Web { .. } | Got::Bytes(_) | Got::Names(_) => None,
        }
    }

    /// Le code et les octets de la réponse d'internet, après [`Ask::web`].
    /// Rien si internet n'a pas répondu ; un code d'erreur (404, 500) est
    /// une réponse.
    pub fn as_web(&self) -> Option<(u16, &[u8])> {
        match &self.0 {
            Got::Web { status, body } => Some((*status, body)),
            Got::Nothing | Got::Done | Got::Date(_) | Got::Bytes(_) | Got::Names(_) => None,
        }
    }

    /// Les octets, après [`Ask::content`] ou [`Ask::load`].
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match &self.0 {
            Got::Bytes(bytes) => Some(bytes),
            Got::Nothing | Got::Done | Got::Date(_) | Got::Web { .. } | Got::Names(_) => None,
        }
    }

    /// Les noms des sauvegardes, après [`Ask::saves`].
    pub fn as_names(&self) -> Option<&[String]> {
        match &self.0 {
            Got::Names(names) => Some(names),
            Got::Nothing | Got::Done | Got::Date(_) | Got::Web { .. } | Got::Bytes(_) => None,
        }
    }

    /// Vrai si c'est fait, après [`Ask::save`] ou [`Ask::erase`].
    pub fn is_done(&self) -> bool {
        self.0 == Got::Done
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    #[test]
    fn a_number_of_seconds_reads_as_a_date() {
        let date = |year, month, day, weekday, hour, minute| Date {
            year,
            month,
            day,
            weekday,
            hour,
            minute,
        };
        assert_eq!(Date::at(0), date(1970, 1, 1, 4, 0, 0));
        // Un mardi 29 février.
        assert_eq!(Date::at(951_782_400), date(2000, 2, 29, 2, 0, 0));
        assert_eq!(Date::at(1_700_000_000), date(2023, 11, 14, 2, 22, 13));
        // Un dimanche, à la dernière minute.
        assert_eq!(Date::at(4_107_542_399), date(2100, 2, 28, 7, 23, 59));
    }

    #[test]
    fn an_answer_reads_only_as_what_it_is() {
        let web = Answer::web(200, vec![1, 2]);
        assert_eq!(web.as_web(), Some((200, [1_u8, 2].as_slice())));
        assert_eq!(web.as_bytes(), None);
        assert_eq!(Answer::bytes(vec![7]).as_bytes(), Some([7_u8].as_slice()));
        assert!(Answer::done().is_done());
        let nothing = Answer::nothing();
        assert_eq!(nothing.as_date(), None);
        assert_eq!(nothing.as_web(), None);
        assert_eq!(nothing.as_names(), None);
        assert!(!nothing.is_done());
    }

    #[test]
    fn a_question_to_the_web_keeps_what_it_was_given() {
        let web = Web::post("https://exemple.test/juge", vec![1])
            .header("authorization", "Bearer {JUGE}");
        assert_eq!(web.method(), Method::Post);
        assert_eq!(web.url(), "https://exemple.test/juge");
        assert_eq!(web.body(), [1]);
        assert_eq!(
            web.headers(),
            [("authorization".to_owned(), "Bearer {JUGE}".to_owned())]
        );
        assert_eq!(Ask::web(3, web.clone()).want, Want::Web(web));
    }
}
