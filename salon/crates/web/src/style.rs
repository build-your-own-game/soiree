//! Values authored by a game, serialized to CSS only at the browser boundary.
//! Property names are supplied by `soiree dom`, with their value type.
//!
//! A color property cannot receive a length or an arbitrary string:
//! ```compile_fail
//! use salon_web::style::{Color, Length, Property};
//! fn wrong(element: &web_sys::HtmlElement, color: Property<Color>) {
//!     color.set(element, &Length::px(12.0).unwrap());
//! }
//! ```
//! ```compile_fail
//! use salon_web::style::{Color, Property};
//! fn wrong(element: &web_sys::HtmlElement, color: Property<Color>) {
//!     color.set(element, &"not a color");
//! }

use core::{fmt, marker::PhantomData};
use wasm_bindgen::JsValue;
use web_sys::HtmlElement;

mod sealed {
    pub trait Sealed {}
}
pub trait Value: sealed::Sealed + fmt::Display {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidValue;

impl fmt::Display for InvalidValue {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("invalid CSS value")
    }
}

impl std::error::Error for InvalidValue {}
impl From<InvalidValue> for JsValue {
    fn from(value: InvalidValue) -> Self {
        Self::from_str(&value.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color([u8; 3]);

impl Color {
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self([red, green, blue])
    }
    pub const fn rgb24(rgb: u32) -> Result<Self, InvalidValue> {
        let [high, red, green, blue] = rgb.to_be_bytes();
        if high == 0 {
            Ok(Self::rgb(red, green, blue))
        } else {
            Err(InvalidValue)
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [r, g, b] = self.0;
        write!(out, "#{r:02x}{g:02x}{b:02x}")
    }
}

impl core::str::FromStr for Color {
    type Err = InvalidValue;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let digits = text
            .strip_prefix('#')
            .filter(|s| s.len() == 6 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or(InvalidValue)?;
        Self::rgb24(u32::from_str_radix(digits, 16).map_err(|_| InvalidValue)?)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Length(f64);
impl Length {
    pub fn px(value: f64) -> Result<Self, InvalidValue> {
        value.is_finite().then_some(Self(value)).ok_or(InvalidValue)
    }
}
impl fmt::Display for Length {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}px", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Number(f64);
impl Number {
    pub fn new(value: f64) -> Result<Self, InvalidValue> {
        value.is_finite().then_some(Self(value)).ok_or(InvalidValue)
    }
}
impl fmt::Display for Number {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(out)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Time(u64);
impl Time {
    pub const fn milliseconds(value: u64) -> Self {
        Self(value)
    }
}
impl fmt::Display for Time {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}ms", self.0)
    }
}

/// A URL inside one quoted CSS string. Quotes and slashes in user-supplied
/// image data never become a second CSS token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image(String);
impl Image {
    pub fn url(value: &str) -> Result<Self, InvalidValue> {
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err(InvalidValue);
        }
        Ok(Self(value.to_owned()))
    }
}
impl fmt::Display for Image {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("url(\"")?;
        for ch in self.0.chars() {
            if matches!(ch, '\\' | '"') {
                out.write_str("\\")?;
            }
            write!(out, "{ch}")?;
        }
        out.write_str("\")")
    }
}

impl sealed::Sealed for Color {}
impl sealed::Sealed for Length {}
impl sealed::Sealed for Number {}
impl sealed::Sealed for Time {}
impl sealed::Sealed for Image {}
impl Value for Color {}
impl Value for Length {}
impl Value for Number {}
impl Value for Time {}
impl Value for Image {}

/// A generated property and the only category of value Rust may assign.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Property<V = ()> {
    name: &'static str,
    kind: PhantomData<fn() -> V>,
}

impl<V> Property<V> {
    /// Used only by `soiree dom` generated modules (enforced by Clippy).
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            kind: PhantomData,
        }
    }

    #[expect(
        clippy::disallowed_methods,
        reason = "typed CSS serialization boundary"
    )]
    pub fn remove(self, element: &HtmlElement) -> Result<(), JsValue> {
        element.style().remove_property(self.name).map(|_| ())
    }
}

impl<V: Value> Property<V> {
    #[expect(
        clippy::disallowed_methods,
        reason = "typed CSS serialization boundary"
    )]
    pub fn set(self, element: &HtmlElement, value: &V) -> Result<(), JsValue> {
        element.style().set_property(self.name, &value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_preserve_units_and_refuse_non_finite_layout() {
        assert_eq!(Color::rgb(0, 127, 255).to_string(), "#007fff");
        assert_eq!(Color::rgb24(0x00_7fff).unwrap(), Color::rgb(0, 127, 255));
        assert!(Color::rgb24(0x1_000000).is_err());
        for invalid in ["#fffff", "#+abcde", "#ffffff ", "#zzzzzz"] {
            assert!(invalid.parse::<Color>().is_err());
        }
        assert_eq!(Length::px(-12.5).unwrap().to_string(), "-12.5px");
        assert_eq!(Number::new(0.85).unwrap().to_string(), "0.85");
        assert_eq!(Time::milliseconds(240).to_string(), "240ms");
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(Length::px(value).is_err());
            assert!(Number::new(value).is_err());
        }
    }

    #[test]
    fn image_strings_cannot_escape_their_css_url() {
        assert_eq!(
            Image::url("image.png").unwrap().to_string(),
            "url(\"image.png\")"
        );
        assert_eq!(
            Image::url("x\");color:red;/*\\").unwrap().to_string(),
            "url(\"x\\\");color:red;/*\\\\\")"
        );
        for value in ["", "x\ncolor:red", "x\0", "x\r"] {
            assert!(Image::url(value).is_err());
        }
    }
}
