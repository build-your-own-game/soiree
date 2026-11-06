//! Une erreur levée par un appel du module, ramenée à un piège brut
//! ([`moteur_commun::Piege`]) : le protocole du commun la classe ensuite.
//!
//! Le navigateur ne donne pas de code de piège : un `WebAssembly.RuntimeError` porte un
//! message, propre à chaque moteur. La pile épuisée n'est pas un `RuntimeError` : un
//! `RangeError` dans V8 (Chrome) et `JavaScriptCore` (Safari), un `InternalError: too much
//! recursion` dans `SpiderMonkey` (Firefox) (PHASE-3, § 2.3).
//!
//! Le message ne sert qu'à nommer le piège, et seulement en dernier recours : la table
//! [`MESSAGES`] ramène ceux que l'on a constatés au nom que wasmtime donne au même
//! piège ([`codes`]), pour que la faute `Piege { code }` d'une entrée écartée s'écrive
//! pareil au journal dans les deux moteurs (E20, lot 3b). Un message qui désigne
//! plusieurs pièges de wasmtime ([`AMBIGUS`]), ou qu'aucune table ne connaît, garde le
//! texte du navigateur, précédé de [`PREFIXE_NON_RAMENE`] : la différence se voit, elle
//! ne se cache pas.

use moteur_commun::Piege;

/// Les noms que wasmtime 50.0.1 donne à ses pièges (`wasmtime_environ::Trap`, son
/// `Display`), tels que `moteur-wasmtime` les met dans [`Piege::Autre`].
pub mod codes {
    /// `unreachable`.
    pub const UNREACHABLE: &str = "wasm trap: wasm `unreachable` instruction executed";
    /// Une division entière, ou un reste, par zéro.
    pub const DIVISION_PAR_ZERO: &str = "wasm trap: integer divide by zero";
    /// Un débordement entier : `i32.div_s` de `i32::MIN` par -1, ou la conversion d'un
    /// flottant hors des bornes de l'entier.
    pub const DEBORDEMENT_ENTIER: &str = "wasm trap: integer overflow";
    /// La conversion d'un NaN en entier.
    pub const CONVERSION_INVALIDE: &str = "wasm trap: invalid conversion to integer";
    /// Un `call_indirect` au-delà de la table.
    pub const TABLE_HORS_BORNES: &str = "wasm trap: undefined element: out of bounds table access";
    /// Un `call_indirect` vers un élément nul de la table.
    pub const ELEMENT_NUL: &str = "wasm trap: uninitialized element";
    /// Un `call_indirect` vers une fonction d'un autre type.
    pub const SIGNATURE: &str = "wasm trap: indirect call type mismatch";
}

/// Ce qui précède le message d'un piège que la table ne ramène pas à wasmtime.
pub const PREFIXE_NON_RAMENE: &str = "navigateur : ";

/// Ce qu'un message de `WebAssembly.RuntimeError` devient : un accès hors de la
/// mémoire, ou un piège nommé comme wasmtime le nomme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ramene {
    /// [`Piege::HorsMemoire`].
    HorsMemoire,
    /// [`Piege::Autre`], sous ce nom.
    Code(&'static str),
}

/// Le moteur JavaScript d'un navigateur : chacun a ses messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoteurJs {
    /// V8 (Chrome).
    V8,
    /// `SpiderMonkey` (Firefox).
    SpiderMonkey,
    /// `JavaScriptCore` (Safari). Aucun de ses messages n'est constaté : le banc n'a pas
    /// encore pu piloter Safari (PHASE-3, lot 3c, question 8). Ses pièges gardent le
    /// texte du navigateur ([`PREFIXE_NON_RAMENE`]), et les essais du banc échouent pour
    /// lui tant qu'aucun n'est rangé.
    JavaScriptCore,
}

impl MoteurJs {
    /// Le moteur d'un navigateur, d'après son `navigator.userAgent` ; `None` pour un
    /// navigateur qu'on ne reconnaît pas. L'agent de Chrome dit aussi `Safari/` : Chrome
    /// se reconnaît d'abord.
    pub fn depuis_agent(agent: &str) -> Option<MoteurJs> {
        if agent.contains("Firefox/") {
            Some(MoteurJs::SpiderMonkey)
        } else if agent.contains("Chrome/") {
            Some(MoteurJs::V8)
        } else if agent.contains("Safari/") && agent.contains("Version/") {
            Some(MoteurJs::JavaScriptCore)
        } else {
            None
        }
    }
}

/// Les messages de `WebAssembly.RuntimeError` constatés, le moteur qui les lève, et ce
/// qu'ils deviennent.
///
/// Ceux de V8 sont constatés dans Chrome 156.0.8078.80 le 2026-11-05 par
/// `soiree-banc machine` (lot 3a), ceux de `SpiderMonkey` dans Firefox 158.0.1 le même
/// jour (lot 3b, `--navigateur firefox`), un piège provoqué par scénario
/// (`essais/machine.wat`, `essais/pieges.txt`). Aucun message n'est commun aux deux
/// moteurs : le classement n'a pas à savoir lequel tourne. Ceux de `JavaScriptCore`
/// (Safari) s'y ajouteront quand le banc pourra le piloter (lot 3c : « pas exécuté »).
pub const MESSAGES: &[(MoteurJs, &str, Ramene)] = &[
    // V8 (Chrome).
    (
        MoteurJs::V8,
        "memory access out of bounds",
        Ramene::HorsMemoire,
    ),
    (
        MoteurJs::V8,
        "unreachable",
        Ramene::Code(codes::UNREACHABLE),
    ),
    (
        MoteurJs::V8,
        "divide by zero",
        Ramene::Code(codes::DIVISION_PAR_ZERO),
    ),
    (
        MoteurJs::V8,
        "remainder by zero",
        Ramene::Code(codes::DIVISION_PAR_ZERO),
    ),
    (
        MoteurJs::V8,
        "divide result unrepresentable",
        Ramene::Code(codes::DEBORDEMENT_ENTIER),
    ),
    (
        MoteurJs::V8,
        "table index is out of bounds",
        Ramene::Code(codes::TABLE_HORS_BORNES),
    ),
    (
        MoteurJs::V8,
        "null function",
        Ramene::Code(codes::ELEMENT_NUL),
    ),
    (
        MoteurJs::V8,
        "function signature mismatch",
        Ramene::Code(codes::SIGNATURE),
    ),
    // SpiderMonkey (Firefox) : ses messages suivent ceux de wasmtime de plus près.
    (
        MoteurJs::SpiderMonkey,
        "unreachable executed",
        Ramene::Code(codes::UNREACHABLE),
    ),
    (
        MoteurJs::SpiderMonkey,
        "integer divide by zero",
        Ramene::Code(codes::DIVISION_PAR_ZERO),
    ),
    (
        MoteurJs::SpiderMonkey,
        "integer overflow",
        Ramene::Code(codes::DEBORDEMENT_ENTIER),
    ),
    (
        MoteurJs::SpiderMonkey,
        "invalid conversion to integer",
        Ramene::Code(codes::CONVERSION_INVALIDE),
    ),
    (
        MoteurJs::SpiderMonkey,
        "indirect call to null",
        Ramene::Code(codes::ELEMENT_NUL),
    ),
    (
        MoteurJs::SpiderMonkey,
        "indirect call signature mismatch",
        Ramene::Code(codes::SIGNATURE),
    ),
];

/// Les messages constatés qui désignent plusieurs pièges de wasmtime, avec ceux qu'ils
/// peuvent désigner : ils gardent le texte du navigateur ([`PREFIXE_NON_RAMENE`]), et la
/// différence avec wasmtime est connue. Un message ni dans [`MESSAGES`] ni ici n'a pas
/// été constaté : les essais du banc le refusent, jusqu'à ce qu'on le range dans l'une
/// des deux tables. Ils refusent aussi un message d'ici levé pour un piège que wasmtime
/// nomme autrement que l'un des siens.
///
/// - V8 : la conversion d'un NaN en entier (wasmtime : `CONVERSION_INVALIDE`) et celle
///   d'un flottant hors des bornes (`DEBORDEMENT_ENTIER`). Un module compilé par Rust
///   n'y tombe pas : `as` y est une conversion saturée (`i32.trunc_sat_f32_s`), qui ne
///   piège jamais.
/// - `SpiderMonkey` : `index out of bounds`, d'un accès hors de la mémoire (wasmtime :
///   [`Piege::HorsMemoire`], la faute `HorsMemoire`) comme d'un `call_indirect` au-delà
///   de la table (`TABLE_HORS_BORNES`). Un module compilé par Rust sûr n'y tombe pas :
///   ses accès sont bornés avant, et un dépassement y est une panique.
pub const AMBIGUS: &[(MoteurJs, &str, &[Ramene])] = &[
    (
        MoteurJs::V8,
        "float unrepresentable in integer range",
        &[
            Ramene::Code(codes::CONVERSION_INVALIDE),
            Ramene::Code(codes::DEBORDEMENT_ENTIER),
        ],
    ),
    (
        MoteurJs::SpiderMonkey,
        "index out of bounds",
        &[Ramene::HorsMemoire, Ramene::Code(codes::TABLE_HORS_BORNES)],
    ),
];

/// Ce que le navigateur a levé pendant un appel, lu sur l'erreur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErreurJs<'a> {
    /// Un `WebAssembly.RuntimeError`, et son message.
    Execution(&'a str),
    /// Un `RangeError` (V8, `JavaScriptCore`), et son message.
    Portee(&'a str),
    /// Une autre erreur, par son nom (`error.name`) et son message.
    Autre {
        /// `error.name`, ou le type de la valeur levée si ce n'est pas une erreur.
        nom: &'a str,
        /// `error.message`, ou la valeur levée en texte.
        message: &'a str,
    },
}

/// Le piège brut d'une erreur levée par `soiree_boite` ou `soiree_appel`.
///
/// - `RuntimeError` : un accès hors de la mémoire ou un piège nommé, par [`MESSAGES`] ;
///   un message inconnu reste un piège du module ([`Piege::Autre`]), sous le texte du
///   navigateur.
/// - `RangeError`, ou `InternalError` (`SpiderMonkey`, `too much recursion`) : la pile
///   native épuisée ([`Piege::PileNative`]), une panne de l'outil : la profondeur
///   réécrite devait piquer avant.
/// - Toute autre erreur n'est pas un piège du module : [`Piege::Moteur`].
///
/// Jamais [`Piege::Interrompu`] : le navigateur ne sait pas interrompre un appel sur
/// son fil ; le garde-fou mural est le Worker que la page tue (PHASE-3, § 2.3, lot 10a).
pub fn classer(e: ErreurJs<'_>) -> Piege {
    match e {
        ErreurJs::Execution(message) => match MESSAGES.iter().find(|(_, m, _)| *m == message) {
            Some((_, _, Ramene::HorsMemoire)) => Piege::HorsMemoire,
            Some((_, _, Ramene::Code(code))) => Piege::Autre(String::from(*code)),
            None => Piege::Autre(format!("{PREFIXE_NON_RAMENE}{message}")),
        },
        ErreurJs::Portee(_) => Piege::PileNative,
        ErreurJs::Autre { nom, message } => {
            if nom == "InternalError" && message.contains("too much recursion") {
                Piege::PileNative
            } else {
                Piege::Moteur(format!("le navigateur a levé {nom} : {message}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use moteur_commun::Piege;

    use super::{
        AMBIGUS, ErreurJs, MESSAGES, MoteurJs, PREFIXE_NON_RAMENE, Ramene, classer, codes,
    };

    #[test]
    fn un_acces_hors_de_la_memoire() {
        assert_eq!(
            classer(ErreurJs::Execution("memory access out of bounds")),
            Piege::HorsMemoire
        );
    }

    #[test]
    fn un_piege_connu_prend_le_nom_de_wasmtime() {
        assert_eq!(
            classer(ErreurJs::Execution("unreachable")),
            Piege::Autre(String::from(codes::UNREACHABLE))
        );
        assert_eq!(
            classer(ErreurJs::Execution("remainder by zero")),
            Piege::Autre(String::from(codes::DIVISION_PAR_ZERO))
        );
        assert_eq!(
            classer(ErreurJs::Execution("indirect call to null")),
            Piege::Autre(String::from(codes::ELEMENT_NUL))
        );
    }

    #[test]
    fn le_moteur_d_un_navigateur() {
        let chrome = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/156.0.0.0 Safari/537.36";
        let firefox =
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:158.0) Gecko/20100101 Firefox/158.0";
        let safari = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/27.2 Safari/605.1.15";
        assert_eq!(MoteurJs::depuis_agent(chrome), Some(MoteurJs::V8));
        assert_eq!(
            MoteurJs::depuis_agent(firefox),
            Some(MoteurJs::SpiderMonkey)
        );
        assert_eq!(
            MoteurJs::depuis_agent(safari),
            Some(MoteurJs::JavaScriptCore)
        );
        assert_eq!(MoteurJs::depuis_agent("curl/8.7.1"), None);
    }

    /// Un message inconnu reste un piège du module, et se voit : il n'est pas déguisé
    /// en un nom de wasmtime.
    #[test]
    fn un_message_inconnu_garde_le_texte_du_navigateur() {
        assert_eq!(
            classer(ErreurJs::Execution(
                "float unrepresentable in integer range"
            )),
            Piege::Autre(format!(
                "{PREFIXE_NON_RAMENE}float unrepresentable in integer range"
            ))
        );
    }

    #[test]
    fn la_pile_native_de_chaque_moteur() {
        assert_eq!(
            classer(ErreurJs::Portee("Maximum call stack size exceeded")),
            Piege::PileNative
        );
        assert_eq!(
            classer(ErreurJs::Autre {
                nom: "InternalError",
                message: "too much recursion"
            }),
            Piege::PileNative
        );
    }

    #[test]
    fn une_autre_erreur_n_est_pas_un_piege_du_module() {
        let p = classer(ErreurJs::Autre {
            nom: "TypeError",
            message: "Cannot convert a BigInt value to a number",
        });
        assert!(matches!(p, Piege::Moteur(m) if m.contains("TypeError")));
    }

    /// Chaque message n'a qu'une ligne, dans une seule des deux tables : elles ne se
    /// contredisent pas.
    #[test]
    fn chaque_message_une_fois() {
        for (i, (_, m, _)) in MESSAGES.iter().enumerate() {
            assert!(
                MESSAGES.iter().skip(i + 1).all(|(_, n, _)| n != m),
                "{m} deux fois"
            );
            assert!(
                AMBIGUS.iter().all(|(_, a, _)| a != m),
                "{m} ramené et ambigu"
            );
        }
        for (i, (_, a, designes)) in AMBIGUS.iter().enumerate() {
            assert!(
                AMBIGUS.iter().skip(i + 1).all(|(_, b, _)| b != a),
                "{a} deux fois"
            );
            // Ambigu : il désigne au moins deux pièges de wasmtime.
            assert!(designes.len() >= 2, "{a} : {designes:?}");
            assert_eq!(
                classer(ErreurJs::Execution(a)),
                Piege::Autre(format!("{PREFIXE_NON_RAMENE}{a}"))
            );
        }
        // Chaque moteur a son accès hors de la mémoire, ramené ou ambigu.
        for moteur in [MoteurJs::V8, MoteurJs::SpiderMonkey] {
            assert!(
                MESSAGES
                    .iter()
                    .any(|(x, _, r)| *x == moteur && *r == Ramene::HorsMemoire)
                    || AMBIGUS
                        .iter()
                        .any(|(x, _, d)| *x == moteur && d.contains(&Ramene::HorsMemoire)),
                "{moteur:?}"
            );
        }
    }
}
