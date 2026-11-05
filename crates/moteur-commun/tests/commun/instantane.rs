//! L'instantané hors des quatre zones (PHASE-3, P3) : il garde la mémoire que les zones
//! laissent, et elle seule ; la restauration remet les zones à zéro ; l'empreinte et
//! l'avenir sont les mêmes qu'avec la mémoire entière.

use jeu_abi::{Appel, SorteAppel, encoder};
use moteur_commun::{Instantane, Machine};
use partie::Instance;
use partie::persistance::InstantaneGarde;

use crate::fausse::{BOITE, BROUILLON, ETAT, PAGES, PANIQUE, PILE, compter_et_salir, instance};

/// Un appel d'écriture quelconque.
pub fn entree() -> Vec<u8> {
    let mut v = Vec::new();
    encoder(
        &Appel::Entree {
            ordre: 1,
            place: 1,
            entree: &[],
        },
        &mut v,
    )
    .unwrap();
    v
}

fn jouer(i: &mut impl Instance) {
    i.appeler(SorteAppel::Ecriture, &entree(), 1_000_000)
        .unwrap();
}

/// Les quatre zones de la machine sont-elles à zéro ?
fn zones_a_zero(memoire: &[u8]) -> bool {
    [(0, PILE), BOITE, BROUILLON, PANIQUE].iter().all(|(a, n)| {
        let a = usize::try_from(*a).unwrap();
        memoire[a..a + usize::try_from(*n).unwrap()]
            .iter()
            .all(|o| *o == 0)
    })
}

#[test]
fn instantane_sans_zones_meme_empreinte() {
    let mut a = instance(compter_et_salir);
    for _ in 0..3 {
        jouer(&mut a);
    }
    // L'appel a laissé quelque chose dans chaque zone.
    assert!(!zones_a_zero(&a.machine().memoire));
    let point = a.instantane().unwrap();
    let e = a.empreinte().unwrap();
    // Exactement la mémoire hors des zones : ici, tout ce qui suit la zone de panique.
    let fin = PAGES * 65_536;
    assert_eq!(point.plages(), [(u64::from(ETAT), fin - u64::from(ETAT))]);
    assert_eq!(point.plages(), a.carte().plages(PAGES).as_slice());
    assert_eq!(
        u64::try_from(point.octets().len()).unwrap(),
        fin - a.carte().octets_remis_a_zero()
    );
    assert_eq!(point.pages(), PAGES);
    // Dans une instance neuve : même empreinte, zones à zéro, même instantané.
    let mut b = instance(compter_et_salir);
    b.restaurer(&point).unwrap();
    assert_eq!(b.empreinte().unwrap(), e);
    assert!(zones_a_zero(&b.machine().memoire));
    assert_eq!(b.machine().compteur(), 3);
    assert!(
        b.instantane().unwrap() == point,
        "l'instantané de l'instance restaurée est le point"
    );
    // Le même avenir.
    jouer(&mut a);
    jouer(&mut b);
    assert_eq!(a.empreinte().unwrap(), b.empreinte().unwrap());
    // Dans l'instance d'origine, après d'autres appels : de retour au point, zones à zéro.
    a.restaurer(&point).unwrap();
    assert_eq!(a.empreinte().unwrap(), e);
    assert!(zones_a_zero(&a.machine().memoire));
    assert_eq!(a.instances_creees(), 1);
}

/// Une mémoire ne rétrécit pas : restaurer un instantané plus petit se fait dans une
/// machine neuve, `soiree_memoire_max` reposé.
#[test]
fn restaurer_dans_plus_grand_cree_une_machine_neuve() {
    let mut a = instance(compter_et_salir);
    jouer(&mut a);
    let point = a.instantane().unwrap();
    let e = a.empreinte().unwrap();
    a.machine().grandir_jusqu_a(PAGES + 2).unwrap();
    assert_ne!(a.empreinte().unwrap(), e);
    a.restaurer(&point).unwrap();
    assert_eq!(a.instances_creees(), 2);
    assert_eq!(a.pages(), PAGES);
    assert_eq!(a.empreinte().unwrap(), e);
    assert_eq!(a.memoire_max().unwrap(), 8);
}

/// Un instantané plus grand que la mémoire d'une instance se restaure en la faisant
/// grandir, sans machine neuve : même taille, même empreinte, même avenir. À taille
/// égale, la restauration ne demande aucune croissance : dans un navigateur,
/// `memory.grow`, même de zéro page, détache le tampon de la mémoire (spécification de
/// l'API JavaScript de WebAssembly, « refresh the memory buffer »).
#[test]
fn restaurer_dans_plus_petit_fait_grandir() {
    let mut a = instance(compter_et_salir);
    a.machine().grandir_jusqu_a(PAGES + 2).unwrap();
    jouer(&mut a);
    let point = a.instantane().unwrap();
    let e = a.empreinte().unwrap();
    assert_eq!(point.pages(), PAGES + 2);
    let mut b = instance(compter_et_salir);
    assert_eq!(b.pages(), PAGES);
    b.restaurer(&point).unwrap();
    assert_eq!(b.pages(), PAGES + 2);
    assert_eq!(b.instances_creees(), 1);
    assert_eq!(b.machine().croissances, 1);
    assert_eq!(b.empreinte().unwrap(), e);
    assert!(
        b.instantane().unwrap() == point,
        "l'instantané de l'instance restaurée est le point"
    );
    jouer(&mut a);
    jouer(&mut b);
    assert_eq!(a.empreinte().unwrap(), b.empreinte().unwrap());
    // À taille égale : aucune croissance de plus.
    b.restaurer(&point).unwrap();
    assert_eq!(b.machine().croissances, 1);
    assert_eq!(b.empreinte().unwrap(), e);
}

/// Un instantané relu de la garde, dont les plages ne sont pas le reste des zones du
/// module, est refusé : la relecture ne connaît pas les zones, la restauration si.
#[test]
fn un_instantane_d_une_autre_forme_est_refuse() {
    let mut a = instance(compter_et_salir);
    jouer(&mut a);
    let point = a.instantane().unwrap();
    let mut o = Vec::new();
    point.ecrire(&mut o);
    assert!(
        Instantane::lire(&o).unwrap() == point,
        "le point relu est le même"
    );
    // L'adresse de la seule plage, après l'empreinte, les pages et le nombre de plages :
    // quatre octets plus loin, la plage est toujours dans la mémoire, mais plus à sa
    // place.
    let adresse = 32 + 8 + 4;
    o[adresse..adresse + 8].copy_from_slice(&u64::from(ETAT + 4).to_le_bytes());
    // La plage déborde d'autant : on la raccourcit de quatre octets.
    let longueur = adresse + 8;
    let n = u64::from_le_bytes(o[longueur..longueur + 8].try_into().unwrap()) - 4;
    o[longueur..longueur + 8].copy_from_slice(&n.to_le_bytes());
    let debut = longueur + 8;
    o.drain(debut..debut + 4);
    let autre = Instantane::lire(&o).unwrap();
    assert!(a.restaurer(&autre).is_err());
}
