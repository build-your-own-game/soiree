//! La passe « valider » (§ 3.1, § 3.3, § 3.6) : ce que le module peut faire se
//! constate sur ses octets.
//!
//! Trois temps. D'abord `wasmparser` valide le module avec toutes les propositions
//! qu'il connaît : un module invalide est [`Refus::Illisible`]. Puis chaque refusé de
//! § 3.1 est cherché et nommé, avec sa position. Enfin `wasmparser` revalide avec les
//! seules fonctionnalités admises : ce qu'aucun refus nommé n'a vu est refusé quand
//! même, rien ne passe par oubli.

use std::collections::BTreeMap;

use wasmparser::{
    ExternalKind, FuncValidatorAllocations, Parser, RefType, ValType, ValidPayload, Validator,
    WasmFeatures,
};

use crate::analyse::{Analyse, Signature, analyser};
use crate::operateurs::{admise, famille, nom};
use crate::{Constat, Refus, RefusMemoire};

/// Les fonctionnalités de § 3.1 : WebAssembly 1.0, globales mutables, extension de
/// signe, conversions saturantes, valeurs multiples, mémoire en bloc, types référence
/// (pour `call_indirect`). `FLOATS` et `GC_TYPES` sont des pseudo-fonctionnalités de
/// `wasmparser`, nécessaires aux flottants et à `funcref`.
pub(crate) const ADMISES: WasmFeatures = WasmFeatures::MUTABLE_GLOBAL
    .union(WasmFeatures::SATURATING_FLOAT_TO_INT)
    .union(WasmFeatures::SIGN_EXTENSION)
    .union(WasmFeatures::MULTI_VALUE)
    .union(WasmFeatures::BULK_MEMORY)
    .union(WasmFeatures::REFERENCE_TYPES)
    .union(WasmFeatures::FLOATS)
    .union(WasmFeatures::GC_TYPES);

/// Les exports exigés (§ 3.3).
pub(crate) const MEMOIRE: &str = "memory";
pub(crate) const BOITE: &str = "soiree_boite";
pub(crate) const APPEL: &str = "soiree_appel";

/// Ce que la validation sait d'un module accepté.
pub(crate) struct Examen {
    pub(crate) constat: Constat,
    pub(crate) pile_initiale: u32,
    pub(crate) pile_globale: Option<u32>,
}

fn illisible(e: &wasmparser::BinaryReaderError) -> Refus {
    Refus::Illisible {
        position: e.offset(),
        quoi: e.message().to_string(),
    }
}

/// La passe complète.
pub(crate) fn examiner(m: &[u8]) -> Result<Examen, Refus> {
    Validator::new_with_features(WasmFeatures::all())
        .validate_all(m)
        .map_err(|e| illisible(&e))?;
    let a = analyser(m)?;
    if let Some((module, nom)) = a.imports.first() {
        return Err(Refus::Import {
            module: module.clone(),
            nom: nom.clone(),
        });
    }
    if a.depart {
        return Err(Refus::Depart);
    }
    let memoire = memoire(&a)?;
    if let Some(position) = a.etiquettes {
        return Err(Refus::Fonctionnalite {
            nom: "exceptions",
            position,
        });
    }
    types(&a)?;
    globales(&a)?;
    tables(&a)?;
    let familles = code(&a)?;
    for c in cadres(m)? {
        if c.valeurs > u64::from(crate::LOCAUX_MAX) {
            return Err(Refus::Fonctionnalite {
                nom: "plus de LOCAUX_MAX paramètres et locaux dans une fonction",
                position: c.position,
            });
        }
        if c.hauteur > crate::HAUTEUR_MAX {
            return Err(Refus::Fonctionnalite {
                nom: "plus de HAUTEUR_MAX valeurs sur la pile d'opérandes d'une fonction",
                position: c.position,
            });
        }
    }
    let exports = exports(&a)?;
    let (pile_initiale, pile_globale) = pile(&a, memoire.0)?;
    revalider(m)?;
    let globales_mutables = a.globales.iter().filter(|g| g.ty.mutable).count();
    Ok(Examen {
        constat: Constat {
            imports: Vec::new(),
            exports,
            memoire,
            familles,
            globales_mutables: u32::try_from(globales_mutables).unwrap_or(u32::MAX),
        },
        pile_initiale,
        pile_globale,
    })
}

/// Ce qu'un cadre de fonction peut garder vivant, lu par `wasmparser`.
pub(crate) struct Cadre {
    /// L'indice de la fonction.
    pub(crate) indice: u32,
    /// Ses paramètres et ses locaux.
    pub(crate) valeurs: u64,
    /// La plus grande hauteur de sa pile d'opérandes.
    pub(crate) hauteur: u32,
    /// La position de son corps.
    pub(crate) position: usize,
}

/// Le cadre de chaque fonction définie, dans l'ordre.
pub(crate) fn cadres(m: &[u8]) -> Result<Vec<Cadre>, Refus> {
    let mut validateur = Validator::new_with_features(WasmFeatures::all());
    let mut allocations = FuncValidatorAllocations::default();
    let mut cadres = Vec::new();
    for payload in Parser::new(0).parse_all(m) {
        let payload = payload.map_err(|e| illisible(&e))?;
        if let ValidPayload::Func(f, corps) =
            validateur.payload(&payload).map_err(|e| illisible(&e))?
        {
            let mut fv = f.into_validator(allocations);
            let mut locaux = corps.get_locals_reader().map_err(|e| illisible(&e))?;
            for _ in 0..locaux.get_count() {
                let position = locaux.original_position();
                let (n, ty) = locaux.read().map_err(|e| illisible(&e))?;
                fv.define_locals(position, n, ty)
                    .map_err(|e| illisible(&e))?;
            }
            let mut lecteur = corps.get_operators_reader().map_err(|e| illisible(&e))?;
            let mut hauteur = 0;
            while !lecteur.eof() {
                let (op, position) = lecteur.read_with_offset().map_err(|e| illisible(&e))?;
                fv.op(position, &op).map_err(|e| illisible(&e))?;
                hauteur = fv.operand_stack_height().max(hauteur);
            }
            cadres.push(Cadre {
                indice: fv.index(),
                valeurs: u64::from(fv.len_locals()),
                hauteur,
                position: corps.range().start,
            });
            allocations = fv.into_allocations();
        }
    }
    Ok(cadres)
}

/// Le dernier temps : `wasmparser` avec les seules fonctionnalités admises. Sert aussi
/// au module réécrit, qui ne doit rien employer de plus que l'original.
pub(crate) fn revalider(m: &[u8]) -> Result<(), Refus> {
    Validator::new_with_features(ADMISES)
        .validate_all(m)
        .map(|_| ())
        .map_err(|e| Refus::Fonctionnalite {
            nom: "hors de la liste de § 3.1",
            position: e.offset(),
        })
}

fn memoire(a: &Analyse<'_>) -> Result<(u32, Option<u32>), Refus> {
    let [(_, mem)] = a.memoires.as_slice() else {
        return Err(Refus::Memoire(if a.memoires.is_empty() {
            RefusMemoire::Absente
        } else {
            RefusMemoire::Plusieurs
        }));
    };
    if mem.shared {
        return Err(Refus::Memoire(RefusMemoire::Partagee));
    }
    if mem.memory64 {
        return Err(Refus::Memoire(RefusMemoire::Bits64));
    }
    if mem.page_size_log2.is_some() {
        return Err(Refus::Memoire(RefusMemoire::PagesChoisies));
    }
    let Some(max) = mem.maximum else {
        return Err(Refus::Memoire(RefusMemoire::SansMaximum));
    };
    // Une mémoire 32 bits a au plus 65 536 pages : la validation l'a vérifié.
    let min = u32::try_from(mem.initial).unwrap_or(u32::MAX);
    let max = u32::try_from(max).unwrap_or(u32::MAX);
    Ok((min, Some(max)))
}

/// Un type de valeur admis : un nombre. `v128` est le SIMD ; une référence ne sert
/// qu'à `call_indirect`, jamais comme valeur.
fn valeur(t: ValType, position: usize) -> Result<(), Refus> {
    match t {
        ValType::I32 | ValType::I64 | ValType::F32 | ValType::F64 => Ok(()),
        ValType::V128 => Err(Refus::Fonctionnalite {
            nom: "simd",
            position,
        }),
        ValType::Ref(_) => Err(Refus::Fonctionnalite {
            nom: "types référence hors de call_indirect",
            position,
        }),
    }
}

fn signature(s: &Signature, position: usize) -> Result<(), Refus> {
    for t in s.parametres.iter().chain(&s.resultats) {
        valeur(*t, position)?;
    }
    Ok(())
}

fn types(a: &Analyse<'_>) -> Result<(), Refus> {
    for (position, sig) in &a.types {
        let Some(sig) = sig else {
            return Err(Refus::Fonctionnalite {
                nom: "gc",
                position: *position,
            });
        };
        signature(sig, *position)?;
    }
    Ok(())
}

fn globales(a: &Analyse<'_>) -> Result<(), Refus> {
    for g in &a.globales {
        if g.ty.shared {
            return Err(Refus::Fonctionnalite {
                nom: "fils",
                position: g.position,
            });
        }
        valeur(g.ty.content_type, g.position)?;
    }
    Ok(())
}

fn tables(a: &Analyse<'_>) -> Result<(), Refus> {
    for (position, t) in &a.tables {
        if t.table64 {
            return Err(Refus::Fonctionnalite {
                nom: "table 64 bits",
                position: *position,
            });
        }
        if t.shared {
            return Err(Refus::Fonctionnalite {
                nom: "fils",
                position: *position,
            });
        }
        if t.element_type != RefType::FUNCREF {
            return Err(Refus::Fonctionnalite {
                nom: "types référence hors de call_indirect",
                position: *position,
            });
        }
    }
    Ok(())
}

/// Chaque instruction de chaque fonction : admise, ou refusée par son nom. Rend le
/// compte des familles.
fn code(a: &Analyse<'_>) -> Result<Vec<(&'static str, u32)>, Refus> {
    let mut familles: BTreeMap<&'static str, u32> = BTreeMap::new();
    for corps in &a.corps {
        for local in corps.get_locals_reader().map_err(|e| illisible(&e))? {
            let (_, t) = local.map_err(|e| illisible(&e))?;
            valeur(t, corps.range().start)?;
        }
        let mut lecteur = corps.get_operators_reader().map_err(|e| illisible(&e))?;
        while !lecteur.eof() {
            let (op, position) = lecteur.read_with_offset().map_err(|e| illisible(&e))?;
            if !admise(&op) {
                return Err(Refus::Instruction {
                    nom: nom(&op),
                    position,
                });
            }
            let compte = familles.entry(famille(&op)).or_insert(0);
            *compte = compte.saturating_add(1);
        }
    }
    Ok(familles.into_iter().collect())
}

/// Exactement les trois exports de § 3.3, chacun de son type.
fn exports(a: &Analyse<'_>) -> Result<Vec<String>, Refus> {
    let boite = Signature {
        parametres: vec![ValType::I32, ValType::I32],
        resultats: vec![ValType::I32],
    };
    let appel = Signature {
        parametres: vec![ValType::I32],
        resultats: vec![ValType::I64],
    };
    let mut vus = [false; 3];
    for e in &a.exports {
        let bon = match e.nom.as_str() {
            MEMOIRE => {
                vus[0] = true;
                e.sorte == ExternalKind::Memory && e.indice == 0
            }
            BOITE => {
                vus[1] = true;
                e.sorte == ExternalKind::Func && fonction(a, e.indice) == Some(&boite)
            }
            APPEL => {
                vus[2] = true;
                e.sorte == ExternalKind::Func && fonction(a, e.indice) == Some(&appel)
            }
            _ => false,
        };
        if !bon {
            return Err(Refus::Export { nom: e.nom.clone() });
        }
    }
    for (vu, nom) in vus.iter().zip([MEMOIRE, BOITE, APPEL]) {
        if !vu {
            return Err(Refus::ExportManquant { nom });
        }
    }
    Ok(a.exports.iter().map(|e| e.nom.clone()).collect())
}

fn fonction<'a>(a: &'a Analyse<'_>, indice: u32) -> Option<&'a Signature> {
    a.signature(usize::try_from(indice).ok()?)
}

/// La pile en bas de la mémoire (§ 3.1) : `__stack_pointer` est la globale que la
/// section des noms nomme ainsi, sinon la première globale mutable `i32` (la
/// convention de LLVM). Sa valeur initiale est une constante, au plus la mémoire
/// minimale, et aucun segment de données ne commence en dessous. Un module sans
/// globale mutable `i32` n'a pas de pile.
fn pile(a: &Analyse<'_>, pages_min: u32) -> Result<(u32, Option<u32>), Refus> {
    let est_pile = |g: &crate::analyse::Globale| g.ty.mutable && g.ty.content_type == ValType::I32;
    let indice = match a.globale_pile {
        Some(i) => Some(i),
        None => a
            .globales
            .iter()
            .position(est_pile)
            .and_then(|i| u32::try_from(i).ok()),
    };
    let Some(indice) = indice else {
        return Ok((0, None));
    };
    let globale = usize::try_from(indice)
        .ok()
        .and_then(|i| a.globales.get(i))
        .filter(|g| est_pile(g))
        .ok_or(Refus::PileNonEnBas)?;
    let initiale = globale
        .initiale
        .and_then(|v| i32::try_from(v).ok())
        .map(i32::cast_unsigned)
        .ok_or(Refus::PileNonEnBas)?;
    let octets_min = u64::from(pages_min).saturating_mul(65_536);
    if u64::from(initiale) > octets_min {
        return Err(Refus::PileNonEnBas);
    }
    for (_, adresse) in &a.donnees_actives {
        let adresse = adresse
            .and_then(|v| i32::try_from(v).ok())
            .map(i32::cast_unsigned)
            .ok_or(Refus::PileNonEnBas)?;
        if adresse < initiale {
            return Err(Refus::PileNonEnBas);
        }
    }
    Ok((initiale, Some(indice)))
}
