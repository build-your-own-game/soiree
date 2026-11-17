//! Le contenu lourd, rangé par la console : une image, un son qu'un joueur
//! apporte. Une page le confie, la console rend son ticket
//! ([`cartouche::Content`]) ; une page le redemande par le nom du ticket.
//!
//! Un contenu porte le nom de son ticket, dans `.soiree/contenus/` : confié
//! deux fois, il n'est rangé qu'une fois. Quand le dossier dépasse sa limite,
//! les contenus montrés il y a le plus longtemps s'effacent.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use cartouche::{Content, Kind};

/// Un contenu pèse au plus tant d'octets.
pub(crate) const CONTENT_MAX: usize = 16_777_216;
/// Au-delà, les contenus montrés il y a le plus longtemps s'effacent.
const STORE_LIMIT: u64 = 500_000_000;
/// Le type d'un contenu dont la page n'a rien dit d'utilisable.
const UNKNOWN: &str = "application/octet-stream";
/// À côté de chaque contenu, un fichier de ce suffixe garde son type.
const TYPE_SUFFIX: &str = ".type";
const TYPE_MAX: usize = 100;

/// Pourquoi un contenu n'a pas été rangé.
#[derive(Debug)]
pub(crate) enum Refused {
    /// Il n'y a rien dedans.
    Empty,
    /// Il dépasse [`CONTENT_MAX`].
    TooBig,
    /// Le disque n'en a pas voulu.
    Disk(io::Error),
}

/// Le dossier des contenus.
pub(crate) struct Store {
    folder: PathBuf,
    limit: u64,
    /// Une seule écriture à la fois : deux pages qui confient le même contenu.
    writing: Mutex<()>,
}

impl Store {
    /// Le dossier s'écrit au premier contenu confié.
    pub(crate) fn open(folder: PathBuf) -> Self {
        Self::with_limit(folder, STORE_LIMIT)
    }

    fn with_limit(folder: PathBuf, limit: u64) -> Self {
        let store = Self {
            folder,
            limit,
            writing: Mutex::new(()),
        };
        store.prune();
        store
    }

    /// Range un contenu et rend son ticket. `declared` est le type que la
    /// page annonce (`image/webp`).
    pub(crate) fn put(&self, bytes: &[u8], declared: &str) -> Result<Content, Refused> {
        if bytes.is_empty() {
            return Err(Refused::Empty);
        }
        if bytes.len() > CONTENT_MAX {
            return Err(Refused::TooBig);
        }
        let content_type = clean(declared);
        let content = Content::of(bytes, Kind::of_type(&content_type));
        let path = self.folder.join(content.name());
        let writing = self.lock();
        if path.is_file() {
            touch(&path);
        } else {
            self.write(&path, bytes, &content_type)
                .map_err(Refused::Disk)?;
        }
        drop(writing);
        self.prune();
        Ok(content)
    }

    fn write(&self, path: &Path, bytes: &[u8], content_type: &str) -> io::Result<()> {
        fs::create_dir_all(&self.folder)?;
        // Le type d'abord : un contenu sans type se montrerait mal. Le
        // contenu arrive d'un coup, sous son nom : jamais à moitié écrit.
        fs::write(sidecar(path), content_type)?;
        let draft = path.with_extension("ecriture");
        fs::write(&draft, bytes)?;
        fs::rename(&draft, path)
    }

    /// Les octets et le type du contenu de ce nom. `None` si le nom ne mène
    /// à rien, ou à des octets qui ne sont pas ceux du ticket.
    pub(crate) fn get(&self, name: &str) -> Option<(Vec<u8>, String)> {
        let content = Content::named(name)?;
        let path = self.folder.join(content.name());
        let bytes = fs::read(&path).ok()?;
        let content_type = fs::read_to_string(sidecar(&path))
            .ok()
            .map_or_else(|| UNKNOWN.to_owned(), |declared| clean(&declared));
        let kind = Kind::of_type(&content_type);
        if Content::of(&bytes, kind) != content {
            return None;
        }
        touch(&path);
        Some((bytes, content_type))
    }

    /// Efface les contenus montrés il y a le plus longtemps, tant que le
    /// dossier dépasse sa limite.
    fn prune(&self) {
        let _writing = self.lock();
        let Ok(entries) = fs::read_dir(&self.folder) else {
            return;
        };
        let mut contents: Vec<(SystemTime, u64, PathBuf)> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name();
                let _: Content = Content::named(name.to_str()?)?;
                let metadata = entry.metadata().ok()?;
                Some((metadata.modified().ok()?, metadata.len(), entry.path()))
            })
            .collect();
        contents.sort();
        let mut total = contents
            .iter()
            .fold(0_u64, |total, (_, len, _)| total.saturating_add(*len));
        for (_, len, path) in contents {
            if total <= self.limit {
                break;
            }
            if fs::remove_file(&path).is_ok() {
                let _: io::Result<()> = fs::remove_file(sidecar(&path));
                total = total.saturating_sub(len);
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, ()> {
        self.writing.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn sidecar(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(TYPE_SUFFIX);
    PathBuf::from(name)
}

/// Dit que ce contenu vient de servir : il s'effacera après les autres.
fn touch(path: &Path) {
    if let Ok(file) = File::options().append(true).open(path) {
        let _: io::Result<()> = file.set_modified(SystemTime::now());
    }
}

/// Le type annoncé, sans ses paramètres, s'il a la forme d'un type ; sinon
/// [`UNKNOWN`]. Il repart tel quel dans un en-tête : rien d'autre n'y passe.
fn clean(declared: &str) -> String {
    let essence = declared
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let token = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "!#$&^_.+-".contains(c))
    };
    match essence.split_once('/') {
        Some((family, subtype)) if essence.len() <= TYPE_MAX && token(family) && token(subtype) => {
            essence
        }
        Some(_) | None => UNKNOWN.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::*;

    fn folder(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "telecommande-contenus-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn a_content_comes_back_by_its_name() {
        let store = Store::open(folder("retour"));
        let content = store
            .put(b"une image", "image/webp; charset=binary")
            .unwrap();
        assert_eq!(content, Content::of(b"une image", Kind::Image));
        let (bytes, content_type) = store.get(&content.name()).unwrap();
        assert_eq!(bytes, b"une image");
        assert_eq!(content_type, "image/webp");
    }

    #[test]
    fn the_same_content_is_kept_once() {
        let folder = folder("une-fois");
        let store = Store::open(folder.clone());
        let first = store.put(b"abc", "audio/ogg").unwrap();
        let second = store.put(b"abc", "audio/ogg").unwrap();
        assert_eq!(first, second);
        // Le contenu et son type : deux fichiers.
        assert_eq!(fs::read_dir(&folder).unwrap().count(), 2);
    }

    #[test]
    fn a_name_that_leads_nowhere_gives_nothing() {
        let store = Store::open(folder("nulle-part"));
        let content = store.put(b"abc", "image/png").unwrap();
        assert!(store.get("../etat").is_none());
        assert!(store.get(&format!("{}.type", content.name())).is_none());
        let other = Content::of(b"abd", Kind::Image);
        assert!(store.get(&other.name()).is_none());
    }

    #[test]
    fn a_ticket_that_lies_about_its_content_gives_nothing() {
        let folder = folder("menteur");
        let store = Store::open(folder.clone());
        let content = store.put(b"abc", "image/png").unwrap();
        fs::write(folder.join(content.name()), b"autre chose").unwrap();
        assert!(store.get(&content.name()).is_none());
    }

    #[test]
    fn nothing_and_too_much_are_refused() {
        let store = Store::open(folder("refus"));
        assert!(matches!(store.put(b"", "image/png"), Err(Refused::Empty)));
        let heavy = vec![0_u8; CONTENT_MAX.saturating_add(1)];
        assert!(matches!(
            store.put(&heavy, "image/png"),
            Err(Refused::TooBig)
        ));
    }

    #[test]
    fn a_type_that_is_not_one_becomes_unknown() {
        assert_eq!(clean("IMAGE/Webp"), "image/webp");
        assert_eq!(clean("image/svg+xml"), "image/svg+xml");
        assert_eq!(clean(""), UNKNOWN);
        assert_eq!(clean("image"), UNKNOWN);
        assert_eq!(clean("image/png\r\nX-Autre: 1"), UNKNOWN);
        assert_eq!(clean("image/a/b"), UNKNOWN);
    }

    #[test]
    fn the_oldest_contents_go_when_the_folder_is_full() {
        let folder = folder("plein");
        let store = Store::with_limit(folder.clone(), 25);
        let old = store.put(&[1; 10], "image/png").unwrap();
        let kept = store.put(&[2; 10], "image/png").unwrap();
        let long_ago = SystemTime::now()
            .checked_sub(Duration::from_secs(3600))
            .unwrap();
        File::options()
            .append(true)
            .open(folder.join(old.name()))
            .unwrap()
            .set_modified(long_ago)
            .unwrap();
        let new = store.put(&[3; 10], "image/png").unwrap();
        assert!(store.get(&old.name()).is_none());
        assert!(store.get(&kept.name()).is_some());
        assert!(store.get(&new.name()).is_some());
        assert!(!sidecar(&folder.join(old.name())).exists());
    }
}
