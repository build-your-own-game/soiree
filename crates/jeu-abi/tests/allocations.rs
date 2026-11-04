//! Une longueur absurde échoue sans réserver de mémoire pour elle (REM-58).
//!
//! Ce test a son propre binaire : il remplace l'allocateur global pour compter ce
//! que le décodage réserve, sur le fil qui décode.

#![cfg(test)]
#![expect(
    unsafe_code,
    reason = "compter les allocations demande un allocateur global, qui délègue au système"
)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use jeu_abi::{Appel, decoder};

struct Compteur;

thread_local! {
    static ALLOUE: Cell<usize> = const { Cell::new(0) };
}

fn compter(n: usize) {
    // Pendant la destruction du fil, le compteur n'existe plus : on ne compte rien.
    let _: Result<(), _> = ALLOUE.try_with(|a| a.set(a.get().saturating_add(n)));
}

// SAFETY : chaque méthode délègue à `System` avec les mêmes arguments, et ne fait
// que compter les octets demandés.
unsafe impl GlobalAlloc for Compteur {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        compter(l.size());
        // SAFETY : le contrat de `alloc` est celui de l'appelant.
        unsafe { System.alloc(l) }
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY : le contrat de `dealloc` est celui de l'appelant.
        unsafe { System.dealloc(p, l) }
    }

    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        compter(n);
        // SAFETY : le contrat de `realloc` est celui de l'appelant.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static ALLOCATEUR: Compteur = Compteur;

/// Les octets réservés par `f`, sur ce fil.
fn reserve(f: impl FnOnce()) -> usize {
    let avant = ALLOUE.with(Cell::get);
    f();
    ALLOUE.with(Cell::get).saturating_sub(avant)
}

/// Un varint LEB128 de `v`, écrit ici sans l'encodeur.
fn varint(mut v: u64) -> Vec<u8> {
    let mut o = Vec::new();
    loop {
        let bas = u8::try_from(v & 0x7f).unwrap();
        v >>= 7;
        if v == 0 {
            o.push(bas);
            return o;
        }
        o.push(bas | 0x80);
    }
}

#[test]
fn longueur_absurde_echoue_sans_reserver() {
    for longueur in [1 << 20, 1 << 40, 1 << 62, u64::MAX >> 1] {
        // `Regarder` (5) annonce `longueur` regards ; `Etat` (6) et `Ressources` (7),
        // autant de `u64`. Aucun octet ne suit : rien n'est réservé.
        for variant in [5u8, 6, 7] {
            let mut octets = vec![variant];
            octets.extend(varint(longueur));
            let mut resultat = Ok(());
            let n = reserve(|| resultat = decoder::<Appel<'_>>(&octets).map(|_| ()));
            assert!(resultat.is_err(), "variant {variant}, longueur {longueur}");
            assert_eq!(
                n, 0,
                "variant {variant}, longueur {longueur} : {n} octets réservés"
            );

            // Suivie de 64 Kio d'éléments valides, la réservation suit les octets
            // réellement présents, jamais la longueur annoncée.
            octets.extend(std::iter::repeat_n(0u8, 64 * 1024));
            let n = reserve(|| resultat = decoder::<Appel<'_>>(&octets).map(|_| ()));
            assert!(resultat.is_err());
            assert!(
                n <= octets.len().saturating_mul(64),
                "variant {variant} : {n} octets réservés pour {} octets reçus",
                octets.len()
            );
        }
    }
}
