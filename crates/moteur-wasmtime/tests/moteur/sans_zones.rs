//! L'instantané hors des quatre zones, dans wasmtime (PHASE-3, § 2.4, P3) : il garde la
//! mémoire que les zones laissent, et elle seule ; relu de la garde et restauré dans une
//! instance neuve, il rend la même empreinte et le même avenir.

use std::fmt::Write as _;

use moteur_wasmtime::InstantaneWasm;
use partie::persistance::InstantaneGarde;
use partie::{Fabrique, Instance};

use crate::commun::{BOITE, BROUILLON, Colle, ECRITURE, LECTURE, PANIQUE, entree, lecture};

/// Une écriture qui ajoute son ordre au compteur de l'état, et laisse un compteur dans
/// chacune des quatre zones : la pile, la boîte au-delà de l'appel, le brouillon au-delà
/// du retour, la zone de panique au-delà de son entête.
fn colle_qui_salit_les_zones() -> Colle {
    let mut salir = String::new();
    for a in [
        1000,
        BOITE.0.checked_add(3000).unwrap(),
        BROUILLON.0.checked_add(300).unwrap(),
        PANIQUE.0.checked_add(3000).unwrap(),
    ] {
        write!(
            salir,
            "(i64.store (i32.const {a}) (i64.add (i64.load (i32.const {a})) (i64.const 1)))"
        )
        .unwrap();
    }
    Colle::new(&format!(
        "{salir}
         (i64.store (i32.const 200000)
           (i64.add (i64.load (i32.const 200000)) (i64.load8_u (i32.const 65537))))
         (call $fait)"
    ))
}

#[test]
fn instantane_sans_zones_meme_empreinte() {
    let module = colle_qui_salit_les_zones().charger();
    let carte = module.carte();
    let mut fabrique = module.fabrique();
    let mut a = fabrique.neuve().unwrap();
    for ordre in [1, 2, 3] {
        a.appeler(ECRITURE, &entree(ordre), 1_000_000).unwrap();
    }
    a.appeler(LECTURE, &lecture(), 1_000_000).unwrap();
    let point = a.instantane().unwrap();
    let avant = a.empreinte().unwrap();
    // Exactement la mémoire hors des zones : aucune plage n'en touche une, et ensemble
    // elles font la mémoire moins les zones.
    let pages = a.pages().unwrap();
    assert_eq!(point.pages(), pages);
    assert_eq!(point.plages(), carte.plages(pages).as_slice());
    for (za, zn) in carte.remises_a_zero() {
        let (za, zf) = (u64::from(za), u64::from(za) + u64::from(zn));
        for (pa, pn) in point.plages() {
            assert!(
                pa + pn <= za || zf <= *pa,
                "la plage {pa}+{pn} touche une zone"
            );
        }
    }
    assert_eq!(
        u64::try_from(point.octets().len()).unwrap(),
        pages * 65_536 - module.octets_remis_a_zero()
    );
    // Écrit et relu par la garde.
    let mut garde = Vec::new();
    point.ecrire(&mut garde);
    let relu = InstantaneWasm::lire(&garde).unwrap();
    assert!(relu == point, "l'instantané relu est le même");
    // Restauré dans une instance neuve : la même empreinte, le même avenir.
    let mut b = fabrique.neuve().unwrap();
    b.restaurer(&relu).unwrap();
    assert_eq!(b.empreinte().unwrap(), avant);
    for ordre in [4, 50, 5] {
        a.appeler(ECRITURE, &entree(ordre), 1_000_000).unwrap();
        b.appeler(ECRITURE, &entree(ordre), 1_000_000).unwrap();
        assert_eq!(a.empreinte().unwrap(), b.empreinte().unwrap());
    }
    // Et dans l'instance d'origine, après ces écritures : de retour au point.
    a.restaurer(&relu).unwrap();
    assert_eq!(a.empreinte().unwrap(), avant);
}
