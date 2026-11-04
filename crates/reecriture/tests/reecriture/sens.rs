//! La réécriture garde le sens (§ 8.5, `reecrire_preserve_le_sens`) : un corpus de
//! modules `wat`, exécuté avant et après la chaîne des passes sur des entrées tirées
//! au hasard, avec un carburant très grand, donne les mêmes résultats, les mêmes
//! pièges, la même mémoire et les mêmes globales. Seule différence permise : la
//! charge utile d'un NaN, que la norme ne fixe pas.
//!
//! Aussi `reecrire_est_pure` et `reecrit_se_valide`.

use proptest::prelude::*;
use reecriture::{Reglages, nom_de_globale, passes, reecrire};

use crate::commun::{
    Execution, Issue, MOTEUR_A, MOTEUR_B, REGLAGES, Valeur, chaine, module_abi, wat,
};

/// Un module du corpus : son texte, ses fonctions exportées et la forme de leurs
/// paramètres.
struct Cas {
    texte: &'static str,
    fonctions: &'static [(&'static str, &'static [Forme])],
    globales: &'static [u32],
}

#[derive(Clone, Copy, Debug)]
enum Forme {
    I32,
    I64,
    F32,
    F64,
}

const CORPUS: &[Cas] = &[
    Cas {
        texte: r#"(module (memory (export "memory") 1 1)
          (func (export "arith") (param i32 i32) (result i32)
            (i32.add
              (i32.mul (i32.rotl (local.get 0) (local.get 1)) (i32.clz (local.get 1)))
              (i32.div_s (local.get 0) (local.get 1))))
          (func (export "arith64") (param i64 i64) (result i64)
            (i64.rem_u (i64.xor (local.get 0) (i64.shl (local.get 1) (i64.const 3)))
                       (i64.or (local.get 1) (i64.const 1)))))"#,
        fonctions: &[
            ("arith", &[Forme::I32, Forme::I32]),
            ("arith64", &[Forme::I64, Forme::I64]),
        ],
        globales: &[],
    },
    Cas {
        texte: r#"(module (memory (export "memory") 1 1)
          (func (export "somme") (param $n i32) (result i64) (local $s i64)
            (local.set $n (i32.and (local.get $n) (i32.const 1023)))
            (block $fin (loop $l
              (br_if $fin (i32.eqz (local.get $n)))
              (local.set $s (i64.add (local.get $s) (i64.extend_i32_u (local.get $n))))
              (local.set $n (i32.sub (local.get $n) (i32.const 1)))
              (br $l)))
            (local.get $s))
          (func $fib (export "fib") (param $n i32) (result i32)
            (local.set $n (i32.and (local.get $n) (i32.const 15)))
            (if (result i32) (i32.lt_u (local.get $n) (i32.const 2))
              (then (local.get $n))
              (else (i32.add (call $fib (i32.sub (local.get $n) (i32.const 1)))
                             (call $fib (i32.sub (local.get $n) (i32.const 2))))))))"#,
        fonctions: &[("somme", &[Forme::I32]), ("fib", &[Forme::I32])],
        globales: &[],
    },
    Cas {
        texte: r#"(module (memory (export "memory") 2 2) (data (i32.const 100) "le commun et les jeux")
          (func (export "memoire") (param $a i32) (param $b i32) (param $n i32) (result i32)
            (local.set $a (i32.and (local.get $a) (i32.const 4095)))
            (local.set $b (i32.and (local.get $b) (i32.const 4095)))
            (local.set $n (i32.and (local.get $n) (i32.const 511)))
            (memory.copy (local.get $a) (i32.const 100) (local.get $n))
            (memory.fill (local.get $b) (local.get $n) (i32.const 17))
            (i32.store16 offset=5 (local.get $b) (local.get $a))
            (i64.store (i32.const 8000) (i64.extend_i32_s (local.get $n)))
            (i32.add (i32.load8_u (local.get $a)) (i32.load (local.get $b))))
          (func (export "hors") (param $a i32) (result i32) (i32.load (local.get $a))))"#,
        fonctions: &[
            ("memoire", &[Forme::I32, Forme::I32, Forme::I32]),
            ("hors", &[Forme::I32]),
        ],
        globales: &[],
    },
    Cas {
        texte: r#"(module (memory (export "memory") 1 1)
          (func (export "f64") (param f64 f64) (result f64 f64 f64)
            (f64.add (f64.sqrt (f64.abs (local.get 0))) (f64.div (local.get 0) (local.get 1)))
            (f64.min (f64.nearest (local.get 0)) (f64.max (local.get 1) (f64.ceil (local.get 0))))
            (f64.copysign (f64.floor (local.get 1)) (local.get 0)))
          (func (export "f32") (param f32 f64) (result f32 f64 i32)
            (f32.mul (local.get 0) (f32.demote_f64 (local.get 1)))
            (f64.sub (f64.promote_f32 (f32.trunc (local.get 0))) (local.get 1))
            (i32.trunc_sat_f32_s (local.get 0))))"#,
        fonctions: &[
            ("f64", &[Forme::F64, Forme::F64]),
            ("f32", &[Forme::F32, Forme::F64]),
        ],
        globales: &[],
    },
    Cas {
        texte: r#"(module (memory (export "memory") 1 1)
          (type $t (func (param i32) (result i32)))
          (table 3 3 funcref) (elem (i32.const 0) $double $neg $carre)
          (func $double (param i32) (result i32) (i32.shl (local.get 0) (i32.const 1)))
          (func $neg (param i32) (result i32) (i32.sub (i32.const 0) (local.get 0)))
          (func $carre (param i32) (result i32) (i32.mul (local.get 0) (local.get 0)))
          (func (export "indirect") (param $i i32) (param $x i32) (result i32)
            (call_indirect (type $t) (local.get $x) (i32.rem_u (local.get $i) (i32.const 4))))
          (func (export "aiguillage") (param $i i32) (param $x i32) (result i32 i32)
            (block $c (result i32) (block $b (result i32) (block $a (result i32)
              (local.get $x) (local.get $i) (br_table $a $b $c))
              (return (i32.extend8_s) (i32.const 1)))
              (return (i32.extend16_s) (i32.const 2)))
            (select (local.get $x) (local.get $i) (i32.and (local.get $x) (i32.const 1)))))"#,
        fonctions: &[
            ("indirect", &[Forme::I32, Forme::I32]),
            ("aiguillage", &[Forme::I32, Forme::I32]),
        ],
        globales: &[],
    },
    Cas {
        texte: r#"(module (memory (export "memory") 1 32)
          (global $compte (mut i32) (i32.const 0))
          (global $cumul (mut i64) (i64.const 5))
          (func (export "grandir") (param i32) (result i32 i32)
            (global.set $compte (i32.add (global.get $compte) (i32.const 1)))
            (memory.grow (i32.and (local.get 0) (i32.const 7)))
            (memory.size))
          (func (export "cumuler") (param i64) (result i64)
            (global.set $cumul (i64.add (global.get $cumul) (local.get 0)))
            (global.get $cumul)))"#,
        fonctions: &[("grandir", &[Forme::I32]), ("cumuler", &[Forme::I64])],
        globales: &[0, 1],
    },
];

fn valeur(forme: Forme, graine: u64) -> Valeur {
    // Des valeurs remarquables une fois sur quatre : zéro, extrêmes, infinis, NaN
    // canonique. Un NaN d'une autre charge changerait de bits, ce que la norme permet.
    let remarquable = graine.is_multiple_of(4);
    let k = (graine >> 2) % 6;
    match forme {
        Forme::I32 => Valeur::I32(if remarquable {
            [0, 1, -1, i32::MIN, i32::MAX, 2][usize::try_from(k).unwrap()]
        } else {
            (graine >> 8) as i32
        }),
        Forme::I64 => Valeur::I64(if remarquable {
            [0, 1, -1, i64::MIN, i64::MAX, 3][usize::try_from(k).unwrap()]
        } else {
            (graine >> 3) as i64
        }),
        Forme::F32 => Valeur::F32(if remarquable {
            [
                0.0f32,
                -0.0,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::from_bits(0x7fc0_0000),
                1e-40,
            ][usize::try_from(k).unwrap()]
            .to_bits()
        } else {
            ((graine >> 8) as i32 as f32 / 1000.0).to_bits()
        }),
        Forme::F64 => Valeur::F64(if remarquable {
            [
                0.0f64,
                -0.0,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::from_bits(0x7ff8_0000_0000_0000),
                1e-310,
            ][usize::try_from(k).unwrap()]
            .to_bits()
        } else {
            ((graine >> 8) as i64 as f64 / 1000.0).to_bits()
        }),
    }
}

fn meme_sens(a: &Issue, b: &Issue) -> bool {
    match (a, b) {
        (Issue::Valeurs(x), Issue::Valeurs(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(u, v)| u.meme_sens(*v))
        }
        (Issue::Piege(x), Issue::Piege(y)) => x == y,
        _ => false,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn reecrire_preserve_le_sens(graines in proptest::collection::vec(any::<u64>(), 1..12)) {
        for cas in CORPUS {
            let original = wat(cas.texte);
            wasmparser::validate(&original).unwrap();
            let reecrit = chaine(&original, None);
            // `globales` n'ajoute que des exports : de quoi lire les globales d'origine.
            let mut avant = Execution::new(&passes::globales(&original).unwrap(), MOTEUR_A);
            let mut apres = Execution::new(&reecrit, MOTEUR_B);
            let mut g = graines.iter().copied().cycle();
            for _ in 0..graines.len() {
                for (nom, formes) in cas.fonctions {
                    let args: Vec<Valeur> = formes
                        .iter()
                        .map(|f| valeur(*f, g.next().unwrap().wrapping_mul(0x9e37_79b9_7f4a_7c15)))
                        .collect();
                    let x = avant.appeler(nom, &args);
                    let y = apres.appeler(nom, &args);
                    prop_assert!(meme_sens(&x, &y), "{nom}({args:?}) : {x:?} puis {y:?}");
                }
            }
            prop_assert!(avant.memoire() == apres.memoire());
            for i in cas.globales {
                let nom = nom_de_globale(*i);
                prop_assert_eq!(avant.globale(&nom), apres.globale(&nom));
            }
        }
    }
}

/// Les globales d'origine suivent les mêmes appels avant et après la réécriture.
#[test]
fn globales_suivent_les_memes_appels() {
    let cas = CORPUS.last().unwrap();
    let original = passes::globales(&wat(cas.texte)).unwrap();
    let reecrit = chaine(&wat(cas.texte), None);
    let mut avant = Execution::new(&original, MOTEUR_A);
    let mut apres = Execution::new(&reecrit, MOTEUR_A);
    for x in [3, 9, 1000, -4] {
        assert_eq!(
            avant.appeler("grandir", &[Valeur::I32(x)]),
            apres.appeler("grandir", &[Valeur::I32(x)])
        );
        assert_eq!(
            avant.appeler("cumuler", &[Valeur::I64(i64::from(x))]),
            apres.appeler("cumuler", &[Valeur::I64(i64::from(x))])
        );
    }
    for i in cas.globales {
        assert_eq!(
            avant.globale(&nom_de_globale(*i)),
            apres.globale(&nom_de_globale(*i))
        );
    }
    assert_eq!(avant.memoire(), apres.memoire());
}

#[test]
fn reecrire_est_pure() {
    for cas in CORPUS {
        let m = wat(cas.texte);
        assert_eq!(chaine(&m, None), chaine(&m, None));
    }
    let m = module_abi(
        "(func $f (param f64 f64) (result f64) (f64.div (f64.sqrt (local.get 0)) (local.get 1)))
         (func (result f64) (call $f (f64.const 1) (f64.const 2)))",
    );
    let a = reecrire(&m, &REGLAGES).unwrap();
    let b = reecrire(&m, &REGLAGES).unwrap();
    assert_eq!(a, b);
    // D'autres réglages donnent d'autres octets : ils sont dans l'empreinte.
    let c = reecrire(
        &m,
        &Reglages {
            profondeur_max: 7,
            ..REGLAGES
        },
    )
    .unwrap();
    assert_ne!(a.empreinte, c.empreinte);
    assert_eq!(a.empreinte_originale, c.empreinte_originale);
    assert_eq!(a.empreinte, *blake3::hash(&a.octets).as_bytes());
}

/// `reecrire` est exactement valider puis la chaîne des passes.
#[test]
fn reecrire_est_la_chaine_des_passes() {
    let m = module_abi("(func (param f32) (result f32) (f32.sqrt (local.get 0)))");
    assert_eq!(reecrire(&m, &REGLAGES).unwrap().octets, chaine(&m, None));
}

#[test]
fn reecrit_se_valide() {
    for cas in CORPUS {
        let r = chaine(&wat(cas.texte), None);
        wasmparser::validate(&r).unwrap();
        // Et sans aucune fonctionnalité de plus que l'original.
        let mut v = wasmparser::Validator::new_with_features(
            wasmparser::WasmFeatures::MUTABLE_GLOBAL
                | wasmparser::WasmFeatures::SATURATING_FLOAT_TO_INT
                | wasmparser::WasmFeatures::SIGN_EXTENSION
                | wasmparser::WasmFeatures::MULTI_VALUE
                | wasmparser::WasmFeatures::BULK_MEMORY
                | wasmparser::WasmFeatures::REFERENCE_TYPES
                | wasmparser::WasmFeatures::FLOATS
                | wasmparser::WasmFeatures::GC_TYPES,
        );
        v.validate_all(&r).unwrap();
    }
}

/// Le même module réécrit, sous deux moteurs différemment réglés, donne les mêmes
/// bits partout : c'est ce que la réécriture promet (D32).
#[test]
fn deux_moteurs_memes_bits_apres_reecriture() {
    for cas in CORPUS {
        let r = chaine(&wat(cas.texte), None);
        let mut a = Execution::new(&r, MOTEUR_A);
        let mut b = Execution::new(&r, MOTEUR_B);
        for graine in 0..200u64 {
            for (nom, formes) in cas.fonctions {
                let args: Vec<Valeur> = formes
                    .iter()
                    .zip(0u64..)
                    .map(|(f, i)| valeur(*f, graine.wrapping_mul(7919).wrapping_add(i)))
                    .collect();
                assert_eq!(
                    a.appeler(nom, &args),
                    b.appeler(nom, &args),
                    "{nom}({args:?})"
                );
            }
        }
        assert_eq!(a.memoire(), b.memoire());
    }
}

/// Le carburant consommé par `wasmtime` lui-même (une unité par opérateur exécuté, sauf
/// quelques-uns gratuits), le juge mûr de notre compte (CONTRAT, « Le code tiers »).
fn carburant_de_wasmtime(octets: &[u8], nom: &str, args: &[Valeur]) -> u64 {
    let mut c = wasmtime::Config::new();
    c.consume_fuel(true);
    let engine = wasmtime::Engine::new(&c).unwrap();
    let module = wasmtime::Module::new(&engine, octets).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    store.set_fuel(u64::MAX / 4).unwrap();
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();
    let avant = store.get_fuel().unwrap();
    let f = instance.get_func(&mut store, nom).unwrap();
    let n = f.ty(&store).results().len();
    let mut sorties = vec![wasmtime::Val::I32(0); n];
    let args: Vec<wasmtime::Val> = args.iter().map(|a| a.vers()).collect();
    let _: Result<(), wasmtime::Error> = f.call(&mut store, &args, &mut sorties);
    avant.saturating_sub(store.get_fuel().unwrap())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Besoin 13 des jeux : le carburant compté par segment reste un majorant du travail
    /// fait. Sur le corpus, il n'est jamais sous celui que `wasmtime` compte pour le
    /// module d'origine, appel par appel, piège compris. Les modules qui copient,
    /// remplissent ou agrandissent la mémoire n'y sont pas : `wasmtime` compte un par
    /// octet copié et charge la croissance en plus de l'instruction, § 3.6 un par 64
    /// octets (`carburant_compte_la_longueur_des_copies`) et l'instruction seule.
    #[test]
    fn carburant_majore_le_carburant_de_wasmtime(graines in proptest::collection::vec(any::<u64>(), 1..6)) {
        for cas in CORPUS
            .iter()
            .filter(|c| !["memory.copy", "memory.fill", "memory.grow"].iter().any(|i| c.texte.contains(i)))
        {
            let original = passes::globales(&wat(cas.texte)).unwrap();
            let reecrit = chaine(&wat(cas.texte), None);
            let mut g = graines.iter().copied().cycle();
            for (nom, formes) in cas.fonctions {
                let args: Vec<Valeur> = formes
                    .iter()
                    .map(|f| valeur(*f, g.next().unwrap().wrapping_mul(0x9e37_79b9_7f4a_7c15)))
                    .collect();
                let juge = carburant_de_wasmtime(&original, nom, &args);
                let mut e = Execution::new(&reecrit, MOTEUR_A);
                e.poser(reecriture::CARBURANT, wasmtime::Val::I64(1 << 40));
                let _: Issue = e.appeler(nom, &args);
                let Valeur::I64(reste) = e.globale(reecriture::CARBURANT) else {
                    panic!("carburant d'un autre type")
                };
                let compte = u64::try_from((1i64 << 40).saturating_sub(reste)).unwrap();
                prop_assert!(compte >= juge, "{nom}({args:?}) : compté {compte}, wasmtime {juge}");
            }
        }
    }
}

/// Besoin 13 des jeux : le chemin court d'une grande fonction ne paie pas le corps
/// entier, seulement les segments qu'il traverse.
#[test]
fn carburant_ne_paie_que_le_chemin_pris() {
    let long = "(local.set $x (i32.add (local.get $x) (i32.const 1)))\n".repeat(200);
    let m = chaine(
        &wat(&format!(
            r#"(module (memory 1 1)
              (func (export "f") (param $c i32) (result i32) (local $x i32)
                (if (local.get $c)
                  (then {long}))
                (local.get $x)))"#
        )),
        None,
    );
    let consomme = |c: i32| {
        let mut e = Execution::new(&m, MOTEUR_A);
        e.poser(reecriture::CARBURANT, wasmtime::Val::I64(1_000_000));
        let _: Issue = e.appeler("f", &[Valeur::I32(c)]);
        let Valeur::I64(reste) = e.globale(reecriture::CARBURANT) else {
            panic!()
        };
        1_000_000 - reste
    };
    let court = consomme(0);
    let entier = consomme(1);
    assert!(court < 20, "le chemin court coûte {court}");
    assert!(entier > 800, "le chemin long coûte {entier}");
}
