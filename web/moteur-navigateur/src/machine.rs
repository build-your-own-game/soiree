//! Les primitives du navigateur sur une instance du module réécrit
//! ([`moteur_commun::Machine`]) : `WebAssembly.Module`, `Instance`, `Memory` et
//! `Global`. Le protocole d'un appel est dans `moteur-commun`.

use std::collections::BTreeMap;
use std::rc::Rc;

use js_sys::{ArrayBuffer, Function, Object, Reflect, Uint8Array, WebAssembly};
use moteur_commun::{Bits, Machine, Piege};
use partie::{CausePanne, PanneOutil};
use wasm_bindgen::{JsCast, JsValue};

use crate::pieges::{ErreurJs, classer};
use crate::types::{TypeGlobale, globales_exportees};

/// Une page de la mémoire, en octets.
const OCTETS_PAR_PAGE: u64 = 65_536;

fn interne(quoi: impl Into<String>) -> PanneOutil {
    PanneOutil::new(CausePanne::Interne, quoi)
}

/// Le texte d'une valeur levée par le navigateur, pour un rapport.
pub(crate) fn texte(v: &JsValue) -> String {
    if let Some(e) = v.dyn_ref::<js_sys::Error>() {
        let nom: String = e.name().into();
        let message: String = e.message().into();
        return format!("{nom} : {message}");
    }
    v.as_string().unwrap_or_else(|| format!("{v:?}"))
}

/// Le piège brut d'une valeur levée par un appel du module.
pub(crate) fn piege(v: &JsValue) -> Piege {
    let Some(e) = v.dyn_ref::<js_sys::Error>() else {
        return classer(ErreurJs::Autre {
            nom: "une valeur qui n'est pas une erreur",
            message: &texte(v),
        });
    };
    let message: String = e.message().into();
    if v.is_instance_of::<WebAssembly::RuntimeError>() {
        classer(ErreurJs::Execution(&message))
    } else if v.is_instance_of::<js_sys::RangeError>() {
        classer(ErreurJs::Portee(&message))
    } else {
        let nom: String = e.name().into();
        classer(ErreurJs::Autre {
            nom: &nom,
            message: &message,
        })
    }
}

/// Un `i32` exact, depuis un nombre du navigateur.
fn entier_i32(v: &JsValue) -> Option<i32> {
    let n = v.as_f64()?;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "la conversion est vérifiée aussitôt : le nombre revient égal, ou il est refusé"
    )]
    let i = n as i32;
    #[expect(
        clippy::float_cmp,
        reason = "l'égalité exacte est voulue : le nombre doit être cet entier, sans arrondi"
    )]
    let exact = f64::from(i) == n;
    exact.then_some(i)
}

/// Un module compilé par le navigateur, et le type de ses globales exportées : ce que
/// toutes ses instances partagent.
pub struct ModuleNavigateur {
    module: WebAssembly::Module,
    globales: BTreeMap<String, TypeGlobale>,
}

impl ModuleNavigateur {
    /// Compile `octets` (`new WebAssembly.Module`, synchrone : permis dans un Worker ;
    /// sur le fil principal, Chrome le refuse au-delà de quelques Mio) et lit le type de
    /// ses globales exportées. Un refus du navigateur est une panne de l'outil : le
    /// module réécrit a déjà été validé.
    pub fn compiler(octets: &[u8]) -> Result<Rc<ModuleNavigateur>, PanneOutil> {
        let globales = globales_exportees(octets)?;
        let module = WebAssembly::Module::new(&Uint8Array::from(octets).into())
            .map_err(|e| interne(format!("le navigateur refuse le module : {}", texte(&e))))?;
        Ok(Rc::new(ModuleNavigateur { module, globales }))
    }

    /// Les noms des exports du module, dans son ordre (`WebAssembly.Module.exports`).
    pub fn exports(&self) -> Vec<String> {
        WebAssembly::Module::exports(&self.module)
            .iter()
            .filter_map(|d| Reflect::get(&d, &JsValue::from_str("name")).ok())
            .filter_map(|n| n.as_string())
            .collect()
    }
}

/// Une instance du module réécrit, dans le navigateur.
///
/// La mémoire se lit et s'écrit par un `Uint8Array` posé sur `memory.buffer`, relu à
/// chaque opération : un `memory.grow`, du module ou de la machine, détache l'ancien
/// tampon. Les globales `i64` passent par `BigInt`, exactes sur leurs 64 bits ; les
/// flottants par un nombre, exacts hors des NaN non canoniques, que la réécriture
/// remplace par le NaN canonique (§ 3.6).
///
/// Le navigateur n'interrompt pas un appel sur son propre fil : la machine ne rend
/// jamais [`Piege::Interrompu`], et le garde-fou mural est celui du Worker, que la page
/// tue (PHASE-3, § 2.3).
pub struct MachineNavigateur {
    module: Rc<ModuleNavigateur>,
    memoire: WebAssembly::Memory,
    boite: Function,
    appel: Function,
    globales: BTreeMap<String, (WebAssembly::Global, TypeGlobale)>,
}

fn export(exports: &Object, nom: &str) -> Result<JsValue, PanneOutil> {
    let v = Reflect::get(exports, &JsValue::from_str(nom))
        .map_err(|e| interne(format!("export {nom} : {}", texte(&e))))?;
    if v.is_undefined() {
        return Err(interne(format!("export {nom} absent du module réécrit")));
    }
    Ok(v)
}

impl MachineNavigateur {
    /// Une instance neuve du module (`new WebAssembly.Instance`, sans imports) : rien
    /// n'y a été appelé.
    pub fn new(module: &Rc<ModuleNavigateur>) -> Result<MachineNavigateur, PanneOutil> {
        let instance = WebAssembly::Instance::new(&module.module, &Object::new())
            .map_err(|e| PanneOutil::new(CausePanne::Instanciation, texte(&e)))?;
        let exports = instance.exports();
        let memoire = export(&exports, "memory")?
            .dyn_into::<WebAssembly::Memory>()
            .map_err(|_| interne("memory n'est pas une mémoire"))?;
        let boite = export(&exports, "soiree_boite")?
            .dyn_into::<Function>()
            .map_err(|_| interne("soiree_boite n'est pas une fonction"))?;
        let appel = export(&exports, "soiree_appel")?
            .dyn_into::<Function>()
            .map_err(|_| interne("soiree_appel n'est pas une fonction"))?;
        let mut globales = BTreeMap::new();
        for (nom, t) in &module.globales {
            let g = export(&exports, nom)?
                .dyn_into::<WebAssembly::Global>()
                .map_err(|_| interne(format!("{nom} n'est pas une globale")))?;
            globales.insert(nom.clone(), (g, *t));
        }
        Ok(MachineNavigateur {
            module: Rc::clone(module),
            memoire,
            boite,
            appel,
            globales,
        })
    }

    /// La mémoire de l'instance, pour les essais : un tampon pris avant une opération
    /// dit si elle l'a détaché.
    #[doc(hidden)]
    pub fn memoire(&self) -> &WebAssembly::Memory {
        &self.memoire
    }

    fn tampon(&self) -> Result<ArrayBuffer, PanneOutil> {
        self.memoire
            .buffer()
            .dyn_into::<ArrayBuffer>()
            .map_err(|_| interne("memory.buffer n'est pas un ArrayBuffer"))
    }

    /// `[adresse, adresse + n)` dans la mémoire, et son tampon.
    fn vue(&self, adresse: u32, n: usize) -> Result<Uint8Array, PanneOutil> {
        let tampon = self.tampon()?;
        let n = u32::try_from(n).map_err(|_| interne("lecture ou écriture hors de la mémoire"))?;
        let fin = adresse
            .checked_add(n)
            .ok_or_else(|| interne("lecture ou écriture hors de la mémoire"))?;
        // `byteLength` tient sur 32 bits tant que la mémoire est sous 4 Gio, ce que le
        // plafond de la réécriture garantit (`memoire_plafond_pages`, 16 384 pages).
        if fin > tampon.byte_length() {
            return Err(interne("lecture ou écriture hors de la mémoire"));
        }
        Ok(Uint8Array::new_with_byte_offset_and_length(
            &tampon, adresse, n,
        ))
    }

    fn global(&self, nom: &str) -> Result<&(WebAssembly::Global, TypeGlobale), PanneOutil> {
        self.globales
            .get(nom)
            .ok_or_else(|| interne(format!("export {nom} absent du module réécrit")))
    }
}

impl Machine for MachineNavigateur {
    fn neuve(&mut self) -> Result<MachineNavigateur, PanneOutil> {
        MachineNavigateur::new(&self.module)
    }

    fn pages(&mut self) -> u64 {
        self.tampon()
            .map_or(0, |t| u64::from(t.byte_length()) / OCTETS_PAR_PAGE)
    }

    fn lire(&mut self, adresse: u32, dans: &mut [u8]) -> Result<(), PanneOutil> {
        self.vue(adresse, dans.len())?.copy_to(dans);
        Ok(())
    }

    fn ecrire(&mut self, adresse: u32, octets: &[u8]) -> Result<(), PanneOutil> {
        self.vue(adresse, octets.len())?.copy_from(octets);
        Ok(())
    }

    fn remplir_de_zeros(&mut self, adresse: u32, longueur: u32) -> Result<(), PanneOutil> {
        let n = usize::try_from(longueur).map_err(|_| interne("zone hors de la mémoire"))?;
        let v = self.vue(adresse, n)?;
        let _: Uint8Array = v.fill(0, 0, longueur);
        Ok(())
    }

    /// Ne demande rien au navigateur si la mémoire a déjà `pages` pages : un
    /// `memory.grow(0)` détacherait le tampon pour rien.
    fn grandir_jusqu_a(&mut self, pages: u64) -> Result<(), PanneOutil> {
        let avant = self.pages();
        if pages <= avant {
            return Ok(());
        }
        let refus = |quoi: String| {
            PanneOutil::new(
                CausePanne::CroissanceRefusee,
                format!("restaurer : la machine refuse la mémoire ({quoi})"),
            )
        };
        let delta = u32::try_from(pages.saturating_sub(avant))
            .map_err(|_| refus(format!("{pages} pages")))?;
        // `Memory.prototype.grow` lève un `RangeError` au-delà du maximum ou faute de
        // mémoire : appelée par `Reflect`, l'erreur revient ici au lieu de traverser le
        // module.
        let grow = Reflect::get(&self.memoire, &JsValue::from_str("grow"))
            .ok()
            .and_then(|f| f.dyn_into::<Function>().ok())
            .ok_or_else(|| interne("Memory.prototype.grow absent"))?;
        grow.call1(&self.memoire, &JsValue::from(delta))
            .map(|_| ())
            .map_err(|e| refus(texte(&e)))
    }

    fn globale(&mut self, nom: &str) -> Result<Bits, PanneOutil> {
        let (g, t) = self.global(nom)?;
        let v = g.value();
        let b = match t {
            TypeGlobale::I32 => entier_i32(&v).map(Bits::I32),
            TypeGlobale::I64 => i64::try_from(v).ok().map(Bits::I64),
            #[expect(
                clippy::cast_possible_truncation,
                reason = "la valeur d'une globale f32 est un f32 élargi : le rétrécir est exact"
            )]
            TypeGlobale::F32 => v.as_f64().map(|x| Bits::F32((x as f32).to_bits())),
            TypeGlobale::F64 => v.as_f64().map(|x| Bits::F64(x.to_bits())),
        };
        b.ok_or_else(|| interne(format!("globale {nom} d'une valeur inattendue")))
    }

    fn poser_globale(&mut self, nom: &str, b: Bits) -> Result<(), PanneOutil> {
        let (g, t) = self.global(nom)?;
        let v = match (b, t) {
            (Bits::I32(x), TypeGlobale::I32) => JsValue::from(x),
            (Bits::I64(x), TypeGlobale::I64) => JsValue::from(x),
            (Bits::F32(x), TypeGlobale::F32) => JsValue::from(f32::from_bits(x)),
            (Bits::F64(x), TypeGlobale::F64) => JsValue::from(f64::from_bits(x)),
            (b, t) => {
                return Err(interne(format!(
                    "globale {nom} de type {t:?}, valeur {b:?}"
                )));
            }
        };
        // Le setter de `value` lève un `TypeError` sur une globale immuable : par
        // `Reflect`, l'erreur revient ici.
        Reflect::set(g, &JsValue::from_str("value"), &v)
            .map_err(|e| interne(format!("poser {nom} : {}", texte(&e))))
            .and_then(|fait| {
                if fait {
                    Ok(())
                } else {
                    Err(interne(format!("poser {nom} : refusé")))
                }
            })
    }

    fn boite(&mut self, taille: i32, sorte: i32) -> Result<i32, Piege> {
        let v = self
            .boite
            .call2(
                &JsValue::UNDEFINED,
                &JsValue::from(taille),
                &JsValue::from(sorte),
            )
            .map_err(|e| piege(&e))?;
        entier_i32(&v).ok_or_else(|| Piege::Moteur(format!("soiree_boite a rendu {}", texte(&v))))
    }

    fn appel(&mut self, taille: i32) -> Result<i64, Piege> {
        let v = self
            .appel
            .call1(&JsValue::UNDEFINED, &JsValue::from(taille))
            .map_err(|e| piege(&e))?;
        i64::try_from(v).map_err(|v| Piege::Moteur(format!("soiree_appel a rendu {}", texte(&v))))
    }
}
