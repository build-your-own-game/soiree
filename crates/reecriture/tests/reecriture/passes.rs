//! Chaque passe rougit quand on la retire (§ 8.5) : sans elle, le module hostile
//! qu'elle vise ne rend pas la main, diverge entre deux moteurs, ou dépasse son
//! plafond ; avec elle, il s'arrête au même endroit à chaque exécution.

use reecriture::passes::{NAN_F32, NAN_F64};
use reecriture::{
    CARBURANT, CROISSANCE_BORNEE, CROISSANCE_REFUSEE, MEMOIRE_MAX, PROFONDEUR, nom_de_globale,
    passes,
};
use wasmtime::{Trap, Val};

use crate::commun::{Execution, Issue, MOTEUR_A, MOTEUR_B, Passe, REGLAGES, Valeur, chaine, wat};

fn boucle_infinie() -> Vec<u8> {
    wat(r#"(module (memory 1 1)
      (func (export "f") (loop $l (br $l))))"#)
}

#[test]
fn sans_carburant_une_boucle_infinie_ne_rend_pas_la_main() {
    // Sans la passe, seul le garde-fou mural de l'exécuteur l'arrête (SQ-59).
    let mut e = Execution::new(&chaine(&boucle_infinie(), Some(Passe::Carburant)), MOTEUR_A);
    assert_eq!(e.appeler("f", &[]), Issue::Piege(Trap::Interrupt));
}

#[test]
fn avec_carburant_une_boucle_infinie_epuise_son_budget_au_meme_endroit() {
    let m = chaine(&boucle_infinie(), None);
    let mut restes = Vec::new();
    for r in [MOTEUR_A, MOTEUR_B, MOTEUR_A] {
        let mut e = Execution::new(&m, r);
        e.poser(CARBURANT, Val::I64(10_000));
        assert_eq!(
            e.appeler("f", &[]),
            Issue::Piege(Trap::UnreachableCodeReached)
        );
        let Valeur::I64(reste) = e.globale(CARBURANT) else {
            panic!()
        };
        assert!(reste < 0);
        restes.push(reste);
    }
    assert!(restes.windows(2).all(|w| w[0] == w[1]), "{restes:?}");
}

/// Le carburant consommé par un appel est le même à chaque exécution, et suit le
/// nombre de tours : `a + b × n`.
#[test]
fn carburant_compte_pareil() {
    let m = chaine(
        &wat(r#"(module (memory 1 1)
          (func (export "f") (param $n i32)
            (block $fin (loop $l
              (br_if $fin (i32.eqz (local.get $n)))
              (local.set $n (i32.sub (local.get $n) (i32.const 1)))
              (br $l)))))"#),
        None,
    );
    let consomme = |n: i32| {
        let mut e = Execution::new(&m, MOTEUR_A);
        e.poser(CARBURANT, Val::I64(1_000_000));
        assert_eq!(e.appeler("f", &[Valeur::I32(n)]), Issue::Valeurs(vec![]));
        let Valeur::I64(reste) = e.globale(CARBURANT) else {
            panic!()
        };
        1_000_000 - reste
    };
    let c0 = consomme(0);
    let c1 = consomme(1);
    assert!(c1 > c0);
    for n in [2, 7, 100] {
        assert_eq!(consomme(n), consomme(n));
        assert_eq!(consomme(n), c0 + i64::from(n) * (c1 - c0));
    }
}

/// `memory.copy` et `memory.fill` coûtent leur longueur divisée par 64.
#[test]
fn carburant_compte_la_longueur_des_copies() {
    let m = chaine(
        &wat(r#"(module (memory 1 1)
          (func (export "f") (param $n i32)
            (memory.fill (i32.const 0) (i32.const 7) (local.get $n))
            (memory.copy (i32.const 0) (i32.const 1000) (local.get $n))))"#),
        None,
    );
    let consomme = |n: i32| {
        let mut e = Execution::new(&m, MOTEUR_A);
        e.poser(CARBURANT, Val::I64(1_000_000));
        e.appeler("f", &[Valeur::I32(n)]);
        let Valeur::I64(reste) = e.globale(CARBURANT) else {
            panic!()
        };
        1_000_000 - reste
    };
    assert_eq!(consomme(6400) - consomme(0), 2 * 100);
}

/// `memory.init` paie sa longueur comme `memory.copy` : sinon une boucle copie des
/// gigaoctets d'un segment passif sous le budget d'une entrée, et seul le garde-fou
/// mural l'arrête, selon la charge de la machine (SQ-59, HIS-06, OUT-11).
#[test]
fn carburant_compte_la_longueur_de_memory_init() {
    let segment = "a".repeat(6400);
    let m = chaine(
        &wat(&format!(
            r#"(module (memory 1 1)
          (data $d "{segment}")
          (func (export "f") (param $n i32)
            (memory.init $d (i32.const 0) (i32.const 0) (local.get $n))))"#
        )),
        None,
    );
    let consomme = |n: i32| {
        let mut e = Execution::new(&m, MOTEUR_A);
        e.poser(CARBURANT, Val::I64(1_000_000));
        assert_eq!(e.appeler("f", &[Valeur::I32(n)]), Issue::Valeurs(vec![]));
        let Valeur::I64(reste) = e.globale(CARBURANT) else {
            panic!()
        };
        1_000_000 - reste
    };
    assert_eq!(consomme(6400) - consomme(0), 100);
}

/// Une récursion sans fin qui compte ses étages dans une globale.
fn recursion() -> Vec<u8> {
    wat(r#"(module (memory 1 1)
      (global $etages (export "etages") (mut i32) (i32.const 0))
      (func $r (export "f") (param i64 i64 i64 i64) (result i64)
        (global.set $etages (i32.add (global.get $etages) (i32.const 1)))
        (i64.add (local.get 0)
          (call $r (local.get 1) (local.get 2) (local.get 3) (local.get 0)))))"#)
}

fn etages_au_piege(m: &[u8], r: crate::commun::Reglage) -> (Issue, i32) {
    let mut e = Execution::new(m, r);
    let args = [
        Valeur::I64(1),
        Valeur::I64(2),
        Valeur::I64(3),
        Valeur::I64(4),
    ];
    let issue = e.appeler("f", &args);
    let Valeur::I32(n) = e.globale("etages") else {
        panic!()
    };
    (issue, n)
}

#[test]
fn sans_profondeur_une_recursion_diverge_entre_deux_reglages_de_pile() {
    let m = chaine(&recursion(), Some(Passe::Profondeur));
    let (ia, na) = etages_au_piege(&m, MOTEUR_A);
    let (ib, nb) = etages_au_piege(&m, MOTEUR_B);
    assert_eq!(ia, Issue::Piege(Trap::StackOverflow));
    assert_eq!(ib, Issue::Piege(Trap::StackOverflow));
    assert_ne!(na, nb, "les deux piles natives ont piégé au même étage");
}

#[test]
fn avec_profondeur_une_recursion_piege_au_meme_etage() {
    let m = chaine(&recursion(), None);
    let (ia, na) = etages_au_piege(&m, MOTEUR_A);
    let (ib, nb) = etages_au_piege(&m, MOTEUR_B);
    assert_eq!(ia, Issue::Piege(Trap::UnreachableCodeReached));
    assert_eq!(ia, ib);
    assert_eq!(na, nb);
    // L'appel du moteur, puis `profondeur_max` appels imbriqués permis.
    assert_eq!(na, i32::try_from(REGLAGES.profondeur_max).unwrap() + 1);
    let mut e = Execution::new(&m, MOTEUR_A);
    e.appeler("f", &[Valeur::I64(0); 4]);
    assert_eq!(
        e.globale(PROFONDEUR),
        Valeur::I32(i32::try_from(REGLAGES.profondeur_max).unwrap() + 1)
    );
}

#[test]
fn profondeur_rendue_apres_chaque_appel() {
    let m = chaine(
        &wat(r#"(module (memory 1 1)
          (type $t (func (param i32) (result i32)))
          (table 1 1 funcref) (elem (i32.const 0) $g)
          (func $g (param i32) (result i32)
            (if (result i32) (i32.eqz (local.get 0))
              (then (return (i32.const 0)))
              (else (i32.add (i32.const 1) (call $g (i32.sub (local.get 0) (i32.const 1)))))))
          (func (export "f") (param i32) (result i32)
            (call_indirect (type $t) (local.get 0) (i32.const 0))))"#),
        None,
    );
    let mut e = Execution::new(&m, MOTEUR_A);
    assert_eq!(
        e.appeler("f", &[Valeur::I32(100)]),
        Issue::Valeurs(vec![Valeur::I32(100)])
    );
    assert_eq!(e.globale(PROFONDEUR), Valeur::I32(0));
}

/// Des NaN de toutes les formes : charges utiles, signes, NaN signalants, et chaque
/// opération qui peut en rendre un. L'entrée et la sortie passent par des entiers,
/// pour que l'exécuteur ne touche pas aux bits.
fn nan_de_toutes_les_formes() -> Vec<u8> {
    wat(r#"(module (memory 1 1)
      (func (export "f32") (param $op i32) (param $a i32) (param $b i32) (result i32)
        (local $x f32) (local $y f32)
        (local.set $x (f32.reinterpret_i32 (local.get $a)))
        (local.set $y (f32.reinterpret_i32 (local.get $b)))
        (i32.reinterpret_f32
          (block $b (result f32)
            (block $sqrt (block $min (block $max (block $div (block $mul (block $sub (block $add
            (block $ceil (block $floor (block $trunc (block $nearest (block $demote
              (br_table $add $sub $mul $div $max $min $sqrt $ceil $floor $trunc $nearest $demote
                (local.get $op)))
            (br $b (f32.demote_f64 (f64.promote_f32 (local.get $x)))))
            (br $b (f32.nearest (local.get $x))))
            (br $b (f32.trunc (local.get $x))))
            (br $b (f32.floor (local.get $x))))
            (br $b (f32.ceil (local.get $x))))
            (br $b (f32.add (local.get $x) (local.get $y))))
            (br $b (f32.sub (local.get $x) (local.get $y))))
            (br $b (f32.mul (local.get $x) (local.get $y))))
            (br $b (f32.div (local.get $x) (local.get $y))))
            (br $b (f32.max (local.get $x) (local.get $y))))
            (br $b (f32.min (local.get $x) (local.get $y))))
            (f32.sqrt (local.get $x)))))
      (func (export "f64") (param $op i32) (param $a i64) (param $b i64) (result i64)
        (local $x f64) (local $y f64)
        (local.set $x (f64.reinterpret_i64 (local.get $a)))
        (local.set $y (f64.reinterpret_i64 (local.get $b)))
        (i64.reinterpret_f64
          (block $b (result f64)
            (block $sqrt (block $min (block $max (block $div (block $mul (block $sub (block $add
            (block $ceil (block $floor (block $trunc (block $nearest (block $promote
              (br_table $add $sub $mul $div $max $min $sqrt $ceil $floor $trunc $nearest $promote
                (local.get $op)))
            (br $b (f64.promote_f32 (f32.demote_f64 (local.get $x)))))
            (br $b (f64.nearest (local.get $x))))
            (br $b (f64.trunc (local.get $x))))
            (br $b (f64.floor (local.get $x))))
            (br $b (f64.ceil (local.get $x))))
            (br $b (f64.add (local.get $x) (local.get $y))))
            (br $b (f64.sub (local.get $x) (local.get $y))))
            (br $b (f64.mul (local.get $x) (local.get $y))))
            (br $b (f64.div (local.get $x) (local.get $y))))
            (br $b (f64.max (local.get $x) (local.get $y))))
            (br $b (f64.min (local.get $x) (local.get $y))))
            (f64.sqrt (local.get $x))))))"#)
}

const NAN32: [u32; 5] = [
    0x7fc0_0001,
    0xffc0_1234,
    0x7fa0_0001,
    0xff80_0001,
    0x7fff_ffff,
];
const NAN64: [u64; 4] = [
    0x7ff8_0000_0000_0001,
    0xfff8_0000_dead_beef,
    0x7ff4_0000_0000_0001,
    0xfff0_0000_0000_0001,
];

/// Les bits de chaque opération sur chaque NaN, sous un réglage.
fn bits(m: &[u8], r: crate::commun::Reglage) -> Vec<Issue> {
    let mut e = Execution::new(m, r);
    let mut sorties = Vec::new();
    for op in 0..12 {
        for a in NAN32 {
            let args = [
                Valeur::I32(op),
                Valeur::I32(a.cast_signed()),
                Valeur::I32(1.5f32.to_bits().cast_signed()),
            ];
            sorties.push(e.appeler("f32", &args));
        }
        for a in NAN64 {
            let args = [
                Valeur::I32(op),
                Valeur::I64(a.cast_signed()),
                Valeur::I64(2.5f64.to_bits().cast_signed()),
            ];
            sorties.push(e.appeler("f64", &args));
        }
    }
    sorties
}

#[test]
fn sans_nan_les_bits_different_entre_deux_moteurs() {
    let m = chaine(&nan_de_toutes_les_formes(), Some(Passe::Nan));
    assert_ne!(bits(&m, MOTEUR_A), bits(&m, MOTEUR_B));
}

#[test]
fn avec_nan_les_bits_sont_canoniques_sous_les_deux_moteurs() {
    let m = chaine(&nan_de_toutes_les_formes(), None);
    let a = bits(&m, MOTEUR_A);
    assert_eq!(a, bits(&m, MOTEUR_B));
    for issue in &a {
        let Issue::Valeurs(v) = issue else {
            panic!("{issue:?}")
        };
        match v.as_slice() {
            [Valeur::I32(x)] => assert_eq!(x.cast_unsigned(), NAN_F32),
            [Valeur::I64(x)] => assert_eq!(x.cast_unsigned(), NAN_F64),
            autre => panic!("{autre:?}"),
        }
    }
}

/// Un NaN né d'opérandes finis (0/0, ∞ − ∞) est canonique aussi.
#[test]
fn nan_ne_de_nombres_finis_canonique() {
    let m = chaine(&nan_de_toutes_les_formes(), None);
    let mut e = Execution::new(&m, MOTEUR_A);
    let zero = Valeur::I32(0);
    assert_eq!(
        e.appeler("f32", &[Valeur::I32(3), zero, zero]),
        Issue::Valeurs(vec![Valeur::I32(NAN_F32.cast_signed())])
    );
    let inf = Valeur::I64(f64::INFINITY.to_bits().cast_signed());
    assert_eq!(
        e.appeler("f64", &[Valeur::I32(1), inf, inf]),
        Issue::Valeurs(vec![Valeur::I64(NAN_F64.cast_signed())])
    );
}

fn croissance_sans_fin() -> Vec<u8> {
    wat(r#"(module (memory (export "memory") 1 1000)
      (func (export "grandir") (param i32) (result i32) (memory.grow (local.get 0)))
      (func (export "taille") (result i32) (memory.size))
      (func (export "sans_fin") (result i32)
        (loop $l (br_if $l (i32.ne (memory.grow (i32.const 1)) (i32.const -1))))
        (memory.size)))"#)
}

#[test]
fn sans_croissance_memory_grow_depasse_le_plafond() {
    let m = chaine(&croissance_sans_fin(), Some(Passe::Croissance));
    let mut e = Execution::new(&m, MOTEUR_A);
    let au_dela = i32::try_from(REGLAGES.memoire_plafond_pages).unwrap() + 10;
    assert_eq!(
        e.appeler("grandir", &[Valeur::I32(au_dela)]),
        Issue::Valeurs(vec![Valeur::I32(1)])
    );
    assert_eq!(
        e.appeler("taille", &[]),
        Issue::Valeurs(vec![Valeur::I32(au_dela + 1)])
    );
}

#[test]
fn avec_croissance_memory_grow_s_arrete_au_plafond() {
    let m = chaine(&croissance_sans_fin(), None);
    let plafond = i32::try_from(REGLAGES.memoire_plafond_pages).unwrap();
    let mut e = Execution::new(&m, MOTEUR_A);
    assert_eq!(e.globale(MEMOIRE_MAX), Valeur::I32(plafond));
    assert_eq!(
        e.appeler("grandir", &[Valeur::I32(plafond)]),
        Issue::Valeurs(vec![Valeur::I32(-1)])
    );
    assert_eq!(e.globale(CROISSANCE_BORNEE), Valeur::I32(1));
    assert_eq!(e.globale(CROISSANCE_REFUSEE), Valeur::I32(0));
    // La croissance sans fin s'arrête au plafond, et pas au maximum déclaré.
    assert_eq!(
        e.appeler("sans_fin", &[]),
        Issue::Valeurs(vec![Valeur::I32(plafond)])
    );
    // Un delta énorme ne déborde pas le calcul.
    assert_eq!(
        e.appeler("grandir", &[Valeur::I32(-1)]),
        Issue::Valeurs(vec![Valeur::I32(-1)])
    );
    // Le moteur peut abaisser le maximum.
    let mut e = Execution::new(&m, MOTEUR_A);
    e.poser(MEMOIRE_MAX, Val::I32(3));
    assert_eq!(
        e.appeler("grandir", &[Valeur::I32(2)]),
        Issue::Valeurs(vec![Valeur::I32(1)])
    );
    assert_eq!(
        e.appeler("grandir", &[Valeur::I32(1)]),
        Issue::Valeurs(vec![Valeur::I32(-1)])
    );
}

/// Le maximum déclaré plus petit que le plafond : `soiree_memoire_max` le prend, et
/// un refus sous lui reste celui de la machine.
#[test]
fn croissance_prend_le_maximum_declare() {
    let m = passes::croissance(
        &wat(r#"(module (memory 1 5) (func (export "f") (result i32) (memory.grow (i32.const 10))))"#),
        64,
    )
    .unwrap();
    let mut e = Execution::new(&m, MOTEUR_A);
    assert_eq!(e.globale(MEMOIRE_MAX), Valeur::I32(5));
    assert_eq!(e.appeler("f", &[]), Issue::Valeurs(vec![Valeur::I32(-1)]));
    assert_eq!(e.globale(CROISSANCE_REFUSEE), Valeur::I32(0));
}

#[test]
fn globales_mutables_exportees() {
    let m = passes::globales(&wat("(module (memory 1 1)
          (global (mut i32) (i32.const 1))
          (global i64 (i64.const 2))
          (global (mut f64) (f64.const 3)))"))
    .unwrap();
    let mut e = Execution::new(&m, MOTEUR_A);
    assert_eq!(e.globale(&nom_de_globale(0)), Valeur::I32(1));
    assert_eq!(e.globale(&nom_de_globale(2)), Valeur::F64(3f64.to_bits()));
    assert!(
        e.instance
            .get_global(&mut e.store, &nom_de_globale(1))
            .is_none(),
        "une globale immuable n'est pas de l'état"
    );
}

/// Sans la passe des globales, une globale mutable d'origine ne se lit pas du dehors :
/// un instantané ne pourrait pas la rendre.
#[test]
fn sans_globales_l_etat_ne_se_copie_pas() {
    let m = wat("(module (memory 1 1) (global (mut i32) (i32.const 1)))");
    let mut e = Execution::new(&chaine(&m, Some(Passe::Globales)), MOTEUR_A);
    assert!(
        e.instance
            .get_global(&mut e.store, &nom_de_globale(0))
            .is_none()
    );
    let mut e = Execution::new(&chaine(&m, None), MOTEUR_A);
    assert!(
        e.instance
            .get_global(&mut e.store, &nom_de_globale(0))
            .is_some()
    );
}

/// `data.drop` devient `nop` : l'état d'un segment ne vit pas hors de la mémoire.
#[test]
fn data_drop_devient_nop() {
    let m = wat(r#"(module (memory 1 1) (data $d "xyz")
      (func (export "f") (result i32)
        (data.drop $d)
        (memory.init $d (i32.const 0) (i32.const 0) (i32.const 3))
        (i32.load8_u (i32.const 2))))"#);
    let mut avant = Execution::new(&m, MOTEUR_A);
    assert_eq!(
        avant.appeler("f", &[]),
        Issue::Piege(Trap::MemoryOutOfBounds)
    );
    let mut apres = Execution::new(&passes::globales(&m).unwrap(), MOTEUR_A);
    assert_eq!(
        apres.appeler("f", &[]),
        Issue::Valeurs(vec![Valeur::I32(i32::from(b'z'))])
    );
}

/// Un module qui imite ce que les trois jeux ont constaté (`conception/BESOINS-DES-JEUX.md`,
/// besoin 1) : une lecture (`taille` 0) laisse des octets sur la pile ; une écriture
/// (`taille` 1) copie dans l'état un mot qu'elle n'a pas écrit, sous le pointeur de pile,
/// comme le bourrage d'une valeur bâtie sur la pile puis copiée dans le tas.
fn pile_laissee() -> Vec<u8> {
    wat(r#"(module
  (memory (export "memory") 4 16)
  (global $__stack_pointer (mut i32) (i32.const 65536))
  (func (export "soiree_boite") (param i32 i32) (result i32) (i32.const 65536))
  (func (export "soiree_appel") (param $taille i32) (result i64)
    (if (i32.eqz (local.get $taille))
      (then
        (memory.fill
          (i32.sub (global.get $__stack_pointer) (i32.const 64))
          (i32.const 0xab)
          (i32.const 64)))
      (else
        (i64.store (i32.const 131072)
          (i64.load (i32.sub (global.get $__stack_pointer) (i32.const 8))))))
    (i64.const 0)))"#)
}

/// Précédée d'une lecture, ou non.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Oui {
    Oui,
    Non,
}

/// L'état (tout ce qui est au-dessus de la pile) après une écriture, précédée ou non
/// d'une lecture.
fn etat_apres_l_ecriture(m: &[u8], lecture_avant: Oui) -> Vec<u8> {
    let mut e = Execution::new(m, MOTEUR_A);
    if lecture_avant == Oui::Oui {
        let _: Issue = e.appeler("soiree_boite", &[Valeur::I32(0), Valeur::I32(0)]);
        let _: Issue = e.appeler("soiree_appel", &[Valeur::I32(0)]);
    }
    let _: Issue = e.appeler("soiree_boite", &[Valeur::I32(1), Valeur::I32(1)]);
    let _: Issue = e.appeler("soiree_appel", &[Valeur::I32(1)]);
    e.memoire()[65536..].to_vec()
}

/// Besoin 1 des jeux (Punchline n° 2, Sidequest B1, Tier list n° 1) : sans la passe de
/// la pile, une lecture faite avant une écriture change l'état, par les octets que
/// l'écriture recopie de la pile.
#[test]
fn sans_pile_une_lecture_change_l_etat_par_la_pile_laissee() {
    let m = chaine(&pile_laissee(), Some(Passe::Pile));
    assert_ne!(
        etat_apres_l_ecriture(&m, Oui::Oui),
        etat_apres_l_ecriture(&m, Oui::Non)
    );
}

/// Avec elle, l'écriture part d'une pile de zéros : l'état ne dépend que des écritures.
#[test]
fn avec_pile_une_ecriture_part_d_une_pile_de_zeros() {
    let m = passes::pile(&chaine(&pile_laissee(), Some(Passe::Pile)), Some(0)).unwrap();
    wasmparser::validate(&m).unwrap();
    assert_eq!(m, chaine(&pile_laissee(), None));
    assert_eq!(
        etat_apres_l_ecriture(&m, Oui::Oui),
        etat_apres_l_ecriture(&m, Oui::Non)
    );
    // `reecrire` la passe aussi.
    let r = reecriture::reecrire(&pile_laissee(), &REGLAGES).unwrap();
    assert_eq!(r.pile_globale, Some(0));
    assert_eq!(
        etat_apres_l_ecriture(&r.octets, Oui::Oui),
        etat_apres_l_ecriture(&r.octets, Oui::Non)
    );
}

/// Une lecture ne remet pas la pile à zéro (elle n'en a pas besoin), et la remise à
/// zéro ne coûte pas de carburant au jeu.
#[test]
fn pile_remise_a_zero_aux_seules_ecritures_et_sans_carburant() {
    let m = chaine(&pile_laissee(), None);
    let mut e = Execution::new(&m, MOTEUR_A);
    let _: Issue = e.appeler("soiree_boite", &[Valeur::I32(0), Valeur::I32(0)]);
    let _: Issue = e.appeler("soiree_appel", &[Valeur::I32(0)]);
    let _: Issue = e.appeler("soiree_boite", &[Valeur::I32(0), Valeur::I32(0)]);
    assert_eq!(e.memoire()[65536 - 1], 0xab, "une lecture garde la pile");
    e.poser(CARBURANT, Val::I64(1000));
    let _: Issue = e.appeler("soiree_boite", &[Valeur::I32(1), Valeur::I32(1)]);
    assert!(e.memoire()[..65536].iter().all(|o| *o == 0));
    let Valeur::I64(reste) = e.globale(CARBURANT) else {
        panic!("carburant d'un autre type")
    };
    // `soiree_boite` coûte ses deux instructions d'origine ; les 65 536 octets remis à
    // zéro ne coûtent rien.
    assert!(reste >= 1000 - 4, "il reste {reste}");
}
