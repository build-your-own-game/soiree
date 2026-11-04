//! Le module WebAssembly de `devine` (INTERFACE.md, § 3.1 et § 8.9, étape 2) : il se
//! compile pour `wasm32-unknown-unknown`, n'importe rien, n'exporte que la mémoire et
//! les deux fonctions de § 3.3, n'emploie que les fonctionnalités acceptées, et rend,
//! appel pour appel, les mêmes octets que la copie native (E9, en petit).
//!
//! Le module est compilé par ce test, avec le profil `module`, dans un dossier de
//! compilation à part (`target/verif-module`) : rien n'attend le verrou de `cargo test`.
//! Il tourne dans wasmtime tel quel, sans la réécriture de § 3.6 (carburant, profondeur,
//! NaN, croissance), qui est l'affaire de `reecriture` et `moteur-wasmtime`.

#![expect(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::let_underscore_untyped,
    clippy::panic,
    clippy::unwrap_used,
    clippy::wildcard_enum_match_arm,
    reason = "code de test : un échec s'y lit comme l'échec du test qui l'appelle"
)]

mod commun;

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use commun::{A, B, Pilote, Porte};
use devine::{Devine, GesteTel};
use jeu::Jeu;
use jeu_abi::{Appel, Bonjour, RegardFil, Retour, SorteAppel, StyleFil};
use wasmparser::{Parser, Payload, Validator, WasmFeatures};

/// Compile le module une fois pour tous les tests de ce fichier, et rend ses octets.
fn module() -> &'static [u8] {
    static OCTETS: OnceLock<Vec<u8>> = OnceLock::new();
    OCTETS.get_or_init(|| {
        let racine = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let cible = racine.join("target/verif-module");
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));
        let statut = Command::new(cargo)
            .current_dir(&racine)
            .env("CARGO_TARGET_DIR", &cible)
            .args([
                "build",
                "-p",
                "devine-module",
                "--target",
                "wasm32-unknown-unknown",
                "--profile",
                "module",
            ])
            .status()
            .expect("lancer cargo");
        assert!(statut.success(), "la compilation du module a échoué");
        std::fs::read(cible.join("wasm32-unknown-unknown/module/devine_module.wasm"))
            .expect("lire le module")
    })
}

/// Les fonctionnalités de § 3.1 : WebAssembly 1.0, globales mutables, extension de signe,
/// conversions saturantes, valeurs multiples, mémoire en bloc, types référence. Ni SIMD,
/// ni fils, ni mémoire 64 bits, ni plusieurs mémoires, ni exceptions, ni appels
/// terminaux, ni GC.
fn acceptees() -> WasmFeatures {
    WasmFeatures::WASM1
        | WasmFeatures::SIGN_EXTENSION
        | WasmFeatures::SATURATING_FLOAT_TO_INT
        | WasmFeatures::MULTI_VALUE
        | WasmFeatures::BULK_MEMORY
        | WasmFeatures::REFERENCE_TYPES
}

/// Le début des données : la pile est en dessous (§ 3.1, `--stack-first`).
fn debut_des_donnees(octets: &[u8]) -> u64 {
    let mut debut = u64::MAX;
    for p in Parser::new(0).parse_all(octets) {
        if let Payload::DataSection(s) = p.unwrap() {
            for d in s {
                if let wasmparser::DataKind::Active { offset_expr, .. } = d.unwrap().kind {
                    let mut r = offset_expr.get_operators_reader();
                    if let wasmparser::Operator::I32Const { value } = r.read().unwrap() {
                        debut = debut.min(u64::from(value.cast_unsigned()));
                    }
                }
            }
        }
    }
    debut
}

/// Le module n'importe rien, et n'exporte que `memory`, `soiree_boite` et `soiree_appel`
/// (§ 3.1, § 3.3, D5) ; une seule mémoire, 32 bits, non partagée, avec un maximum égal à
/// `limites.memoire` ; pas de fonction de départ ; la pile en bas ; et rien hors des
/// fonctionnalités acceptées.
#[test]
fn le_module_n_importe_rien() {
    let octets = module();
    Validator::new_with_features(acceptees())
        .validate_all(octets)
        .expect("le module emploie une fonctionnalité refusée par § 3.1");
    let mut imports = 0;
    let mut exports = Vec::new();
    let mut memoires = Vec::new();
    let mut depart = false;
    for p in Parser::new(0).parse_all(octets) {
        match p.unwrap() {
            Payload::ImportSection(s) => imports += s.count(),
            Payload::ExportSection(s) => {
                for e in s {
                    let e = e.unwrap();
                    exports.push((e.name.to_owned(), e.kind));
                }
            }
            Payload::MemorySection(s) => {
                for m in s {
                    memoires.push(m.unwrap());
                }
            }
            Payload::StartSection { .. } => depart = true,
            _ => {}
        }
    }
    assert_eq!(imports, 0, "le module importe quelque chose");
    assert!(!depart, "le module a une fonction de départ");
    exports.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        exports,
        [
            ("memory".to_owned(), wasmparser::ExternalKind::Memory),
            ("soiree_appel".to_owned(), wasmparser::ExternalKind::Func),
            ("soiree_boite".to_owned(), wasmparser::ExternalKind::Func),
        ]
    );
    assert_eq!(memoires.len(), 1);
    let m = memoires[0];
    assert!(!m.memory64 && !m.shared);
    assert_eq!(
        m.maximum,
        Some(u64::from(Devine::DECLARATION.limites.memoire))
    );
    assert!(m.initial <= u64::from(Devine::DECLARATION.limites.memoire));
    assert!(
        debut_des_donnees(octets) > 0,
        "la pile doit être sous les données"
    );
}

/// Le module dans wasmtime, appelé comme le ferait le moteur (§ 3.4), sans réécriture.
struct Module {
    store: wasmtime::Store<()>,
    memoire: wasmtime::Memory,
    boite: wasmtime::TypedFunc<(i32, i32), i32>,
    appel: wasmtime::TypedFunc<i32, i64>,
}

impl Module {
    fn new() -> Module {
        let moteur = wasmtime::Engine::default();
        let m = wasmtime::Module::from_binary(&moteur, module()).unwrap();
        let mut store = wasmtime::Store::new(&moteur, ());
        let i = wasmtime::Instance::new(&mut store, &m, &[]).unwrap();
        Module {
            memoire: i.get_memory(&mut store, "memory").unwrap(),
            boite: i.get_typed_func(&mut store, "soiree_boite").unwrap(),
            appel: i.get_typed_func(&mut store, "soiree_appel").unwrap(),
            store,
        }
    }
}

impl Porte for Module {
    fn appeler(&mut self, sorte: SorteAppel, octets: &[u8]) -> Result<Vec<u8>, String> {
        let n = i32::try_from(octets.len()).unwrap();
        let a = self
            .boite
            .call(&mut self.store, (n, sorte.code()))
            .map_err(|e| e.to_string())?;
        if a == 0 {
            return Err(String::from("la boîte refuse l'appel"));
        }
        let a = usize::try_from(a.cast_unsigned()).unwrap();
        self.memoire.write(&mut self.store, a, octets).unwrap();
        let r = self
            .appel
            .call(&mut self.store, n)
            .map_err(|e| e.to_string())?
            .cast_unsigned();
        let adresse = usize::try_from(r >> 32).unwrap();
        let longueur = usize::try_from(r & 0xffff_ffff).unwrap();
        Ok(self.memoire.data(&self.store)[adresse..adresse + longueur].to_vec())
    }
}

fn bonjour<P: Porte>(p: &mut Pilote<P>) -> Bonjour {
    p.avec(&Appel::Bonjour, |r| match r {
        Retour::Bonjour(b) => b,
        autre => panic!("{autre:?}"),
    })
}

/// La même soirée, appel pour appel, sur le module et sur la copie native : mêmes
/// octets rendus, même forme canonique (E9, en petit). Seul `Bonjour` diffère : il
/// nomme les zones du module, que la copie native n'a pas.
#[test]
fn module_et_copie_native_rendent_les_memes_octets() {
    let mut m = Pilote::new(Module::new());
    let mut n = Pilote::new(jeu_natif::InstanceNative::<Devine>::new());
    let (bm, bn) = (bonjour(&mut m), bonjour(&mut n));
    assert_eq!(
        (bm.abi, bm.formes, bm.version_jeu),
        (bn.abi, bn.formes, bn.version_jeu)
    );
    assert_ne!(bm.brouillon.1, 0, "le module annonce son brouillon");
    let fin = 10 + jeu::prelude::Duree::s(20).en_pas();
    m.creer([42; 32], None);
    n.creer([42; 32], None);
    // Chaque coup sur les deux, puis les mêmes lectures, comparées octet pour octet.
    let lectures = |p: &mut dyn FnMut(&Appel<'_>) -> Vec<u8>| -> Vec<Vec<u8>> {
        vec![
            p(&Appel::Regarder {
                regards: vec![
                    RegardFil::Ecran,
                    RegardFil::Appareil(A),
                    RegardFil::Appareil(B),
                ],
            }),
            p(&Appel::Etat { connues: vec![] }),
            p(&Appel::Canonique { morceau: 0 }),
            p(&Appel::Projeter {
                regard: RegardFil::Appareil(A),
                morceau: 0,
            }),
            p(&Appel::Resumer),
            p(&Appel::Verifier),
            p(&Appel::Garder),
            p(&Appel::Bot {
                regard: RegardFil::Appareil(B),
                style: StyleFil::Hostile,
                graine: 9,
                depuis_vue: 100,
            }),
            p(&Appel::Bot {
                regard: RegardFil::Ecran,
                style: StyleFil::Sage,
                graine: 9,
                depuis_vue: 500,
            }),
            p(&Appel::Decrire),
        ]
    };
    let comparer =
        |m: &mut Pilote<Module>, n: &mut Pilote<jeu_natif::InstanceNative<Devine>>, etape: &str| {
            let lm = lectures(&mut |a| m.brut(a).unwrap());
            let ln = lectures(&mut |a| n.brut(a).unwrap());
            for (i, (x, y)) in lm.iter().zip(&ln).enumerate() {
                assert_eq!(x, y, "lecture {i} après {etape}");
            }
        };
    comparer(&mut m, &mut n, "la création");
    macro_rules! les_deux {
        ($etape:literal, |$p:ident| $corps:expr) => {{
            let dm = {
                let $p = &mut m;
                $corps
            };
            let dn = {
                let $p = &mut n;
                $corps
            };
            assert_eq!(dm, dn, "{}", $etape);
            comparer(&mut m, &mut n, $etape);
        }};
    }
    les_deux!("rejoindre", |p| p.rejoindre(A, "Léa", 1));
    les_deux!("un nom hostile", |p| p.rejoindre(
        B,
        "Z\u{0338}\u{0322}o\u{200B}é\u{202E}🇫🇷👩\u{200D}👧",
        2
    ));
    les_deux!("lancer", |p| p.lancer(10, 9));
    les_deux!("le curseur", |p| p.curseur(A, 4, 11));
    les_deux!("proposer", |p| p.geste(A, &GesteTel::Proposer, 12, 11));
    les_deux!("proposer deux fois", |p| p.geste(
        A,
        &GesteTel::Proposer,
        13,
        12
    ));
    les_deux!("le temps trop tôt", |p| p.temps(fin - 1));
    les_deux!("la révélation", |p| p.temps(fin));
    les_deux!("une seconde manche", |p| p.lancer(fin + 1, fin + 1));
}

/// Une lecture ne touche ni les statiques ni le tas du module : hors de la pile, de la
/// boîte, du brouillon et de la zone de panique, la mémoire est la même avant et après
/// (§ 3.2, D4, E7 en petit).
#[test]
fn une_lecture_du_module_ne_touche_pas_l_etat() {
    let mut m = Pilote::new(Module::new());
    let b = bonjour(&mut m);
    m.creer([5; 32], None);
    m.rejoindre(A, "Léa", 1);
    m.lancer(2, 2);
    let pile = usize::try_from(debut_des_donnees(module())).unwrap();
    let exclues = [b.boite, b.brouillon, b.panique].map(|(a, n)| {
        let a = usize::try_from(a).unwrap();
        a..a + usize::try_from(n).unwrap()
    });
    let etat = |m: &mut Pilote<Module>| -> Vec<u8> {
        let d = m.porte.memoire.data(&m.porte.store);
        d.iter()
            .enumerate()
            .skip(pile)
            .filter(|(i, _)| !exclues.iter().any(|z| z.contains(i)))
            .map(|(_, o)| *o)
            .collect()
    };
    let avant = etat(&mut m);
    for a in [
        Appel::Regarder {
            regards: vec![
                RegardFil::Ecran,
                RegardFil::Appareil(A),
                RegardFil::Appareil(B),
            ],
        },
        Appel::Etat { connues: vec![] },
        Appel::Canonique { morceau: 0 },
        Appel::Projeter {
            regard: RegardFil::Ecran,
            morceau: 0,
        },
        Appel::Bot {
            regard: RegardFil::Appareil(A),
            style: StyleFil::Hostile,
            graine: 1,
            depuis_vue: 100,
        },
        Appel::Verifier,
        Appel::Garder,
        Appel::Memoire,
    ] {
        let _ = m.brut(&a).unwrap();
        assert_eq!(etat(&mut m), avant, "{a:?} a changé l'état du module");
    }
    // Et une écriture le change.
    m.curseur(A, 3, 3);
    assert_ne!(etat(&mut m), avant);
}

/// Une panique écrit son message dans la zone de panique, puis le module s'arrête
/// (`unreachable`) : ici, un brouillon épuisé par une lecture trop grande (§ 2.14, § 4.6).
#[test]
fn une_panique_ecrit_dans_sa_zone() {
    let mut m = Pilote::new(Module::new());
    let b = bonjour(&mut m);
    m.creer([5; 32], None);
    let trop = Appel::Regarder {
        regards: vec![RegardFil::Ecran; 30_000],
    };
    let echec = m.brut(&trop).unwrap_err();
    assert!(
        echec.contains("rust_begin_unwind"),
        "le gestionnaire de panique : {echec}"
    );
    let (a, n) = b.panique;
    let zone = &m.porte.memoire.data(&m.porte.store)[usize::try_from(a).unwrap()..]
        [..usize::try_from(n).unwrap()];
    let message = jeu::colle::lire_panique(zone).expect("la zone de panique est vide");
    assert!(message.contains("memory allocation"), "{message}");
}
