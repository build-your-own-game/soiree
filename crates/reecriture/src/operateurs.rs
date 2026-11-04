//! Le nom et la proposition de chaque instruction, tirés de la liste de `wasmparser`
//! elle-même : une instruction ajoutée par une version de `wasmparser` a son nom et
//! sa proposition sans qu'on l'écrive ici, et n'est admise que si sa proposition
//! l'est (§ 3.1).

use wasmparser::Operator;

macro_rules! definir {
    ($( @$proposition:ident $op:ident $({ $($arg:ident: $argty:ty),* })? => $visite:ident ($($ann:tt)*))*) => {
        /// Le nom de l'instruction, celui de son variant dans `wasmparser::Operator`.
        pub(crate) fn nom(op: &Operator<'_>) -> &'static str {
            match op {
                $( Operator::$op { .. } => stringify!($op), )*
                _ => "inconnue",
            }
        }

        /// La proposition qui a introduit l'instruction, telle que `wasmparser` la
        /// nomme (`mvp`, `sign_extension`, `simd`…).
        pub(crate) fn proposition(op: &Operator<'_>) -> &'static str {
            match op {
                $( Operator::$op { .. } => stringify!($proposition), )*
                _ => "inconnue",
            }
        }
    };
}

wasmparser::for_each_operator!(definir);

/// L'instruction est-elle dans la liste de § 3.1 ? WebAssembly 1.0, extension de
/// signe, conversions saturantes, et la mémoire en bloc sans les instructions de
/// table. Les types référence ne servent qu'à `call_indirect`, qui est de la 1.0 :
/// aucune instruction de leur proposition n'est admise.
pub(crate) fn admise(op: &Operator<'_>) -> bool {
    match proposition(op) {
        "mvp" | "sign_extension" | "saturating_float_to_int" => true,
        "bulk_memory" => matches!(
            op,
            Operator::MemoryInit { .. }
                | Operator::DataDrop { .. }
                | Operator::MemoryCopy { .. }
                | Operator::MemoryFill { .. }
        ),
        _ => false,
    }
}

/// La famille d'une instruction admise, pour le constat (§ 3.6) : la base de
/// WebAssembly 1.0 sans flottant, ses flottants, puis une famille par proposition.
pub(crate) fn famille(op: &Operator<'_>) -> &'static str {
    match proposition(op) {
        "mvp" => {
            let n = nom(op);
            if n.contains("F32") || n.contains("F64") {
                "flottants"
            } else {
                "base"
            }
        }
        "sign_extension" => "extension_de_signe",
        "saturating_float_to_int" => "conversions_saturantes",
        "bulk_memory" => "memoire_en_bloc",
        autre => autre,
    }
}
