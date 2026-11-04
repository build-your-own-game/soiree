//! Un JSON écrit à la main : le rapport n'en a besoin que d'un sous-ensemble (objets,
//! listes, textes, entiers, booléens), et une dépendance de plus ne vaut pas ces
//! quelques lignes.

use std::fmt::Write as _;

/// Une valeur JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Json {
    Texte(String),
    Nombre(u64),
    Booleen(bool),
    Liste(Vec<Json>),
    Objet(Vec<(&'static str, Json)>),
}

impl Json {
    pub(crate) fn texte(s: &str) -> Json {
        Json::Texte(String::from(s))
    }

    /// Le texte JSON, indenté de deux espaces.
    pub(crate) fn ecrire(&self) -> String {
        let mut s = String::new();
        self.ecrire_dans(&mut s, 0);
        s.push('\n');
        s
    }

    fn ecrire_dans(&self, s: &mut String, niveau: usize) {
        let marge = |n: usize| "  ".repeat(n);
        match self {
            Json::Texte(t) => echapper(s, t),
            Json::Nombre(n) => {
                let _: Result<(), std::fmt::Error> = write!(s, "{n}");
            }
            Json::Booleen(b) => s.push_str(if *b { "true" } else { "false" }),
            Json::Liste(l) if l.is_empty() => s.push_str("[]"),
            Json::Liste(l) => {
                s.push_str("[\n");
                for (k, v) in l.iter().enumerate() {
                    s.push_str(&marge(niveau.saturating_add(1)));
                    v.ecrire_dans(s, niveau.saturating_add(1));
                    if k.saturating_add(1) < l.len() {
                        s.push(',');
                    }
                    s.push('\n');
                }
                s.push_str(&marge(niveau));
                s.push(']');
            }
            Json::Objet(o) if o.is_empty() => s.push_str("{}"),
            Json::Objet(o) => {
                s.push_str("{\n");
                for (k, (cle, v)) in o.iter().enumerate() {
                    s.push_str(&marge(niveau.saturating_add(1)));
                    echapper(s, cle);
                    s.push_str(": ");
                    v.ecrire_dans(s, niveau.saturating_add(1));
                    if k.saturating_add(1) < o.len() {
                        s.push(',');
                    }
                    s.push('\n');
                }
                s.push_str(&marge(niveau));
                s.push('}');
            }
        }
    }
}

/// Un texte JSON : guillemets, barre oblique inverse et caractères de contrôle
/// échappés (RFC 8259, § 7).
fn echapper(s: &mut String, t: &str) {
    s.push('"');
    for c in t.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _: Result<(), std::fmt::Error> = write!(s, "\\u{:04x}", u32::from(c));
            }
            c => s.push(c),
        }
    }
    s.push('"');
}

#[cfg(test)]
mod tests {
    use super::Json;

    #[test]
    fn echappe_et_imbrique() {
        let j = Json::Objet(vec![
            ("a", Json::texte("dit \"oui\"\n\u{1}")),
            (
                "b",
                Json::Liste(vec![Json::Nombre(3), Json::Booleen(false)]),
            ),
            ("c", Json::Liste(Vec::new())),
        ]);
        assert_eq!(
            j.ecrire(),
            "{\n  \"a\": \"dit \\\"oui\\\"\\n\\u0001\",\n  \"b\": [\n    3,\n    false\n  ],\n  \"c\": []\n}\n"
        );
    }
}
