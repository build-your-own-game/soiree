//! Le module de `devine` tel que rustc le livre, sans la réécriture de § 3.6, dans un
//! wasmtime aux réglages par défaut : la référence de `reecrire_preserve_le_sens`.
//!
//! Il est appelé comme le moteur l'appelle (§ 3.4) : `soiree_boite`, l'appel copié
//! dans la boîte, `soiree_appel`, puis la sortie lue à l'adresse et la longueur
//! rendues.

pub struct Brut {
    store: wasmtime::Store<()>,
    memoire: wasmtime::Memory,
    boite: wasmtime::TypedFunc<(i32, i32), i32>,
    appel: wasmtime::TypedFunc<i32, i64>,
}

impl Brut {
    pub fn new(octets: &[u8]) -> Brut {
        let moteur = wasmtime::Engine::default();
        let m = wasmtime::Module::from_binary(&moteur, octets).expect("compiler le module brut");
        let mut store = wasmtime::Store::new(&moteur, ());
        let i = wasmtime::Instance::new(&mut store, &m, &[]).expect("instancier le module brut");
        Brut {
            memoire: i.get_memory(&mut store, "memory").expect("la mémoire"),
            boite: i
                .get_typed_func(&mut store, "soiree_boite")
                .expect("soiree_boite"),
            appel: i
                .get_typed_func(&mut store, "soiree_appel")
                .expect("soiree_appel"),
            store,
        }
    }

    /// Un appel encodé ; rend les octets du retour, ou le piège.
    pub fn appeler(
        &mut self,
        sorte: jeu_abi::SorteAppel,
        octets: &[u8],
    ) -> Result<Vec<u8>, String> {
        let n = i32::try_from(octets.len()).expect("un appel de moins de 2 Gio");
        let a = self
            .boite
            .call(&mut self.store, (n, sorte.code()))
            .map_err(|e| e.to_string())?;
        if a == 0 {
            return Err(String::from("la boîte refuse l'appel"));
        }
        let a = usize::try_from(a.cast_unsigned()).expect("une adresse");
        self.memoire
            .write(&mut self.store, a, octets)
            .expect("écrire dans la boîte");
        let r = self
            .appel
            .call(&mut self.store, n)
            .map_err(|e| e.to_string())?
            .cast_unsigned();
        let adresse = usize::try_from(r >> 32).expect("une adresse");
        let longueur = usize::try_from(r & 0xffff_ffff).expect("une longueur");
        self.memoire
            .data(&self.store)
            .get(adresse..adresse + longueur)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| String::from("sortie hors de la mémoire"))
    }
}
