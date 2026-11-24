//! Le chargement de la page : ce qu'un téléphone télécharge avant de pouvoir
//! jouer, et le temps que ça prend à travers le tuyau.
//!
//! Le téléphone d'essai lit la page, y trouve ce qu'elle appelle (sa feuille
//! de style, son programme, ses polices), puis télécharge le tout d'une
//! traite, sur une seule connexion, en disant comme un navigateur qu'il sait
//! lire du compressé. Il ne voit pas ce que le jeu charge plus tard par
//! lui-même : ses sons, ses images.

use std::io;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader};

use super::Target;
use crate::internet::Stream;

/// Au-delà, ce n'est plus une page de jeu : le banc s'arrête de lire.
const BODY_MAX: u64 = 64 * 1024 * 1024;
const HEAD_MAX: usize = 32 * 1024;
const FILES_MAX: usize = 64;

/// Un fichier de la page, tel qu'il est arrivé.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct File {
    pub(super) path: String,
    /// Les octets qui ont passé le réseau.
    pub(super) bytes: u64,
    /// La compression annoncée par le serveur, s'il y en a une.
    pub(super) encoding: Option<String>,
}

/// La page, chargée une fois.
#[derive(Clone, Debug)]
pub(super) struct Load {
    pub(super) files: Vec<File>,
    pub(super) took: Duration,
}

impl Load {
    pub(super) fn bytes(&self) -> u64 {
        self.files
            .iter()
            .fold(0, |total, file| total.saturating_add(file.bytes))
    }

    pub(super) fn heaviest(&self) -> Option<&File> {
        self.files.iter().max_by_key(|file| file.bytes)
    }
}

/// Ce que le téléphone d'essai dit savoir lire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Accept {
    /// Tel quel : pour lire le texte et y trouver ce qu'il appelle.
    Plain,
    /// Comme un navigateur : compressé si le serveur sait.
    Compressed,
}

struct Got {
    status: u16,
    encoding: Option<String>,
    body: Vec<u8>,
}

fn wrong(what: &str) -> io::Error {
    io::Error::other(what.to_owned())
}

/// Demande un fichier sur une connexion gardée ouverte, et lit sa réponse.
async fn get(
    reader: &mut BufReader<Stream>,
    target: &Target,
    path: &str,
    accept: Accept,
) -> io::Result<Got> {
    let accept = match accept {
        Accept::Plain => "identity",
        Accept::Compressed => "gzip, br",
    };
    let ask = format!(
        "GET {path} HTTP/1.1\r\nHost: {}\r\nAccept-Encoding: {accept}\r\nUser-Agent: \
         soiree-banc\r\nConnection: keep-alive\r\n\r\n",
        target.authority()
    );
    reader.get_mut().write_all(ask.as_bytes()).await?;
    let mut line = String::new();
    let _: usize = reader.read_line(&mut line).await?;
    let status = line
        .split_whitespace()
        .nth(1)
        .and_then(|status| status.parse::<u16>().ok())
        .ok_or_else(|| wrong("réponse illisible"))?;
    let (mut length, mut chunked, mut encoding, mut head) = (None, false, None, line.len());
    loop {
        line.clear();
        let read = reader.read_line(&mut line).await?;
        head = head.saturating_add(read);
        if read == 0 || head > HEAD_MAX {
            return Err(wrong("en-tête illisible"));
        }
        let Some((name, value)) = line.trim_end().split_once(':') else {
            break;
        };
        let value = value.trim();
        match name.to_ascii_lowercase().as_str() {
            "content-length" => length = value.parse::<u64>().ok(),
            "transfer-encoding" => chunked = value.eq_ignore_ascii_case("chunked"),
            "content-encoding" => encoding = Some(value.to_ascii_lowercase()),
            _ => {}
        }
    }
    let mut body = Vec::new();
    if chunked {
        loop {
            line.clear();
            let _: usize = reader.read_line(&mut line).await?;
            let size = line.trim().split(';').next().unwrap_or_default();
            let size = u64::from_str_radix(size, 16).map_err(|_| wrong("morceau illisible"))?;
            if size == 0 {
                // La ligne vide qui ferme la réponse.
                line.clear();
                let _: usize = reader.read_line(&mut line).await?;
                break;
            }
            let room = BODY_MAX.saturating_sub(u64::try_from(body.len()).unwrap_or(u64::MAX));
            if size > room {
                return Err(wrong("réponse trop lourde"));
            }
            let _: usize = (&mut *reader).take(size).read_to_end(&mut body).await?;
            let mut end = [0_u8; 2];
            let _: usize = reader.read_exact(&mut end).await?;
        }
    } else if let Some(length) = length {
        if length > BODY_MAX {
            return Err(wrong("réponse trop lourde"));
        }
        let read = (&mut *reader).take(length).read_to_end(&mut body).await?;
        if u64::try_from(read).ok() != Some(length) {
            return Err(wrong("réponse coupée"));
        }
    } else {
        return Err(wrong("réponse sans longueur"));
    }
    Ok(Got {
        status,
        encoding,
        body,
    })
}

/// Le chemin qu'une référence désigne, vue depuis le fichier qui la porte.
/// `None` : ce n'est pas un fichier de ce jeu (une ancre, une image écrite
/// sur place, un autre site).
fn resolve(from: &str, reference: &str) -> Option<String> {
    let reference = reference
        .trim()
        .trim_matches(|sign| sign == '"' || sign == '\'');
    if reference.is_empty()
        || reference.starts_with('#')
        || reference.starts_with("data:")
        || reference.contains("://")
        || reference.starts_with("//")
    {
        return None;
    }
    if reference.starts_with('/') {
        return Some(reference.to_owned());
    }
    let from = from.split('?').next().unwrap_or(from);
    let folder = from.rsplit_once('/').map_or("", |(folder, _)| folder);
    Some(format!("{folder}/{reference}"))
}

/// Ce qu'un texte appelle : ce qui suit chaque `opening`, jusqu'à `closing`.
fn references(text: &str, opening: &str, closing: char) -> Vec<String> {
    text.split(opening)
        .skip(1)
        .filter_map(|rest| rest.split(closing).next())
        .map(str::to_owned)
        .collect()
}

/// Ce que la page appelle, elle comprise, sans doublon.
async fn discover(reader: &mut BufReader<Stream>, target: &Target, page: &str) -> Vec<String> {
    let mut found = vec![page.to_owned()];
    let add = |found: &mut Vec<String>, path: Option<String>| {
        if let Some(path) = path
            && !found.contains(&path)
            && found.len() < FILES_MAX
        {
            found.push(path);
        }
    };
    let Ok(html) = get(reader, target, page, Accept::Plain).await else {
        return found;
    };
    let html = String::from_utf8_lossy(&html.body).into_owned();
    // `from "` : le programme de la page, appelé par `import … from "/pages.js"`.
    for opening in ["href=\"", "src=\"", "from \""] {
        for reference in references(&html, opening, '"') {
            add(&mut found, resolve("/", &reference));
        }
    }
    for path in found.clone() {
        let bare = path.split('?').next().unwrap_or(&path);
        if let Some(stem) = bare.strip_suffix(".js") {
            // Le programme de la page : `wasm-bindgen` le range à côté.
            add(&mut found, Some(format!("{stem}_bg.wasm")));
        }
        let style = std::path::Path::new(bare)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("css"));
        if style && let Ok(css) = get(reader, target, &path, Accept::Plain).await {
            let css = String::from_utf8_lossy(&css.body).into_owned();
            for reference in references(&css, "url(", ')') {
                add(&mut found, resolve(&path, &reference));
            }
        }
    }
    found
}

/// Charge la page comme un téléphone qui vient de scanner le QR code.
pub(super) async fn load(target: &Target, through: u16, page: &str) -> Result<Load, String> {
    let told = |error: &dyn std::fmt::Display| format!("la page ne se charge pas ({error})");
    // Un premier passage pour savoir quoi charger : il ne compte pas.
    let stream = super::open(target, through).await?;
    let paths = discover(&mut BufReader::new(stream), target, page).await;
    // Le passage mesuré : une connexion neuve, tout d'une traite.
    let since = Instant::now();
    let stream = super::open(target, through).await?;
    let mut reader = BufReader::new(stream);
    let mut files = Vec::new();
    for path in paths {
        let got = get(&mut reader, target, &path, Accept::Compressed)
            .await
            .map_err(|error| told(&error))?;
        if got.status == 200 {
            files.push(File {
                path,
                bytes: u64::try_from(got.body.len()).unwrap_or(u64::MAX),
                encoding: got.encoding,
            });
        }
    }
    if files.is_empty() {
        return Err(told(&"le jeu ne sert pas de page à cette adresse"));
    }
    Ok(Load {
        files,
        took: since.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reference_is_a_path_of_the_game() {
        assert_eq!(resolve("/", "/style.css"), Some("/style.css".to_owned()));
        assert_eq!(resolve("/", "pages.js"), Some("/pages.js".to_owned()));
        assert_eq!(
            resolve("/css/style.css", "\"fonts/a.woff2\""),
            Some("/css/fonts/a.woff2".to_owned())
        );
        assert_eq!(
            resolve("/style.css", "'/fonts/a.woff2'"),
            Some("/fonts/a.woff2".to_owned())
        );
        // Ni une ancre, ni une image écrite sur place, ni un autre site.
        assert_eq!(resolve("/style.css", "#clay"), None);
        assert_eq!(resolve("/style.css", "data:image/png;base64,AAAA"), None);
        assert_eq!(resolve("/", "https://exemple.com/a.js"), None);
    }

    #[test]
    fn what_a_text_calls_is_found() {
        let html = r#"<link rel="stylesheet" href="/style.css"><script type="module" src="/pages.js"></script>"#;
        assert_eq!(references(html, "href=\"", '"'), ["/style.css"]);
        assert_eq!(references(html, "src=\"", '"'), ["/pages.js"]);
        let css = r#"src: url("/fonts/a.woff2"); filter: url(#clay);"#;
        assert_eq!(
            references(css, "url(", ')'),
            ["\"/fonts/a.woff2\"", "#clay"]
        );
    }
}
