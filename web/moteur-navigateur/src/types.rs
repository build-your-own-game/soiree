//! Le type de chaque globale exportée d'un module, lu dans ses octets.
//!
//! Le navigateur rend la valeur d'une globale (`WebAssembly.Global.prototype.value`),
//! pas son type : un `i64` y est un `BigInt`, mais un `i32`, un `f32` et un `f64` y
//! sont tous trois un nombre. `WebAssembly.Global.prototype.type` (la réflexion des
//! types) n'est pas livré. Le type se lit donc une fois, dans le module, à sa
//! compilation.

use std::collections::BTreeMap;

use partie::{CausePanne, PanneOutil};
use wasmparser::{ExternalKind, Parser, Payload, TypeRef, ValType};

/// Le type d'une globale que [`moteur_commun::Bits`] sait porter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeGlobale {
    I32,
    I64,
    F32,
    F64,
}

fn interne(quoi: impl Into<String>) -> PanneOutil {
    PanneOutil::new(CausePanne::Interne, quoi)
}

fn type_de(v: ValType) -> Option<TypeGlobale> {
    match v {
        ValType::I32 => Some(TypeGlobale::I32),
        ValType::I64 => Some(TypeGlobale::I64),
        ValType::F32 => Some(TypeGlobale::F32),
        ValType::F64 => Some(TypeGlobale::F64),
        ValType::V128 | ValType::Ref(_) => None,
    }
}

/// Les globales exportées du module, par leur nom. Une globale d'un type que
/// [`moteur_commun::Bits`] ne porte pas (`v128`, une référence) n'y est pas : la
/// réécriture refuse ces types, et lire une telle globale par son nom est une panne.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "Payload est non exhaustif ; seules les globales et les exports disent un type, toute autre section est sans objet ici"
)]
pub(crate) fn globales_exportees(
    octets: &[u8],
) -> Result<BTreeMap<String, TypeGlobale>, PanneOutil> {
    let lire = |e: wasmparser::BinaryReaderError| interne(format!("module illisible : {e}"));
    // L'espace des globales : les importées d'abord, puis celles du module (§ 2.5.5 de
    // la spécification). Un module réécrit n'importe rien ; un module d'essai non plus.
    let mut types: Vec<Option<TypeGlobale>> = Vec::new();
    let mut exports: Vec<(String, u32)> = Vec::new();
    for charge in Parser::new(0).parse_all(octets) {
        match charge.map_err(lire)? {
            Payload::ImportSection(s) => {
                for i in s.into_imports() {
                    if let TypeRef::Global(g) = i.map_err(lire)?.ty {
                        types.push(type_de(g.content_type));
                    }
                }
            }
            Payload::GlobalSection(s) => {
                for g in s {
                    types.push(type_de(g.map_err(lire)?.ty.content_type));
                }
            }
            Payload::ExportSection(s) => {
                for e in s {
                    let e = e.map_err(lire)?;
                    if e.kind == ExternalKind::Global {
                        exports.push((e.name.to_string(), e.index));
                    }
                }
            }
            _ => {}
        }
    }
    let mut r = BTreeMap::new();
    for (nom, indice) in exports {
        let t = usize::try_from(indice)
            .ok()
            .and_then(|i| types.get(i).copied())
            .ok_or_else(|| interne(format!("export {nom} : globale {indice} absente")))?;
        if let Some(t) = t {
            r.insert(nom, t);
        }
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::{TypeGlobale, globales_exportees};

    #[test]
    fn chaque_globale_exportee_a_son_type() {
        let octets = wat::parse_str(
            r#"(module
                 (global (export "a") (mut i32) (i32.const 1))
                 (global (mut i64) (i64.const 2))
                 (global (export "c") (mut f32) (f32.const 3))
                 (global (export "d") f64 (f64.const 4))
                 (global (export "e") (mut i64) (i64.const 5))
                 (global (export "v") v128 (v128.const i64x2 0 0)))"#,
        )
        .unwrap();
        let g = globales_exportees(&octets).unwrap();
        let attendu: Vec<(&str, TypeGlobale)> = vec![
            ("a", TypeGlobale::I32),
            ("c", TypeGlobale::F32),
            ("d", TypeGlobale::F64),
            ("e", TypeGlobale::I64),
        ];
        assert_eq!(
            g.iter().map(|(n, t)| (n.as_str(), *t)).collect::<Vec<_>>(),
            attendu
        );
    }

    #[test]
    fn un_module_illisible_est_une_panne() {
        assert!(globales_exportees(b"\0asm\x01\0\0\0\x07\xff").is_err());
    }
}
