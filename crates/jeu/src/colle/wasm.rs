//! Ce que `jeu::module!` assemble dans un module WebAssembly (§ 2.14, § 3.2, § 3.3) :
//! l'allocateur à deux modes, l'état du module, et le corps des deux exports.
//!
//! C'est ici et dans les exports de la macro que vit le code `unsafe` d'un module,
//! chaque fois sous `#[expect(unsafe_code, reason = "…")]`. WebAssembly sans fils
//! n'a qu'un fil d'exécution : aucun accès concurrent n'est possible.

use alloc::vec::Vec;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::mem::ManuallyDrop;
use core::sync::atomic::{AtomicU8, AtomicU64, AtomicUsize, Ordering};

use jeu_abi::{ErreurFil, Illisible, Retour, SorteAppel};

use super::{
    CAPACITE_BOITE, CAPACITE_PANIQUE, Cadre, Enveloppe, Mode, Modes, Sortie, Zone, Zones, appeler,
    effacer_panique, reserver,
};
use crate::traits::{Epreuve, Jeu};

// Le mode brouillon vaut 0 : l'allocateur est tout de zéros à sa création, donc dans
// `.bss`, et son arène ne pèse rien dans le fichier du module.
const MODE_BROUILLON: u8 = 0;
const MODE_TAS: u8 = 1;

/// L'allocateur global d'un module : `dlmalloc` en mode tas, une arène qui ne fait que
/// monter en mode brouillon (D34). Il libère selon l'adresse : une adresse dans le
/// brouillon ne touche pas le tas.
pub struct Allocateur<const N: usize> {
    brouillon: [AtomicU8; N],
    haut: AtomicUsize,
    mode: AtomicU8,
    vivants: AtomicU64,
}

impl<const N: usize> Default for Allocateur<N> {
    fn default() -> Self {
        Allocateur::new()
    }
}

impl<const N: usize> Allocateur<N> {
    /// L'allocateur, en mode brouillon, son arène vide.
    pub const fn new() -> Allocateur<N> {
        Allocateur {
            brouillon: [const { AtomicU8::new(0) }; N],
            haut: AtomicUsize::new(0),
            mode: AtomicU8::new(MODE_BROUILLON),
            vivants: AtomicU64::new(0),
        }
    }

    fn base(&self) -> *mut u8 {
        self.brouillon.as_ptr().cast::<u8>().cast_mut()
    }

    fn dans_brouillon(&self, p: *mut u8) -> bool {
        let debut = self.base().addr();
        let a = p.addr();
        debut <= a && a < debut.saturating_add(N)
    }

    /// Vide l'arène : au début de chaque appel.
    pub fn remettre_a_zero(&self) {
        self.haut.store(0, Ordering::Relaxed);
    }

    /// L'adresse et la capacité du brouillon.
    pub fn zone(&self) -> (u32, u32) {
        (
            u32::try_from(self.base().addr()).unwrap_or(0),
            u32::try_from(N).unwrap_or(u32::MAX),
        )
    }

    fn reserver(&self, l: Layout) -> *mut u8 {
        let base = self.base();
        let debut = base.addr();
        let haut = debut.saturating_add(self.haut.load(Ordering::Relaxed));
        match reserver(debut, debut.saturating_add(N), haut, l.size(), l.align()) {
            Some((depart, arrivee)) => {
                self.haut
                    .store(arrivee.saturating_sub(debut), Ordering::Relaxed);
                base.wrapping_add(depart.saturating_sub(debut))
            }
            None => core::ptr::null_mut(),
        }
    }
}

impl<const N: usize> Modes for Allocateur<N> {
    fn bascule(&self, m: Mode) {
        let code = match m {
            Mode::Tas => MODE_TAS,
            Mode::Brouillon => MODE_BROUILLON,
        };
        self.mode.store(code, Ordering::Relaxed);
    }

    fn memoire(&self) -> (u64, u64) {
        let pages = u64::try_from(core::arch::wasm32::memory_size(0)).unwrap_or(0);
        (
            self.vivants.load(Ordering::Relaxed),
            pages.saturating_mul(65_536),
        )
    }
}

#[expect(
    unsafe_code,
    reason = "l'allocateur global d'un module : dlmalloc pour le tas, une arène pour le brouillon (§ 3.2, D34)"
)]
// SAFETY : l'arène rend des blocs alignés, disjoints, dans `brouillon`, jamais rendus
// avant la remise à zéro du début d'appel ; le reste va à `dlmalloc`, que `std` emploie
// sur cette cible. Un seul fil d'exécution.
unsafe impl<const N: usize> GlobalAlloc for Allocateur<N> {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if self.mode.load(Ordering::Relaxed) == MODE_BROUILLON {
            return self.reserver(l);
        }
        // SAFETY : la disposition vient de l'appelant, comme le demande `GlobalAlloc`.
        let p = unsafe { dlmalloc::GlobalDlmalloc.alloc(l) };
        if !p.is_null() {
            let _: u64 = self.vivants.fetch_add(l.size() as u64, Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        if self.dans_brouillon(p) {
            return;
        }
        // SAFETY : `p` vient de `dlmalloc` avec cette disposition : il n'est pas dans le brouillon.
        unsafe { dlmalloc::GlobalDlmalloc.dealloc(p, l) };
        let _: u64 = self.vivants.fetch_sub(l.size() as u64, Ordering::Relaxed);
    }

    unsafe fn realloc(&self, p: *mut u8, l: Layout, taille: usize) -> *mut u8 {
        let tas = self.mode.load(Ordering::Relaxed) == MODE_TAS;
        if tas && !self.dans_brouillon(p) {
            // SAFETY : `p` vient de `dlmalloc` avec la disposition `l`.
            let q = unsafe { dlmalloc::GlobalDlmalloc.realloc(p, l, taille) };
            if !q.is_null() {
                let _: u64 = self.vivants.fetch_sub(l.size() as u64, Ordering::Relaxed);
                let _: u64 = self.vivants.fetch_add(taille as u64, Ordering::Relaxed);
            }
            return q;
        }
        let Ok(nouvelle) = Layout::from_size_align(taille, l.align()) else {
            return core::ptr::null_mut();
        };
        // SAFETY : une allocation ordinaire dans le mode courant.
        let q = unsafe { self.alloc(nouvelle) };
        if !q.is_null() {
            // SAFETY : deux blocs distincts, chacun d'au moins `min(ancienne, nouvelle)` octets.
            unsafe { core::ptr::copy_nonoverlapping(p, q, l.size().min(taille)) };
            // SAFETY : `p` a été alloué avec `l`.
            unsafe { self.dealloc(p, l) };
        }
        q
    }
}

/// Une cellule du seul fil d'exécution d'un module.
pub struct Cellule<T>(UnsafeCell<T>);

#[expect(
    unsafe_code,
    reason = "WebAssembly sans fils n'a qu'un fil d'exécution : la cellule n'est jamais partagée"
)]
// SAFETY : sans la fonctionnalité des fils, refusée au chargement (§ 3.1), aucun autre
// fil ne peut atteindre la cellule.
unsafe impl<T> Sync for Cellule<T> {}

impl<T> Cellule<T> {
    /// Une cellule.
    pub const fn new(v: T) -> Cellule<T> {
        Cellule(UnsafeCell::new(v))
    }

    /// Prête la valeur. Jamais deux fois à la fois : les exports ne se rappellent pas
    /// eux-mêmes, et une panique arrête le module (`unreachable`).
    #[expect(unsafe_code, reason = "le prêt unique de l'état du module à un export")]
    fn avec<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        // SAFETY : un seul fil, et aucun export n'est réentrant : ce prêt est le seul.
        f(unsafe { &mut *self.0.get() })
    }
}

/// L'état d'un module entre deux appels.
pub struct EtatModule<J: Jeu> {
    enveloppe: Option<Enveloppe<J>>,
    sorte: Option<SorteAppel>,
    tampon: Option<Vec<u8>>,
}

impl<J: Jeu> EtatModule<J> {
    /// Avant toute création.
    pub const fn new() -> EtatModule<J> {
        EtatModule {
            enveloppe: None,
            sorte: None,
            tampon: None,
        }
    }
}

impl<J: Jeu> Default for EtatModule<J> {
    fn default() -> Self {
        EtatModule::new()
    }
}

/// Le corps de `soiree_boite(taille, sorte)` (§ 3.3).
pub fn boite<J: Jeu, const N: usize>(
    etat: &Cellule<EtatModule<J>>,
    alloc: &Allocateur<N>,
    zone: &Zone<CAPACITE_BOITE>,
    taille: i32,
    sorte: i32,
) -> i32 {
    let (Ok(n), Some(s)) = (usize::try_from(taille), SorteAppel::depuis_code(sorte)) else {
        return 0;
    };
    etat.avec(|e| {
        e.sorte = Some(s);
        e.tampon = None;
        if n <= zone.capacite() {
            return i32::try_from(zone.adresse().addr()).unwrap_or(0);
        }
        match s {
            SorteAppel::Lecture => 0,
            SorteAppel::Ecriture => {
                // Au-delà de la boîte, une écriture reçoit un tampon du tas, libéré à la
                // fin de l'appel : le rejeu du journal refait la même allocation.
                alloc.bascule(Mode::Tas);
                let t = alloc::vec![0u8; n];
                let adresse = t.as_ptr().addr();
                e.tampon = Some(t);
                i32::try_from(adresse).unwrap_or(0)
            }
        }
    })
}

/// Le corps de `soiree_appel(taille)` (§ 3.3, § 3.4).
#[expect(
    unsafe_code,
    reason = "lire les octets que le moteur a écrits dans la boîte statique"
)]
pub fn appel<J: Jeu + Epreuve, const N: usize>(
    etat: &Cellule<EtatModule<J>>,
    alloc: &Allocateur<N>,
    zone: &Zone<CAPACITE_BOITE>,
    panique: &Zone<CAPACITE_PANIQUE>,
    taille: i32,
) -> i64 {
    alloc.remettre_a_zero();
    effacer_panique(panique);
    let zones = Zones {
        boite: (
            u32::try_from(zone.adresse().addr()).unwrap_or(0),
            u32::try_from(CAPACITE_BOITE).unwrap_or(0),
        ),
        brouillon: alloc.zone(),
        panique: (
            u32::try_from(panique.adresse().addr()).unwrap_or(0),
            u32::try_from(CAPACITE_PANIQUE).unwrap_or(0),
        ),
    };
    let cadre = Cadre {
        modes: alloc,
        zones,
    };
    let octets = etat.avec(|e| {
        let n = usize::try_from(taille).unwrap_or(usize::MAX);
        let Some(sorte) = e.sorte.take() else {
            // Aucun `soiree_boite` avant : le moteur n'a rien écrit.
            alloc.bascule(Mode::Brouillon);
            let mut s = Sortie::new(cadre, SorteAppel::Lecture);
            let _: Result<(), jeu_abi::ErreurEncodage> = jeu_abi::encoder(
                &Retour::Illisible(ErreurFil {
                    ou: 0,
                    quoi: Illisible::Ordre,
                }),
                &mut s.octets,
            );
            return s.octets;
        };
        alloc.bascule(match sorte {
            SorteAppel::Lecture => Mode::Brouillon,
            SorteAppel::Ecriture => Mode::Tas,
        });
        let tampon = e.tampon.take();
        let entree: &[u8] = match &tampon {
            Some(t) => t.get(..n).unwrap_or(&[]),
            None if n <= zone.capacite() => {
                // SAFETY : `n` octets de la boîte statique, que le moteur vient d'écrire ;
                // personne d'autre ne les touche pendant l'appel.
                unsafe { core::slice::from_raw_parts(zone.adresse().cast_const(), n) }
            }
            None => &[],
        };
        let mut s = Sortie::new(cadre, sorte);
        let _: Result<(), ErreurFil> = appeler(&mut e.enveloppe, entree, &mut s);
        alloc.bascule(Mode::Brouillon);
        // Le tampon d'une grande écriture rend sa mémoire au tas (libéré selon son adresse).
        drop(tampon);
        s.octets
    });
    // La sortie est dans le brouillon : elle y reste jusqu'au prochain appel, le temps
    // que le moteur la copie.
    let octets = ManuallyDrop::new(octets);
    let adresse = u64::try_from(octets.as_ptr().addr()).unwrap_or(0);
    let longueur = u64::try_from(octets.len()).unwrap_or(0);
    // Le haut de l'arène est hors du brouillon, dans l'allocateur : le remettre à zéro dès
    // maintenant laisse la mémoire hors des zones exclues telle qu'avant une lecture. La
    // sortie reste où elle est jusqu'au prochain appel.
    alloc.remettre_a_zero();
    (adresse.wrapping_shl(32) | (longueur & 0xffff_ffff)).cast_signed()
}
