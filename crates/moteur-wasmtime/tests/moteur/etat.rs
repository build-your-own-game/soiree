//! L'état d'une instance : instantané, restauration, empreinte des zones d'état
//! (§ 3.2, § 4.5).

use std::fmt::Write as _;

use partie::{Fabrique, Instance};

use crate::commun::{BROUILLON, COMPTER, Colle, ECRITURE, LECTURE, entree, lecture};

/// Un compteur, une croissance à la demande (l'ordre 50 grandit de 3 pages), un
/// flottant calculé depuis un NaN (SQ-68), et une globale mutable d'origine.
fn colle() -> Colle {
    Colle::new(
        "(if (i32.eq (i32.load8_u (i32.const 65537)) (i32.const 50))
           (then (drop (memory.grow (i32.const 3)))))
         (global.set $tours (i32.add (global.get $tours) (i32.const 1)))
         (f32.store (i32.const 200008)
           (f32.add (f32.reinterpret_i32 (i32.const 0x7fc01234))
                    (f32.convert_i32_u (i32.load8_u (i32.const 65537)))))
         (i64.store (i32.const 200000)
           (i64.add (i64.load (i32.const 200000)) (i64.load8_u (i32.const 65537))))
         (call $fait)",
    )
    .extra("(global $tours (mut i32) (i32.const 0))")
}

/// Une lecture qui salit le brouillon, sans toucher à l'état.
fn colle_avec_lecture_qui_salit() -> Colle {
    let mut c = colle();
    c.jouer = format!(
        "(if (i32.eq (i32.load8_u (i32.const 65536)) (i32.const 18))
           (then (memory.fill (i32.const {}) (i32.const 0xaa) (i32.const 1000))
                 (return (call $fait))))
         {}",
        BROUILLON.0.checked_add(100).unwrap(),
        c.jouer
    );
    c
}

#[test]
fn deux_instances_meme_journal_meme_empreinte() {
    let module = colle().charger();
    let mut fabrique = module.fabrique();
    let mut a = fabrique.neuve().unwrap();
    let mut b = fabrique.neuve().unwrap();
    assert_eq!(a.empreinte().unwrap(), b.empreinte().unwrap());
    let mut empreintes = Vec::new();
    for ordre in [1, 2, 3, 50, 4, 5] {
        a.appeler(ECRITURE, &entree(ordre), 1_000_000).unwrap();
        b.appeler(ECRITURE, &entree(ordre), 1_000_000).unwrap();
        assert_eq!(a.carburant(), b.carburant());
        let e = a.empreinte().unwrap();
        assert_eq!(e, b.empreinte().unwrap());
        empreintes.push(e);
    }
    // Chaque entrée change l'état : l'empreinte le voit.
    empreintes.dedup();
    assert_eq!(empreintes.len(), 6);
    // Une autre instance, chargée par un autre moteur, aussi.
    let mut c = colle().charger().fabrique().neuve().unwrap();
    for ordre in [1, 2, 3, 50, 4, 5] {
        c.appeler(ECRITURE, &entree(ordre), 1_000_000).unwrap();
    }
    assert_eq!(c.empreinte().unwrap(), a.empreinte().unwrap());
}

/// Une lecture qui écrit dans le brouillon ne change pas l'empreinte (D4) ; une
/// écriture dans l'état la change.
#[test]
fn empreinte_ignore_le_brouillon() {
    let mut i = colle_avec_lecture_qui_salit()
        .charger()
        .fabrique()
        .neuve()
        .unwrap();
    i.appeler(ECRITURE, &entree(1), 1_000_000).unwrap();
    let e = i.empreinte().unwrap();
    i.appeler(LECTURE, &lecture(), 1_000_000).unwrap();
    assert_eq!(i.empreinte().unwrap(), e);
    i.appeler(ECRITURE, &entree(1), 1_000_000).unwrap();
    assert_ne!(i.empreinte().unwrap(), e);
}

#[test]
fn instantane_restaurer_meme_empreinte() {
    let mut f = colle().charger().fabrique();
    let mut a = f.neuve().unwrap();
    for ordre in [1, 2, 3] {
        a.appeler(ECRITURE, &entree(ordre), 1_000_000).unwrap();
    }
    let point = a.instantane().unwrap();
    let e = a.empreinte().unwrap();
    for ordre in [4, 5] {
        a.appeler(ECRITURE, &entree(ordre), 1_000_000).unwrap();
    }
    assert_ne!(a.empreinte().unwrap(), e);
    a.restaurer(&point).unwrap();
    assert_eq!(a.empreinte().unwrap(), e);
    assert_eq!(a.instances_creees(), 1);
    // Dans une instance neuve aussi, et la suite est la même.
    let mut b = f.neuve().unwrap();
    b.restaurer(&point).unwrap();
    assert_eq!(b.empreinte().unwrap(), e);
    for ordre in [4, 5] {
        a.appeler(ECRITURE, &entree(ordre), 1_000_000).unwrap();
        b.appeler(ECRITURE, &entree(ordre), 1_000_000).unwrap();
    }
    assert_eq!(a.empreinte().unwrap(), b.empreinte().unwrap());
}

/// Une mémoire ne rétrécit pas : restaurer un instantané plus petit crée une
/// instance neuve, et la taille est celle de l'instantané (§ 8.6).
#[test]
fn restaurer_dans_plus_grand_cree_une_instance_neuve() {
    let mut f = colle().charger().fabrique();
    let mut a = f.neuve().unwrap();
    a.appeler(ECRITURE, &entree(1), 1_000_000).unwrap();
    let petit = a.instantane().unwrap();
    let e = a.empreinte().unwrap();
    a.appeler(ECRITURE, &entree(50), 1_000_000).unwrap();
    assert_eq!(a.pages().unwrap(), petit.pages() + 3);
    let grand = a.instantane().unwrap();
    let eg = a.empreinte().unwrap();
    a.restaurer(&petit).unwrap();
    assert_eq!(a.instances_creees(), 2);
    assert_eq!(a.pages().unwrap(), petit.pages());
    assert_eq!(a.empreinte().unwrap(), e);
    // `soiree_memoire_max` est reposé sur l'instance neuve.
    assert_eq!(a.memoire_max().unwrap(), 16);
    // Et dans l'autre sens, la mémoire grandit sans instance neuve.
    a.restaurer(&grand).unwrap();
    assert_eq!(a.instances_creees(), 2);
    assert_eq!(a.empreinte().unwrap(), eg);
    // Un instantané d'un autre module est refusé.
    let mut autre = crate::commun::Colle::new(COMPTER)
        .charger()
        .fabrique()
        .neuve()
        .unwrap();
    assert!(autre.restaurer(&petit).is_err());
}

/// La taille de la mémoire compte dans l'empreinte, même quand les octets ajoutés
/// sont nuls.
#[test]
fn empreinte_compte_la_taille() {
    let c = Colle::new("(drop (memory.grow (i32.const 1))) (call $fait)");
    let mut i = c.charger().fabrique().neuve().unwrap();
    let e = i.empreinte().unwrap();
    i.appeler(ECRITURE, &entree(1), 1_000_000).unwrap();
    assert_ne!(i.empreinte().unwrap(), e);
}

/// Une lecture qui laisse un compteur dans chacune des zones que l'empreinte
/// exclut : la pile, la boîte au-delà de l'entrée, le brouillon, la zone de panique
/// au-delà de l'entête. Une écriture ajoute ces quatre compteurs à l'état.
fn colle_qui_relit_les_zones_exclues() -> Colle {
    let adresses = [
        1000,
        crate::commun::BOITE.0.checked_add(3000).unwrap(),
        BROUILLON.0.checked_add(300).unwrap(),
        crate::commun::PANIQUE.0.checked_add(3000).unwrap(),
    ];
    let mut lecture = String::new();
    let mut ecriture = String::new();
    for a in adresses {
        write!(
            lecture,
            "(i64.store (i32.const {a}) (i64.add (i64.load (i32.const {a})) (i64.const 1)))"
        )
        .unwrap();
        write!(
            ecriture,
            "(i64.store (i32.const 210000)
               (i64.add (i64.load (i32.const 210000)) (i64.load (i32.const {a}))))"
        )
        .unwrap();
    }
    Colle::new(&format!(
        "(if (i32.eq (i32.load8_u (i32.const 65536)) (i32.const 18))
           (then {lecture} (return (call $fait))))
         {ecriture}
         (call $fait)"
    ))
}

/// D4 et § 3.2 : ce qu'une lecture écrit dans une zone exclue de l'empreinte ne
/// survit pas à l'appel. Deux instances au même journal d'écritures, l'une avec
/// trois lectures de plus, gardent la même empreinte après l'écriture suivante.
#[test]
fn une_lecture_ne_laisse_rien_dans_les_zones_exclues() {
    let module = colle_qui_relit_les_zones_exclues().charger();
    let mut f = module.fabrique();
    let mut a = f.neuve().unwrap();
    let mut b = f.neuve().unwrap();
    a.appeler(ECRITURE, &entree(1), 1_000_000).unwrap();
    b.appeler(ECRITURE, &entree(1), 1_000_000).unwrap();
    let e = a.empreinte().unwrap();
    assert_eq!(e, b.empreinte().unwrap());
    for _ in 0..3 {
        a.appeler(LECTURE, &lecture(), 1_000_000).unwrap();
        // E7 : la lecture ne change pas l'empreinte.
        assert_eq!(a.empreinte().unwrap(), e);
    }
    a.appeler(ECRITURE, &entree(2), 1_000_000).unwrap();
    b.appeler(ECRITURE, &entree(2), 1_000_000).unwrap();
    assert_eq!(a.empreinte().unwrap(), b.empreinte().unwrap());
}

/// Ce que l'empreinte ne voit pas ne passe pas d'une écriture à la suivante : deux
/// instances de même empreinte ont le même avenir, même quand une écriture a caché
/// une valeur dans le brouillon. Sans quoi un module qui déclare un brouillon géant
/// y garde son état hors de toute comparaison.
#[test]
fn meme_empreinte_meme_avenir_malgre_le_brouillon() {
    let cache = BROUILLON.0.checked_add(500).unwrap();
    let c = Colle::new(&format!(
        "(if (i32.eq (i32.load8_u (i32.const 65537)) (i32.const 9))
           (then (i64.store (i32.const 210000) (i64.load (i32.const {cache})))
                 (return (call $fait))))
         (i64.store (i32.const {cache}) (i64.load8_u (i32.const 65537)))
         (call $fait)"
    ));
    let module = c.charger();
    let mut f = module.fabrique();
    let mut a = f.neuve().unwrap();
    let mut b = f.neuve().unwrap();
    // Deux entrées différentes qui ne laissent rien dans l'état : même empreinte.
    a.appeler(ECRITURE, &entree(3), 1_000_000).unwrap();
    b.appeler(ECRITURE, &entree(4), 1_000_000).unwrap();
    assert_eq!(a.empreinte().unwrap(), b.empreinte().unwrap());
    a.appeler(ECRITURE, &entree(9), 1_000_000).unwrap();
    b.appeler(ECRITURE, &entree(9), 1_000_000).unwrap();
    assert_eq!(a.empreinte().unwrap(), b.empreinte().unwrap());
}
