//! La référence des essais de `Machine` dans le navigateur : ce que wasmtime rend pour
//! chaque piège de `web/moteur-navigateur/essais/machine.wat`, écrit dans `pieges.txt`
//! (PHASE-3, lot 3a).
//!
//! L'exemple `essais-navigateur` juge le classement du navigateur contre ce fichier ;
//! ce test vérifie que le fichier dit vrai de wasmtime, le moteur de l'épreuve. Le piège
//! brut se lit ici comme `moteur-wasmtime` le lit (`src/machine.rs`, `piege`) : une
//! seconde lecture, écrite à part, de la même règle (SAL-90).

#![cfg(test)]

use wasmtime::{Engine, Instance, Module, Store, Trap};

const MACHINE: &str = include_str!("../../../web/moteur-navigateur/essais/machine.wat");
const PIEGES: &str = include_str!("../../../web/moteur-navigateur/essais/pieges.txt");

/// Le piège de wasmtime, dans la forme de `pieges.txt`.
fn brut(e: &wasmtime::Error) -> String {
    match e.downcast_ref::<Trap>() {
        Some(Trap::MemoryOutOfBounds) => String::from("hors-memoire"),
        Some(Trap::StackOverflow) => String::from("pile-native"),
        Some(t) => format!("code {t}"),
        None => format!("pas un piège : {e}"),
    }
}

#[test]
fn chaque_piege_des_essais_est_celui_de_wasmtime() {
    let engine = Engine::default();
    let module = Module::new(&engine, wat::parse_str(MACHINE).unwrap()).unwrap();
    let mut vus = 0;
    for ligne in PIEGES
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let mut champs = ligne.splitn(3, ' ');
        let k: i32 = champs.next().unwrap().parse().unwrap();
        let _nom = champs.next().unwrap();
        let attendu = champs.next().unwrap();
        let mut store = Store::new(&engine, ());
        let instance = Instance::new(&mut store, &module, &[]).unwrap();
        let appel = instance
            .get_typed_func::<i32, i64>(&mut store, "soiree_appel")
            .unwrap();
        let e = appel
            .call(&mut store, k)
            .expect_err(&format!("le scénario {k} devait piéger"));
        assert_eq!(brut(&e), attendu, "scénario {k}");
        // L'instance sert encore, comme dans le navigateur.
        assert_eq!(appel.call(&mut store, 16).unwrap(), 0x0123_4567_89ab_cdef);
        vus += 1;
    }
    assert_eq!(vus, 11);
}
