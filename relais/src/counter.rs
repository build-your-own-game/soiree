//! Le guichet : le relais prête ses secrets à une question d'un jeu.
//!
//! Un ami qui reçoit chez lui n'a pas les clés de l'hôte (une clé d'IA, par
//! exemple) : elles restent ici, sur le serveur. Quand la console d'un jeu
//! n'a pas le secret qu'une question nomme, elle envoie la question, telle
//! quelle, au guichet de l'étagère, qui la fait suivre ici avec sa clé. Le
//! relais remplace chaque `{NOM}` par `RELAIS_SECRET_NOM`, seulement si
//! l'adresse appelée est celle que `RELAIS_SECRET_NOM_POUR` nomme, en
//! HTTPS, pose la question et rend la réponse.
//!
//! La demande est la question elle-même : sa méthode (`X-Guichet-Methode`),
//! son adresse (`X-Guichet-Adresse`) et chacun de ses en-têtes
//! (`X-Guichet-En-Tete: nom: valeur`), ses octets dans le corps. La réponse
//! est `200`, le statut du serveur appelé dans `X-Guichet-Statut` et ses
//! octets dans le corps ; toute autre réponse dit pourquoi le guichet n'a pas
//! pu, en texte. Un secret n'est jamais écrit, ni au journal ni dans une
//! réponse.

use std::time::Duration;

use ureq::http::Uri;

pub const PATH: &str = "/_relais/guichet";
/// L'en-tête de la demande porte ceux de la question : il a droit à plus
/// qu'une demande du fil.
pub const HEAD_MAX: usize = 32 * 1024;
/// Ce qui part, et ce qui revient, pèse au plus tant d'octets : comme pour
/// la console (`cartouche::WEB_MAX`).
pub const BODY_MAX: usize = 1_048_576;
/// Le serveur appelé a tant de temps pour répondre : moins que la console
/// n'en laisse à l'étagère, pour qu'elle apprenne pourquoi.
const PATIENCE: Duration = Duration::from_secs(25);
const SECRET: &str = "RELAIS_SECRET_";
const SECRET_FOR: &str = "_POUR";

/// Le guichet n'a pas pu : le statut de la réponse, et pourquoi.
#[derive(Debug, PartialEq, Eq)]
pub struct Refusal {
    pub status: &'static str,
    pub why: String,
}

fn refuse(status: &'static str, why: impl Into<String>) -> Refusal {
    Refusal {
        status,
        why: why.into(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Method {
    Get,
    Post,
}

/// La question à poser, lue dans l'en-tête de la demande.
#[derive(Debug, PartialEq, Eq)]
pub struct Call {
    method: Method,
    url: String,
    headers: Vec<(String, String)>,
    /// Le nombre d'octets du corps, qui suit l'en-tête.
    pub length: usize,
}

/// Lit la question dans l'en-tête d'une demande au guichet.
pub fn read(head: &str) -> Result<Call, Refusal> {
    let (mut method, mut url, mut length) = (None, None, 0);
    let mut headers = Vec::new();
    for line in head.split("\r\n").skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "x-guichet-methode" => {
                method = Some(match value {
                    "GET" => Method::Get,
                    "POST" => Method::Post,
                    _ => return Err(refuse("400 Bad Request", "une méthode inconnue")),
                });
            }
            "x-guichet-adresse" => url = Some(value.to_owned()),
            "x-guichet-en-tete" => {
                let (name, value) = value
                    .split_once(':')
                    .ok_or_else(|| refuse("400 Bad Request", "un en-tête sans nom"))?;
                headers.push((name.trim().to_owned(), value.trim().to_owned()));
            }
            "content-length" => {
                length = value
                    .parse()
                    .map_err(|_| refuse("400 Bad Request", "une longueur illisible"))?;
            }
            "transfer-encoding" => {
                return Err(refuse(
                    "411 Length Required",
                    "le corps doit dire sa longueur",
                ));
            }
            _ => {}
        }
    }
    if length > BODY_MAX {
        return Err(refuse(
            "413 Content Too Large",
            format!("l'envoi dépasse {BODY_MAX} octets"),
        ));
    }
    Ok(Call {
        method: method.ok_or_else(|| refuse("400 Bad Request", "la méthode manque"))?,
        url: url.ok_or_else(|| refuse("400 Bad Request", "l'adresse manque"))?,
        headers,
        length,
    })
}

/// Un secret que l'hôte a confié au relais, et l'adresse où il a le droit
/// d'aller.
pub struct Secret {
    value: String,
    host: String,
}

/// Le secret de ce nom, dans les variables du serveur.
pub fn secret(name: &str) -> Option<Secret> {
    Some(Secret {
        value: std::env::var(format!("{SECRET}{name}")).ok()?,
        host: std::env::var(format!("{SECRET}{name}{SECRET_FOR}")).ok()?,
    })
}

/// Le nom de l'hôte d'une adresse en `https://`. Le guichet ne pose pas de
/// question en clair : un secret n'y part pas.
fn host(url: &str) -> Result<String, Refusal> {
    let uri: Uri = url
        .parse()
        .map_err(|_| refuse("400 Bad Request", "l'adresse est illisible"))?;
    if uri.scheme_str() != Some("https") {
        return Err(refuse(
            "403 Forbidden",
            "le guichet n'appelle qu'une adresse en `https://`",
        ));
    }
    uri.host()
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| refuse("400 Bad Request", "l'adresse ne nomme personne"))
}

/// La valeur d'un en-tête, où chaque `{NOM}` est remplacé par le secret de
/// ce nom, s'il est confié pour `host`. Chaque secret mis est ajouté à `used`.
fn fill(
    value: &str,
    host: &str,
    secret: &dyn Fn(&str) -> Option<Secret>,
    used: &mut Vec<String>,
) -> Result<String, Refusal> {
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
            return Err(refuse(
                "403 Forbidden",
                format!(
                    "le relais n'a pas le secret `{name}`. Sur le serveur : {SECRET}{name} (sa \
                     valeur) et {SECRET}{name}{SECRET_FOR} (l'adresse où il a le droit d'aller, \
                     ici `{host}`)"
                ),
            ));
        };
        if !secret.host.trim().eq_ignore_ascii_case(host) {
            return Err(refuse(
                "403 Forbidden",
                format!(
                    "le secret `{name}` est confié pour `{}` : il ne part pas vers `{host}`",
                    secret.host.trim()
                ),
            ));
        }
        filled.push_str(&secret.value);
        used.push(secret.value);
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

/// Ce qui est prêt à partir : l'hôte appelé, les en-têtes remplis, et les
/// secrets qu'ils portent, pour qu'aucun message ne les répète.
struct Filled {
    host: String,
    headers: Vec<(String, String)>,
    used: Vec<String>,
}

fn prepare(call: &Call, secret: &dyn Fn(&str) -> Option<Secret>) -> Result<Filled, Refusal> {
    let host = host(&call.url)?;
    let mut used = Vec::new();
    let headers = call
        .headers
        .iter()
        .map(|(name, value)| Ok((name.clone(), fill(value, &host, secret, &mut used)?)))
        .collect::<Result<Vec<_>, Refusal>>()?;
    // Sans secret à prêter, le guichet ne sert à rien : il n'est pas un
    // passage vers internet.
    if used.is_empty() {
        return Err(refuse(
            "400 Bad Request",
            "la question ne nomme aucun secret : le guichet ne sert qu'à en prêter",
        ));
    }
    Ok(Filled {
        host,
        headers,
        used,
    })
}

/// Pose la question et rend le statut et les octets de la réponse.
pub fn ask(
    call: &Call,
    body: &[u8],
    secret: &dyn Fn(&str) -> Option<Secret>,
) -> Result<(String, u16, Vec<u8>), Refusal> {
    let filled = prepare(call, secret)?;
    // Un message d'erreur vient de la bibliothèque : il ne doit répéter
    // aucun secret, même par accident.
    let hide = |text: String| {
        filled
            .used
            .iter()
            .fold(text, |text, value| text.replace(value.as_str(), "…"))
    };
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(PATIENCE))
        // Une redirection emmènerait les secrets à une autre adresse.
        .max_redirects(0)
        // Un code d'erreur est une réponse : le jeu le lit.
        .http_status_as_error(false)
        .build()
        .into();
    let sent = match call.method {
        Method::Get => filled
            .headers
            .iter()
            .fold(agent.get(&call.url), |request, (name, value)| {
                request.header(name, value)
            })
            .call(),
        Method::Post => filled
            .headers
            .iter()
            .fold(agent.post(&call.url), |request, (name, value)| {
                request.header(name, value)
            })
            .send(body),
    };
    let host = filled.host.clone();
    let response = sent.map_err(|error| {
        refuse(
            "502 Bad Gateway",
            hide(format!("{host} ne répond pas ({error})")),
        )
    })?;
    let status = response.status().as_u16();
    let bytes = response
        .into_body()
        .with_config()
        .limit(u64::try_from(BODY_MAX).unwrap_or(u64::MAX))
        .read_to_vec()
        .map_err(|error| {
            refuse(
                "502 Bad Gateway",
                hide(format!("la réponse de {host} ne se lit pas ({error})")),
            )
        })?;
    Ok((host, status, bytes))
}

/// La réponse du guichet, en octets.
pub fn reply(asked: Result<(u16, Vec<u8>), Refusal>) -> Vec<u8> {
    match asked {
        Ok((status, body)) => {
            let mut reply = format!(
                "HTTP/1.1 200 OK\r\nX-Guichet-Statut: {status}\r\n\
                 Content-Type: application/octet-stream\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n",
                body.len()
            )
            .into_bytes();
            reply.extend_from_slice(&body);
            reply
        }
        Err(refusal) => format!(
            "HTTP/1.1 {}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{}",
            refusal.status,
            refusal.why.len(),
            refusal.why
        )
        .into_bytes(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn judge(name: &str) -> Option<Secret> {
        (name == "JUGE").then(|| Secret {
            value: "s3cr3t".to_owned(),
            host: " Juge.Test ".to_owned(),
        })
    }

    fn call(url: &str, header: &str) -> Call {
        read(&format!(
            "POST /_relais/guichet HTTP/1.1\r\nHost: relais\r\nX-Guichet-Methode: POST\r\n\
             X-Guichet-Adresse: {url}\r\nX-Guichet-En-Tete: {header}\r\n\
             x-guichet-en-tete: content-type: application/json\r\nContent-Length: 4\r\n\r\n"
        ))
        .unwrap()
    }

    #[test]
    fn the_question_is_read_from_the_head() {
        let question = call("https://juge.test/v1?a=b", "authorization: Bearer {JUGE}");
        assert_eq!(
            question,
            Call {
                method: Method::Post,
                url: "https://juge.test/v1?a=b".to_owned(),
                headers: vec![
                    ("authorization".to_owned(), "Bearer {JUGE}".to_owned()),
                    ("content-type".to_owned(), "application/json".to_owned()),
                ],
                length: 4,
            }
        );
        let wrong = |head: &str| read(head).unwrap_err().status;
        assert_eq!(wrong("POST / HTTP/1.1\r\n\r\n"), "400 Bad Request");
        assert_eq!(
            wrong("POST / HTTP/1.1\r\nX-Guichet-Methode: PUT\r\n\r\n"),
            "400 Bad Request"
        );
        assert_eq!(
            wrong(&format!(
                "POST / HTTP/1.1\r\nX-Guichet-Methode: GET\r\nX-Guichet-Adresse: https://a.test/\r\n\
                 Content-Length: {}\r\n\r\n",
                BODY_MAX + 1
            )),
            "413 Content Too Large"
        );
        assert_eq!(
            wrong("POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n"),
            "411 Length Required"
        );
    }

    #[test]
    fn a_secret_is_lent_only_to_its_address() {
        let filled = prepare(
            &call("https://JUGE.test/v1", "authorization: Bearer {JUGE}"),
            &judge,
        )
        .unwrap();
        assert_eq!(filled.host, "juge.test");
        assert_eq!(
            filled.headers,
            vec![
                ("authorization".to_owned(), "Bearer s3cr3t".to_owned()),
                ("content-type".to_owned(), "application/json".to_owned()),
            ]
        );
        // Ni ailleurs, ni en clair, ni un secret que le relais n'a pas.
        for (url, header) in [
            ("https://ailleurs.test/", "authorization: Bearer {JUGE}"),
            ("http://juge.test/", "authorization: Bearer {JUGE}"),
            ("https://juge.test/", "authorization: Bearer {AUTRE}"),
            ("https://juge.test/", "x: {JUGE} {AUTRE}"),
        ] {
            let refused = prepare(&call(url, header), &judge).err().unwrap();
            assert_eq!(refused.status, "403 Forbidden", "{url} {header}");
            assert!(!refused.why.contains("s3cr3t"), "{}", refused.why);
        }
        let missing = prepare(&call("https://juge.test/", "x: {AUTRE}"), &judge)
            .err()
            .unwrap();
        assert!(
            missing.why.contains("RELAIS_SECRET_AUTRE_POUR"),
            "{}",
            missing.why
        );
        // Une question sans secret n'a rien à faire au guichet.
        let none = prepare(&call("https://juge.test/", "x: {\"a\":1} {juge}"), &judge)
            .err()
            .unwrap();
        assert_eq!(none.status, "400 Bad Request");
    }

    #[test]
    fn what_the_counter_says_hides_the_secret() {
        // Un serveur qui n'existe pas : le message d'erreur ne répète pas le secret.
        let refused = ask(
            &call("https://127.0.0.1:1/", "authorization: Bearer {LOCAL}"),
            b"ping",
            &|_| {
                Some(Secret {
                    value: "127.0.0.1".to_owned(),
                    host: "127.0.0.1".to_owned(),
                })
            },
        )
        .err()
        .unwrap();
        assert_eq!(refused.status, "502 Bad Gateway");
        assert!(!refused.why.contains("127.0.0.1"), "{}", refused.why);
    }

    #[test]
    fn the_reply_carries_the_status_of_the_called_server() {
        let reply = String::from_utf8(super::reply(Ok((418, b"pong".to_vec())))).unwrap();
        assert!(reply.starts_with("HTTP/1.1 200 OK\r\n"), "{reply}");
        assert!(reply.contains("X-Guichet-Statut: 418\r\n"), "{reply}");
        assert!(reply.ends_with("\r\n\r\npong"), "{reply}");
        let refused = String::from_utf8(super::reply(Err(refuse("403 Forbidden", "non")))).unwrap();
        assert!(
            refused.starts_with("HTTP/1.1 403 Forbidden\r\n"),
            "{refused}"
        );
        assert!(!refused.contains("X-Guichet-Statut"), "{refused}");
    }
}
