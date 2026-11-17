//! Tirer au hasard une valeur de n'importe quel type que `serde` sait lire.
//!
//! Les pages envoient des commandes écrites par `serde` ; le banc se sert du
//! même chemin, à l'envers, pour en fabriquer : le type dit lui-même quelles
//! variantes et quels champs existent. Une variante ajoutée demain sera tirée
//! sans que personne y pense.
//!
//! Les valeurs sont celles qui font mal : zéro, les bornes, un flottant qui
//! n'est pas un nombre, un texte vide ou trop long.

use alloc::borrow::ToOwned;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use serde::de::{
    self, DeserializeOwned, DeserializeSeed, Deserializer, EnumAccess, IntoDeserializer, MapAccess,
    SeqAccess, VariantAccess, Visitor,
};

use crate::rng::Rng;

/// Au-delà de cette profondeur, un type qui se contient lui-même s'arrête au
/// plus court : la première variante, une liste vide, rien.
const DEPTH_MAX: u8 = 12;

const TEXTS: [&str; 10] = [
    "",
    " ",
    "Ada",
    "  Zoé  ",
    "3",
    "🎉🎉🎉",
    "<b>gras</b> & \"guillemets\"",
    "deux\nlignes",
    "Un texte beaucoup trop long, de ceux qu'un joueur tape pour voir si le jeu tient le coup \
     quand on lui donne bien plus que ce que la page laisse normalement écrire.",
    "\u{0}",
];

const CHARS: [char; 6] = ['a', ' ', 'é', '🎉', '\n', '\u{0}'];

const FLOATS: [f64; 10] = [
    0.0,
    -0.0,
    1.0,
    -1.0,
    0.5,
    1.0e-30,
    1.0e30,
    f64::NAN,
    f64::INFINITY,
    f64::NEG_INFINITY,
];

/// Ce qui a empêché un tirage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    /// Le type ne dit pas sa forme : aucune valeur ne sera jamais tirée.
    HiddenShape,
    /// Le type a refusé la valeur tirée : un autre tirage passera.
    Value,
}

/// Pourquoi une valeur n'a pas pu être tirée.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub cause: Cause,
    pub why: String,
}

impl fmt::Display for Refusal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.why)
    }
}

impl core::error::Error for Refusal {}

impl de::Error for Refusal {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self {
            cause: Cause::Value,
            why: message.to_string(),
        }
    }
}

/// Tire une valeur de ce type.
///
/// Refuse un type qui ne dit pas sa forme à `serde` : une enum « sans
/// étiquette » ou à étiquette interne (`#[serde(untagged)]`, `#[serde(tag)]`),
/// un champ aplati (`#[serde(flatten)]`), une valeur libre
/// (`serde_json::Value`). Et un type qui refuse la valeur tirée (un `usize`
/// trop grand pour un navigateur, par exemple) rend son refus.
pub fn draw<T: DeserializeOwned>(rng: &mut Rng) -> Result<T, Refusal> {
    T::deserialize(Draw { rng, depth: 0 })
}

struct Draw<'rng> {
    rng: &'rng mut Rng,
    depth: u8,
}

impl Draw<'_> {
    fn deeper(&mut self) -> Draw<'_> {
        Draw {
            rng: self.rng,
            depth: self.depth.saturating_add(1),
        }
    }

    fn at_bottom(&self) -> bool {
        self.depth >= DEPTH_MAX
    }

    /// Un entier : souvent une borne ou un petit nombre, parfois n'importe quoi.
    fn integer(&mut self) -> u64 {
        match self.rng.below(10) {
            0 => 0,
            1 => 1,
            2 => u64::MAX,
            3 => u64::from(u32::MAX),
            4..=6 => self.rng.below(16),
            7 | 8 => self.rng.below(1 << 32),
            _ => self.rng.next_u64(),
        }
    }

    fn float(&mut self) -> f64 {
        if self.rng.chance(1, 2) {
            return self.rng.pick(&FLOATS).copied().unwrap_or(0.0);
        }
        let [low, high, ..] = self.rng.next_u64().to_le_bytes();
        // De -1 à 1, puis parfois bien plus grand.
        let unit = f64::from(u16::from_le_bytes([low, high])) / 32_767.5 - 1.0;
        if self.rng.chance(1, 4) {
            unit * 1000.0
        } else {
            unit
        }
    }

    fn length(&mut self) -> usize {
        if self.at_bottom() {
            0
        } else if self.rng.chance(1, 20) {
            24
        } else {
            self.rng.index(4)
        }
    }
}

/// Les entiers : les octets de poids faible du tirage, sans conversion qui
/// puisse perdre un signe en silence.
macro_rules! integers {
    ($($method:ident => $visit:ident, $kind:ty, $bytes:expr;)*) => {
        $(
            fn $method<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Refusal> {
                let wide = self.integer().to_le_bytes();
                let mut narrow = [0_u8; $bytes];
                for (to, from) in narrow.iter_mut().zip(wide) {
                    *to = from;
                }
                visitor.$visit(<$kind>::from_le_bytes(narrow))
            }
        )*
    };
}

impl<'de> Deserializer<'de> for Draw<'_> {
    type Error = Refusal;

    integers! {
        deserialize_u8 => visit_u8, u8, 1;
        deserialize_u16 => visit_u16, u16, 2;
        deserialize_u32 => visit_u32, u32, 4;
        deserialize_u64 => visit_u64, u64, 8;
        deserialize_i8 => visit_i8, i8, 1;
        deserialize_i16 => visit_i16, i16, 2;
        deserialize_i32 => visit_i32, i32, 4;
        deserialize_i64 => visit_i64, i64, 8;
    }

    fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Refusal> {
        Err(Refusal {
            cause: Cause::HiddenShape,
            why: "ce type ne dit pas sa forme : la console ne sait pas en tirer une valeur. Une \
                  enum de commandes s'écrit sans `#[serde(untagged)]`, sans `#[serde(tag = …)]` \
                  et sans `#[serde(flatten)]`."
                .to_owned(),
        })
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Refusal> {
        visitor.visit_bool(self.rng.chance(1, 2))
    }

    fn deserialize_f32<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Refusal> {
        visitor.visit_f64(self.float())
    }

    fn deserialize_f64<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Refusal> {
        visitor.visit_f64(self.float())
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Refusal> {
        visitor.visit_char(self.rng.pick(&CHARS).copied().unwrap_or('a'))
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Refusal> {
        self.deserialize_string(visitor)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Refusal> {
        visitor.visit_string(self.rng.pick(&TEXTS).copied().unwrap_or("").to_owned())
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Refusal> {
        self.deserialize_byte_buf(visitor)
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Refusal> {
        let length = self.length();
        let bytes: Vec<u8> = (0..length)
            .map(|_| {
                let [byte, ..] = self.rng.next_u64().to_le_bytes();
                byte
            })
            .collect();
        visitor.visit_byte_buf(bytes)
    }

    fn deserialize_option<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Refusal> {
        if self.at_bottom() || self.rng.chance(1, 3) {
            visitor.visit_none()
        } else {
            visitor.visit_some(self.deeper())
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Refusal> {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, Refusal> {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        mut self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, Refusal> {
        visitor.visit_newtype_struct(self.deeper())
    }

    fn deserialize_seq<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Refusal> {
        let left = self.length();
        visitor.visit_seq(Items {
            draw: self.deeper(),
            left,
        })
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        mut self,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Refusal> {
        visitor.visit_seq(Items {
            draw: self.deeper(),
            left: len,
        })
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Refusal> {
        self.deserialize_tuple(len, visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(mut self, visitor: V) -> Result<V::Value, Refusal> {
        let left = self.length().min(2);
        visitor.visit_map(Items {
            draw: self.deeper(),
            left,
        })
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Refusal> {
        // Une structure dérivée se lit aussi comme la suite de ses champs.
        self.deserialize_tuple(fields.len(), visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        mut self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Refusal> {
        if variants.is_empty() {
            return Err(Refusal {
                cause: Cause::HiddenShape,
                why: alloc::format!("l'enum `{name}` n'a aucune variante : aucune valeur n'existe"),
            });
        }
        let rank = if self.at_bottom() {
            0
        } else {
            self.rng.index(variants.len())
        };
        visitor.visit_enum(Variant {
            draw: self.deeper(),
            rank: u32::try_from(rank).unwrap_or(0),
        })
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Refusal> {
        self.deserialize_any(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Refusal> {
        visitor.visit_unit()
    }
}

/// Les éléments d'une liste, d'un tuple ou d'une table.
struct Items<'rng> {
    draw: Draw<'rng>,
    left: usize,
}

impl<'de> SeqAccess<'de> for Items<'_> {
    type Error = Refusal;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Refusal> {
        let Some(left) = self.left.checked_sub(1) else {
            return Ok(None);
        };
        self.left = left;
        seed.deserialize(self.draw.deeper()).map(Some)
    }
}

impl<'de> MapAccess<'de> for Items<'_> {
    type Error = Refusal;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Refusal> {
        self.next_element_seed(seed)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Refusal> {
        seed.deserialize(self.draw.deeper())
    }
}

/// Une variante, désignée par son rang.
struct Variant<'rng> {
    draw: Draw<'rng>,
    rank: u32,
}

impl<'de, 'rng> EnumAccess<'de> for Variant<'rng> {
    type Error = Refusal;
    type Variant = Draw<'rng>;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Draw<'rng>), Refusal> {
        let rank = IntoDeserializer::<Refusal>::into_deserializer(self.rank);
        Ok((seed.deserialize(rank)?, self.draw))
    }
}

impl<'de> VariantAccess<'de> for Draw<'_> {
    type Error = Refusal;

    fn unit_variant(self) -> Result<(), Refusal> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Refusal> {
        seed.deserialize(self)
    }

    fn tuple_variant<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value, Refusal> {
        self.deserialize_tuple(len, visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Refusal> {
        self.deserialize_tuple(fields.len(), visitor)
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use alloc::collections::{BTreeMap, BTreeSet};
    use alloc::format;

    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize)]
    enum Stroke {
        Forehand,
        Backhand,
    }

    #[derive(Debug, Deserialize)]
    #[expect(dead_code, reason = "les champs ne servent qu'à être tirés")]
    struct Swing {
        stroke: Stroke,
        power: f32,
        hand: Option<bool>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "snake_case")]
    #[expect(dead_code, reason = "les champs ne servent qu'à être tirés")]
    enum Command {
        Name(String),
        Tap,
        Swing(Swing),
        Sit { row: u8, side: i16 },
        Vote(Vec<u32>),
        Scores(BTreeMap<u8, (char, u64)>),
        Stop,
    }

    #[derive(Debug, Deserialize)]
    #[expect(dead_code, reason = "les champs ne servent qu'à être tirés")]
    enum Tree {
        Leaf,
        Node(Box<Tree>, Box<Tree>),
    }

    #[derive(Debug, Deserialize)]
    #[serde(tag = "kind")]
    enum Tagged {
        One,
    }

    #[test]
    fn every_variant_of_a_command_gets_drawn() {
        let mut rng = Rng::new(1);
        let mut seen = BTreeSet::new();
        for _ in 0..500 {
            let command: Command = draw(&mut rng).unwrap();
            let text = format!("{command:?}");
            let name = text.split(['(', ' ', '{']).next().unwrap_or("").to_owned();
            let _: bool = seen.insert(name);
        }
        let seen: Vec<&str> = seen.iter().map(String::as_str).collect();
        assert_eq!(
            seen,
            ["Name", "Scores", "Sit", "Stop", "Swing", "Tap", "Vote"]
        );
    }

    #[test]
    fn the_values_that_hurt_come_up() {
        let mut rng = Rng::new(2);
        let (mut not_a_number, mut empty, mut most) = (false, false, false);
        for _ in 0..2000 {
            match draw::<Command>(&mut rng).unwrap() {
                Command::Swing(swing) => not_a_number |= swing.power.is_nan(),
                Command::Name(name) => empty |= name.is_empty(),
                Command::Vote(votes) => most |= votes.contains(&u32::MAX),
                Command::Tap | Command::Sit { .. } | Command::Scores(_) | Command::Stop => {}
            }
        }
        assert!(not_a_number && empty && most);
    }

    #[test]
    fn the_same_seed_draws_the_same_commands() {
        let draws = |seed| {
            let mut rng = Rng::new(seed);
            (0..50)
                .map(|_| format!("{:?}", draw::<Command>(&mut rng).unwrap()))
                .collect::<Vec<_>>()
        };
        assert_eq!(draws(3), draws(3));
        assert_ne!(draws(3), draws(4));
    }

    #[test]
    fn a_type_that_contains_itself_ends() {
        let mut rng = Rng::new(5);
        for _ in 0..200 {
            let _: Tree = draw(&mut rng).unwrap();
        }
    }

    #[test]
    fn a_type_that_hides_its_shape_is_refused_with_the_way_out() {
        let refusal = draw::<Tagged>(&mut Rng::new(6)).unwrap_err();
        assert_eq!(refusal.cause, Cause::HiddenShape);
        assert!(refusal.why.contains("#[serde(tag"));
    }
}
