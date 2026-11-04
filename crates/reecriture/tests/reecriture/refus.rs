//! Un refus par ligne de § 3.1 et § 3.3, sur un petit module écrit à la main : le
//! refus nomme l'import, l'export, la fonctionnalité ou l'instruction (§ 8.5).

use proptest::prelude::*;
use reecriture::{Refus, RefusMemoire, passes, reecrire};

use crate::commun::{REGLAGES, module_abi, wat};

fn refus(m: &[u8]) -> Refus {
    let r = passes::valider(m).expect_err("le module devait être refusé");
    // `reecrire` refuse de même, avant toute passe.
    assert_eq!(reecrire(m, &REGLAGES).unwrap_err(), r);
    r
}

fn instruction(r: &Refus) -> &'static str {
    match r {
        Refus::Instruction { nom, .. } => nom,
        autre => panic!("refus inattendu : {autre:?}"),
    }
}

/// Le module témoin passe : sans lui, chaque refus ci-dessous pourrait venir d'autre
/// chose que ce qu'il vise.
#[test]
fn le_module_temoin_est_accepte() {
    let m = module_abi("");
    let c = passes::valider(&m).unwrap();
    assert_eq!(c.imports, Vec::<(String, String)>::new());
    assert_eq!(c.exports, ["memory", "soiree_boite", "soiree_appel"]);
    assert_eq!(c.memoire, (4, Some(16)));
    assert_eq!(c.globales_mutables, 1);
    assert_eq!(c.familles, [("base", 4)]);
    let r = reecrire(&m, &REGLAGES).unwrap();
    assert_eq!(r.pile_initiale, 65_536);
    assert_eq!(r.pile_globale, Some(0));
}

#[test]
fn illisible() {
    assert!(matches!(
        refus(b"\0asm\x01\0\0\0\x05"),
        Refus::Illisible { .. }
    ));
    assert!(matches!(refus(b"pas un module"), Refus::Illisible { .. }));
}

#[test]
fn import_refuse_et_nomme() {
    let m = wat(r#"(module
      (import "env" "horloge" (func (result i64)))
      (memory (export "memory") 1 1))"#);
    assert_eq!(
        refus(&m),
        Refus::Import {
            module: String::from("env"),
            nom: String::from("horloge"),
        }
    );
}

/// Un import caché derrière un module qui a tout le reste : une globale importée,
/// jamais lue, refusée quand même.
#[test]
fn import_cache_refuse() {
    let m = wat(r#"(module
      (import "wasi_snapshot_preview1" "environ" (global i32))
      (memory (export "memory") 4 16)
      (global $__stack_pointer (mut i32) (i32.const 65536))
      (func (export "soiree_boite") (param i32 i32) (result i32) (i32.const 65536))
      (func (export "soiree_appel") (param i32) (result i64) (i64.const 0)))"#);
    assert_eq!(
        refus(&m),
        Refus::Import {
            module: String::from("wasi_snapshot_preview1"),
            nom: String::from("environ"),
        }
    );
}

#[test]
fn depart_refuse() {
    assert_eq!(refus(&module_abi("(func $d) (start $d)")), Refus::Depart);
}

#[test]
fn memoire_absente_plusieurs_partagee_64_sans_maximum() {
    let cas = [
        ("(module)", RefusMemoire::Absente),
        (
            "(module (memory 1 1) (memory 1 1))",
            RefusMemoire::Plusieurs,
        ),
        ("(module (memory 1 1 shared))", RefusMemoire::Partagee),
        ("(module (memory i64 1 1))", RefusMemoire::Bits64),
        ("(module (memory 1))", RefusMemoire::SansMaximum),
        (
            "(module (memory 1 1 (pagesize 1)))",
            RefusMemoire::PagesChoisies,
        ),
    ];
    for (texte, attendu) in cas {
        assert_eq!(refus(&wat(texte)), Refus::Memoire(attendu), "{texte}");
    }
}

#[test]
fn export_en_trop_refuse() {
    let r = refus(&module_abi(r#"(func (export "tricher"))"#));
    assert_eq!(
        r,
        Refus::Export {
            nom: String::from("tricher")
        }
    );
}

#[test]
fn export_manquant_refuse() {
    let m = wat(r#"(module
      (memory (export "memory") 1 1)
      (func (export "soiree_boite") (param i32 i32) (result i32) (i32.const 0)))"#);
    assert_eq!(
        refus(&m),
        Refus::ExportManquant {
            nom: "soiree_appel"
        }
    );
}

#[test]
fn export_du_mauvais_type_refuse() {
    let m = wat(r#"(module
      (memory (export "memory") 1 1)
      (func (export "soiree_boite") (param i32 i32) (result i32) (i32.const 0))
      (func (export "soiree_appel") (param i32) (result i32) (i32.const 0)))"#);
    assert_eq!(
        refus(&m),
        Refus::Export {
            nom: String::from("soiree_appel")
        }
    );
}

#[test]
fn pile_non_en_bas_refusee() {
    // Une donnée sous la valeur initiale de la pile : la pile la piétinerait.
    let r = refus(&module_abi(r#"(data (i32.const 1024) "dessous")"#));
    assert_eq!(r, Refus::PileNonEnBas);
    // La pile au-delà de la mémoire minimale.
    let m = wat(r#"(module
      (memory (export "memory") 1 16)
      (global $__stack_pointer (mut i32) (i32.const 131072))
      (func (export "soiree_boite") (param i32 i32) (result i32) (i32.const 0))
      (func (export "soiree_appel") (param i32) (result i64) (i64.const 0)))"#);
    assert_eq!(refus(&m), Refus::PileNonEnBas);
    // Une donnée au-dessus passe.
    passes::valider(&module_abi(r#"(data (i32.const 70000) "dessus")"#)).unwrap();
}

/// SIMD refusé dans cette version (D35), relâché ou non : l'instruction est nommée.
#[test]
fn simd_refuse() {
    let r = refus(&module_abi(
        "(func (drop (i32x4.extract_lane 0 (i32x4.splat (i32.const 1)))))",
    ));
    assert_eq!(instruction(&r), "I32x4Splat");
    let r = refus(&module_abi(
        "(func (param f32) (drop (i32x4.extract_lane 0 (i32x4.relaxed_trunc_f32x4_s (f32x4.splat (local.get 0))))))",
    ));
    assert_eq!(instruction(&r), "F32x4Splat");
    let r = refus(&module_abi("(func (param v128))"));
    assert!(
        matches!(r, Refus::Fonctionnalite { nom: "simd", .. }),
        "{r:?}"
    );
}

#[test]
fn atomiques_refusees() {
    let r = refus(&module_abi("(func (drop (i32.atomic.load (i32.const 0))))"));
    assert_eq!(instruction(&r), "I32AtomicLoad");
    let r = refus(&module_abi("(func (atomic.fence))"));
    assert_eq!(instruction(&r), "AtomicFence");
}

#[test]
fn exceptions_refusees() {
    let r = refus(&module_abi("(tag $e) (func (throw $e))"));
    assert!(
        matches!(
            r,
            Refus::Fonctionnalite {
                nom: "exceptions",
                ..
            }
        ),
        "{r:?}"
    );
}

#[test]
fn appels_terminaux_refuses() {
    let r = refus(&module_abi("(func $f (return_call $f))"));
    assert_eq!(instruction(&r), "ReturnCall");
}

#[test]
fn gc_refuse() {
    let r = refus(&module_abi("(type (struct (field i32)))"));
    assert!(
        matches!(r, Refus::Fonctionnalite { nom: "gc", .. }),
        "{r:?}"
    );
}

#[test]
fn instructions_de_table_refusees() {
    let table = "(table $t 2 2 funcref) (elem $s func $g) (func $g)";
    let cas = [
        (
            "(func (table.set $t (i32.const 0) (ref.null func)))",
            "RefNull",
        ),
        (
            "(func (drop (table.grow $t (ref.null func) (i32.const 1))))",
            "RefNull",
        ),
        (
            "(func (table.fill $t (i32.const 0) (ref.func $g) (i32.const 1)))",
            "RefFunc",
        ),
        (
            "(func (table.copy $t $t (i32.const 0) (i32.const 0) (i32.const 1)))",
            "TableCopy",
        ),
        (
            "(func (table.init $t $s (i32.const 0) (i32.const 0) (i32.const 1)))",
            "TableInit",
        ),
        ("(func (elem.drop $s))", "ElemDrop"),
        ("(func (drop (table.size $t)))", "TableSize"),
        ("(func (drop (table.get $t (i32.const 0))))", "TableGet"),
    ];
    for (code, nom) in cas {
        let r = refus(&module_abi(&format!("{table} {code}")));
        assert_eq!(instruction(&r), nom, "{code}");
    }
}

/// Les types référence ne servent qu'à `call_indirect` : ni valeur, ni globale, ni
/// table d'`externref`.
#[test]
fn references_hors_de_call_indirect_refusees() {
    let cas = [
        "(func (param funcref))",
        "(global (mut externref) (ref.null extern))",
        "(table 1 1 externref)",
        "(func (local externref))",
    ];
    for code in cas {
        let r = refus(&module_abi(code));
        assert!(
            matches!(
                r,
                Refus::Fonctionnalite {
                    nom: "types référence hors de call_indirect",
                    ..
                }
            ),
            "{code} : {r:?}"
        );
    }
    // `call_indirect` sur une table de `funcref` passe.
    passes::valider(&module_abi(
        "(type $v (func)) (table 1 1 funcref) (elem (i32.const 0) $g) (func $g) (func (call_indirect (type $v) (i32.const 0)))",
    ))
    .unwrap();
}

/// Ce qu'aucun refus nommé ne vise est refusé par la revalidation : ici, une
/// expression constante étendue.
#[test]
fn hors_de_la_liste_refuse_quand_meme() {
    let r = refus(&module_abi(
        "(global i32 (i32.add (i32.const 1) (i32.const 2)))",
    ));
    assert!(
        matches!(
            r,
            Refus::Fonctionnalite {
                nom: "hors de la liste de § 3.1",
                ..
            }
        ),
        "{r:?}"
    );
}

/// Les fonctionnalités admises passent : extension de signe, conversions
/// saturantes, valeurs multiples, mémoire en bloc.
#[test]
fn fonctionnalites_admises_acceptees() {
    let m = module_abi(
        r#"(data $d "abc")
        (func (param i32 f32) (result i32 i32)
          (drop (f32.add (local.get 1) (local.get 1)))
          (drop (i32.extend8_s (local.get 0)))
          (drop (i32.trunc_sat_f32_s (local.get 1)))
          (memory.copy (i32.const 70000) (i32.const 70010) (i32.const 4))
          (memory.fill (i32.const 70000) (i32.const 0) (i32.const 4))
          (memory.init $d (i32.const 70000) (i32.const 0) (i32.const 3))
          (data.drop $d)
          (local.get 0) (local.get 0))"#,
    );
    let c = passes::valider(&m).unwrap();
    let familles: Vec<&str> = c.familles.iter().map(|(f, _)| *f).collect();
    assert_eq!(
        familles,
        [
            "base",
            "conversions_saturantes",
            "extension_de_signe",
            "flottants",
            "memoire_en_bloc"
        ]
    );
}

/// Une fonction à 50 000 locaux (le plafond de `wasmparser`) qui fait un
/// `memory.fill` : la passe carburant lui ajoute un local. Le module est refusé à
/// la validation, par une règle nommée et à sa position dans l'original, et non par
/// une « fonctionnalité hors de § 3.1 » que la réécriture aurait introduite.
fn locaux_au_plafond(locaux: u32) -> Vec<u8> {
    module_abi(&format!(
        "(table 1 1 funcref) (elem (i32.const 0) $f)
        (func $f (param i32) (local {})
          (memory.fill (i32.const 70000) (i32.const 0) (local.get 0)))",
        "i64 ".repeat(locaux as usize)
    ))
}

#[test]
fn locaux_au_plafond_refuses_par_une_regle_nommee() {
    let m = locaux_au_plafond(49_999);
    let r = refus(&m);
    let Refus::Fonctionnalite { nom, position } = r else {
        panic!("refus inattendu : {r:?}");
    };
    assert!(nom.contains("locaux"), "{nom}");
    assert!(position < m.len());
    // Sous la limite, la même fonction passe la réécriture entière.
    let m = locaux_au_plafond(reecriture::LOCAUX_MAX - 1);
    reecrire(&m, &REGLAGES).unwrap();
    let m = locaux_au_plafond(reecriture::LOCAUX_MAX);
    assert!(matches!(refus(&m), Refus::Fonctionnalite { .. }));
}

/// Une fonction qui empile `n` valeurs, les garde à travers un appel, puis les
/// additionne.
fn pile_d_operandes(n: u32) -> Vec<u8> {
    module_abi(&format!(
        "(table 1 1 funcref) (elem (i32.const 0) $f)
        (func $f (param i32) (result i64)
          {}
          (if (result i64) (local.get 0)
            (then (call $f (i32.sub (local.get 0) (i32.const 1))))
            (else (i64.const 0)))
          {})",
        "(i64.load (i32.const 70000)) ".repeat(n as usize),
        "i64.add ".repeat(n as usize)
    ))
}

/// Des milliers de valeurs sur la pile d'opérandes, vivantes à travers un appel,
/// font croître la compilation plus que linéairement : `charger` se figeait des
/// secondes sur une seule fonction. Au-delà de `HAUTEUR_MAX`, la validation refuse.
#[test]
fn pile_d_operandes_trop_haute_refusee() {
    // `if` et l'argument de l'appel empilent deux valeurs de plus.
    reecrire(&pile_d_operandes(reecriture::HAUTEUR_MAX - 2), &REGLAGES).unwrap();
    let m = pile_d_operandes(reecriture::HAUTEUR_MAX + 1);
    let Refus::Fonctionnalite { nom, position } = refus(&m) else {
        panic!("refus inattendu");
    };
    assert!(nom.contains("pile d'opérandes"), "{nom}");
    assert!(position < m.len());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    /// Des octets au hasard, ou un module abîmé : un refus, jamais une panique.
    #[test]
    fn octets_au_hasard_jamais_de_panique(octets in proptest::collection::vec(any::<u8>(), 0..512)) {
        let _: Result<_, Refus> = reecrire(&octets, &REGLAGES);
    }

    #[test]
    fn module_abime_jamais_de_panique(i in any::<prop::sample::Index>(), o in any::<u8>()) {
        let mut m = module_abi("(func (param f32) (result f32) (f32.sqrt (local.get 0)))");
        let k = i.index(m.len());
        m[k] = o;
        if let Ok(r) = reecrire(&m, &REGLAGES) {
            wasmparser::validate(&r.octets).unwrap();
        }
    }
}
