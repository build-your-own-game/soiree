//! Le module réécrit dans le navigateur, comme `partie::Instance` (PHASE-3, § 2.2 et
//! § 2.3, lot 3a) : les primitives de `WebAssembly.Module`, `Instance`, `Memory` et
//! `Global` sous le protocole du commun (`moteur-commun`, P2).
//!
//! Ce qui est ici, et lui seul :
//!
//! - [`MachineNavigateur`], l'`impl moteur_commun::Machine` du navigateur : lire,
//!   écrire, remettre à zéro et faire grandir la mémoire par un `Uint8Array` sur
//!   `memory.buffer`, relu à chaque opération (un `grow` détache l'ancien tampon) ; lire
//!   et poser une globale par son nom, les `i64` par `BigInt` ; appeler `soiree_boite`
//!   et `soiree_appel` ; une instance neuve du même `WebAssembly.Module` ;
//! - [`classer`] : ce que le navigateur lève pendant un appel, ramené à un piège brut
//!   ([`moteur_commun::Piege`]). `RuntimeError` par son message (la table [`MESSAGES`],
//!   constatée dans Chrome et Firefox, qui le ramène au nom que wasmtime donne au même
//!   piège, et [`AMBIGUS`], les messages qui en désignent plusieurs), `RangeError` et
//!   `InternalError: too much recursion` en pile native ;
//! - [`charger`], qui réécrit, compile et fait vérifier un module par
//!   [`moteur_commun::verifier_chargement`], et [`FabriqueNavigateur`].
//!
//! Le déroulé d'un appel, le classement d'un piège en faute ou en panne, l'instantané
//! hors des quatre zones et l'empreinte sont dans `moteur-commun` : rien ne s'en
//! recopie ici.
//!
//! # Ce qui diffère de wasmtime
//!
//! - **Pas de garde-fou mural dans la machine.** Un appel WebAssembly ne s'interrompt
//!   pas sur son propre fil : [`MachineNavigateur`] ne rend jamais
//!   [`moteur_commun::Piege::Interrompu`]. Le garde-fou est le Worker, que la page tue
//!   (PHASE-3, § 2.3 ; lot 10a).
//! - **Pas de fil par instance.** La pile native est celle du fil qui appelle (le
//!   Worker de l'autorité, la page d'un téléphone) : la profondeur que la réécriture
//!   permet doit piquer avant elle, ce que mesure le lot 3c (M9).
//! - **Le nom d'un piège** vient du message du navigateur ; seuls les messages de la
//!   table [`MESSAGES`] prennent le nom que wasmtime donne au même piège. Dans Chrome,
//!   tous les pièges des essais sauf un : V8 dit `float unrepresentable in integer range`
//!   d'un NaN comme d'un flottant hors des bornes, que wasmtime distingue ([`AMBIGUS`]).
//!   Dans Firefox, tous sauf deux : `SpiderMonkey` dit `index out of bounds` d'un accès
//!   hors de la mémoire comme d'un `call_indirect` au-delà de la table.
//!
//! # Où cela se teste
//!
//! Ce qui ne touche pas au navigateur (la table des messages, le type des globales) se
//! teste en natif (`cargo test` dans `web/`). Le reste se joue dans un vrai navigateur :
//! l'exemple `essais-navigateur`, relié à sa page par `wasm-bindgen`, que
//! `soiree-banc machine` sert à Chrome et pilote par CDP (lot 3a).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod chargement;
mod machine;
mod pieges;
mod types;

pub use chargement::{FabriqueNavigateur, InstanceNavigateur, ModuleCharge, charger};
pub use machine::{MachineNavigateur, ModuleNavigateur};
pub use pieges::{
    AMBIGUS, ErreurJs, MESSAGES, MoteurJs, PREFIXE_NON_RAMENE, Ramene, classer, codes,
};
