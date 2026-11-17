//! Le contenu lourd : une image, un son, qu'un joueur apporte.
//!
//! La page le confie à la console, qui le range et rend un [`Content`] : un
//! ticket. La commande, l'état, la vue et le journal ne portent que ce
//! ticket. La machine ne voit jamais les octets : elle rejoue donc la même
//! partie, que le contenu soit encore là ou non.

use alloc::format;
use alloc::string::String;

use serde::{Deserialize, Serialize};

use crate::state::fnv;

/// Ce qu'est un contenu, d'après le type que la page a déclaré en le confiant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Une image (`image/…`).
    Image,
    /// Un son (`audio/…`).
    Sound,
    /// Une vidéo (`video/…`).
    Video,
    /// Tout le reste.
    Other,
}

impl Kind {
    const ALL: [Kind; 4] = [Kind::Image, Kind::Sound, Kind::Video, Kind::Other];

    /// Le genre d'un type de contenu (`image/webp`, `audio/ogg`…).
    pub fn of_type(content_type: &str) -> Self {
        let family = content_type.split('/').next().unwrap_or_default();
        match family.trim() {
            "image" => Kind::Image,
            "audio" => Kind::Sound,
            "video" => Kind::Video,
            _ => Kind::Other,
        }
    }

    /// Le nom du genre, dans le nom d'un contenu.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Image => "image",
            Kind::Sound => "sound",
            Kind::Video => "video",
            Kind::Other => "other",
        }
    }
}

/// Le ticket d'un contenu confié à la console.
///
/// Une page peut envoyer n'importe quel ticket : celui qui ne nomme rien ne
/// mène à rien, et la page qui le montre laisse un vide. Un ticket qui mène
/// à un contenu dit vrai sur sa taille et son genre : une règle peut s'y fier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Content {
    /// L'empreinte de ses octets.
    pub print: u64,
    /// Sa taille, en octets.
    pub len: u32,
    /// Ce que c'est.
    pub kind: Kind,
}

impl Content {
    /// Le ticket de ces octets : celui que la console rend quand une page les
    /// lui confie. Pour `suggest` et les tests, qui n'ont pas de console.
    pub fn of(bytes: &[u8], kind: Kind) -> Self {
        Self {
            print: fnv(bytes),
            len: u32::try_from(bytes.len()).unwrap_or(u32::MAX),
            kind,
        }
    }

    /// Le nom du contenu : la page le demande à la console à `/contenu/<nom>`.
    pub fn name(self) -> String {
        format!("{:016x}-{}.{}", self.print, self.len, self.kind.as_str())
    }

    /// Le ticket que ce nom écrit. `None` si ce n'est pas un nom de contenu.
    pub fn named(name: &str) -> Option<Self> {
        let (number, kind) = name.split_once('.')?;
        let (print, len) = number.split_once('-')?;
        let content = Self {
            print: u64::from_str_radix(print, 16).ok()?,
            len: len.parse().ok()?,
            kind: Kind::ALL.into_iter().find(|one| one.as_str() == kind)?,
        };
        // Un seul nom par ticket : ni majuscules, ni zéros en trop.
        (content.name() == name).then_some(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_reads_back_as_its_ticket() {
        let content = Content::of(b"une image", Kind::Image);
        assert_eq!(content.len, 9);
        assert_eq!(Content::named(&content.name()), Some(content));
    }

    #[test]
    fn the_same_bytes_get_the_same_ticket() {
        assert_eq!(
            Content::of(b"abc", Kind::Sound),
            Content::of(b"abc", Kind::Sound)
        );
        assert_ne!(
            Content::of(b"abc", Kind::Sound),
            Content::of(b"abd", Kind::Sound)
        );
    }

    #[test]
    fn only_one_name_leads_to_a_ticket() {
        for name in [
            "",
            "abc",
            "../etat",
            "00000000000000ff-3.image/..",
            "00000000000000FF-3.image",
            "ff-3.image",
            "00000000000000ff-03.image",
            "00000000000000ff-3.picture",
            "00000000000000ff-3.image.type",
            "00000000000000ff-4294967296.image",
        ] {
            assert_eq!(Content::named(name), None, "{name}");
        }
        assert!(Content::named("00000000000000ff-3.image").is_some());
    }

    #[test]
    fn a_type_tells_the_kind() {
        assert_eq!(Kind::of_type("image/webp"), Kind::Image);
        assert_eq!(Kind::of_type("audio/ogg; codecs=opus"), Kind::Sound);
        assert_eq!(Kind::of_type("video/mp4"), Kind::Video);
        assert_eq!(Kind::of_type("application/json"), Kind::Other);
        assert_eq!(Kind::of_type(""), Kind::Other);
    }
}
