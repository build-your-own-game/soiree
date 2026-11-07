//! Le son de la télé, dans le navigateur (PHASE-3, § 2.9 ; lot 10b) : le contexte audio,
//! créé **au geste seulement** (SAL-175), les quatre `Signal` synthétisés, les sons
//! déclarés décodés par `decodeAudioData`, chaque effet programmé à son instant corrigé de
//! la latence de sortie (SAL-176). Ce qui se décide (joué, perdu, périmé, déjà vu) est
//! dans [`crate::son`].

use std::collections::BTreeMap;

use jeu_abi::{Effet, Pas, Signal, Son};
use js_sys::{Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{AudioBuffer, AudioContext, AudioContextState, OscillatorType};

use crate::son::{Audio, Decision, Effets};

/// Le son de la page.
pub(super) struct HautParleur {
    contexte: Option<AudioContext>,
    /// Les effets vus, et leurs comptes.
    pub(super) effets: Effets,
    /// Les sons déclarés du jeu, tels que reçus.
    bruts: BTreeMap<u64, Vec<u8>>,
    /// Les sons décodés.
    decodes: BTreeMap<u64, AudioBuffer>,
    /// Les erreurs du son, pour le diagnostic et le banc.
    pub(super) erreurs: Vec<String>,
}

impl HautParleur {
    pub(super) const fn new() -> HautParleur {
        HautParleur {
            contexte: None,
            effets: Effets::new(),
            bruts: BTreeMap::new(),
            decodes: BTreeMap::new(),
            erreurs: Vec::new(),
        }
    }

    /// L'état du contexte, pour [`Effets::recevoir`] : seul `running` joue (SAL-174).
    pub(super) fn etat(&self) -> Audio {
        match self.contexte.as_ref().map(AudioContext::state) {
            None => Audio::Aucun,
            Some(AudioContextState::Running) => Audio::EnMarche,
            Some(_) => Audio::Suspendu,
        }
    }

    /// Le nom de l'état, pour le banc.
    pub(super) fn nom_de_l_etat(&self) -> &'static str {
        match self.etat() {
            Audio::Aucun => "aucun",
            Audio::Suspendu => "suspendu",
            Audio::EnMarche => "en-marche",
        }
    }

    /// Un son déclaré du jeu, reçu du Worker.
    pub(super) fn garder(&mut self, cle: u64, octets: Vec<u8>) {
        self.bruts.insert(cle, octets);
    }

    /// Un nouveau jeu : ses sons s'oublient, et ses effets aussi (une autre partie).
    pub(super) fn nouveau_jeu(&mut self) {
        self.bruts.clear();
        self.decodes.clear();
    }

    /// **Dans le gestionnaire d'un geste** : crée le contexte s'il n'y en a pas, et le
    /// relance (`resume`) ; rend la promesse de la relance.
    pub(super) fn au_geste(&mut self) -> Option<js_sys::Promise> {
        if self.contexte.is_none() {
            match AudioContext::new() {
                Ok(c) => self.contexte = Some(c),
                Err(e) => {
                    self.erreurs.push(format!(
                        "AudioContext : {}",
                        crate::toile::texte_d_erreur(&e)
                    ));
                    return None;
                }
            }
        }
        self.contexte.as_ref().and_then(|c| c.resume().ok())
    }

    /// Décode les sons déclarés qui ne le sont pas encore (après le geste : il faut un
    /// contexte).
    pub(super) async fn decoder(etat: &super::Etat) {
        let a_decoder: Vec<(u64, Vec<u8>, AudioContext)> = {
            let p = etat.borrow();
            let Some(c) = p.son.contexte.clone() else {
                return;
            };
            p.son
                .bruts
                .iter()
                .filter(|(k, _)| !p.son.decodes.contains_key(k))
                .map(|(k, o)| (*k, o.clone(), c.clone()))
                .collect()
        };
        for (cle, octets, c) in a_decoder {
            let tampon = Uint8Array::from(octets.as_slice()).buffer();
            let r = match c.decode_audio_data(&tampon) {
                Ok(promesse) => JsFuture::from(promesse).await,
                Err(e) => Err(e),
            };
            let mut p = etat.borrow_mut();
            match r.map(JsCast::dyn_into::<AudioBuffer>) {
                Ok(Ok(b)) => {
                    p.son.decodes.insert(cle, b);
                }
                Ok(Err(_)) => p
                    .son
                    .erreurs
                    .push(format!("son {cle:016x} : pas un tampon")),
                Err(e) => p.son.erreurs.push(format!(
                    "son {cle:016x} : {}",
                    crate::toile::texte_d_erreur(&e)
                )),
            }
        }
    }

    /// Les effets d'une scène reçue au pas `pas` : chacun décidé une fois (joué, perdu,
    /// périmé), et les joués programmés.
    pub(super) fn effets_de_la_scene(&mut self, effets: &[Effet], pas: Pas) {
        let audio = self.etat();
        for (_, d) in self.effets.recevoir(effets, pas, audio) {
            if let Decision::Jouer { son, dans_us } = d
                && let Err(e) = self.jouer(son, dans_us)
            {
                self.erreurs.push(e);
            }
        }
    }

    /// Joue un son dans `dans_us` microsecondes, moins la latence de sortie (SAL-176).
    fn jouer(&self, son: Son, dans_us: u64) -> Result<(), String> {
        let Some(c) = self.contexte.as_ref() else {
            return Err(String::from("aucun contexte audio"));
        };
        let maintenant = c.current_time();
        let latence = Reflect::get(c, &JsValue::from_str("outputLatency"))
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let mut quand =
            maintenant + f64::from(u32::try_from(dans_us).unwrap_or(u32::MAX)) / 1e6 - latence;
        if quand < maintenant {
            quand = maintenant;
        }
        let erreur = |e: JsValue| crate::toile::texte_d_erreur(&e);
        match son {
            Son::Signal(s) => {
                // Un ton bref par signal (choisi) : sa hauteur et sa durée le distinguent.
                let (hauteur, fin, duree) = match s {
                    Signal::Debut => (523.25, 783.99, 0.30),
                    Signal::Avance => (880.0, 880.0, 0.12),
                    Signal::Resultat => (659.25, 1046.5, 0.40),
                    Signal::Fin => (392.0, 261.63, 0.60),
                };
                let osc = c.create_oscillator().map_err(erreur)?;
                osc.set_type(OscillatorType::Sine);
                osc.frequency()
                    .set_value_at_time(hauteur, quand)
                    .map_err(erreur)?;
                osc.frequency()
                    .linear_ramp_to_value_at_time(fin, quand + duree)
                    .map_err(erreur)?;
                let volume = c.create_gain().map_err(erreur)?;
                volume
                    .gain()
                    .set_value_at_time(0.0, quand)
                    .map_err(erreur)?;
                volume
                    .gain()
                    .linear_ramp_to_value_at_time(0.2, quand + 0.01)
                    .map_err(erreur)?;
                volume
                    .gain()
                    .exponential_ramp_to_value_at_time(0.0001, quand + duree)
                    .map_err(erreur)?;
                osc.connect_with_audio_node(&volume).map_err(erreur)?;
                volume
                    .connect_with_audio_node(&c.destination())
                    .map_err(erreur)?;
                osc.start_with_when(quand).map_err(erreur)?;
                osc.stop_with_when(quand + duree + 0.05).map_err(erreur)?;
            }
            Son::Asset(cle) => {
                let Some(tampon) = self.decodes.get(&cle.fil()) else {
                    return Err(format!(
                        "son {:016x} : pas encore décodé (ou pas un son déclaré)",
                        cle.fil()
                    ));
                };
                let source = c.create_buffer_source().map_err(erreur)?;
                source.set_buffer(Some(tampon));
                source
                    .connect_with_audio_node(&c.destination())
                    .map_err(erreur)?;
                source.start_with_when(quand).map_err(erreur)?;
            }
        }
        Ok(())
    }
}
