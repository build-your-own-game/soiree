//! Le garde-fou mural (§ 4.6) : une époque de wasmtime qui avance seule.
//!
//! Un fil fait avancer l'époque du moteur tous les [`TIC_MS`] millisecondes. Avant
//! chaque appel, l'instance pose son échéance à un nombre de tics : un appel qui la
//! dépasse est interrompu par wasmtime (`Trap::Interrupt`). C'est une panne de
//! l'outil, jamais une faute du jeu : le carburant aurait dû l'arrêter avant (HIS-06,
//! OUT-11).
//!
//! Le fil dort : il ne lit aucune horloge. La durée réelle d'un tic dépend de la
//! machine ; le garde-fou ne mesure rien, il borne.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use partie::{CausePanne, PanneOutil};
use wasmtime::Engine;

/// La durée d'un tic, en millisecondes.
pub const TIC_MS: u32 = 2;

/// Le fil qui fait avancer l'époque. Il s'arrête quand la dernière référence tombe.
pub(crate) struct Horloge {
    arret: Arc<AtomicBool>,
    fil: Option<JoinHandle<()>>,
}

impl Horloge {
    pub(crate) fn lancer(engine: Engine) -> Result<Horloge, PanneOutil> {
        let arret = Arc::new(AtomicBool::new(false));
        let a = Arc::clone(&arret);
        let fil = thread::Builder::new()
            .name(String::from("soiree-garde-fou"))
            .spawn(move || {
                while !a.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(u64::from(TIC_MS)));
                    engine.increment_epoch();
                }
            })
            .map_err(|e| PanneOutil::new(CausePanne::Interne, format!("garde-fou : {e}")))?;
        Ok(Horloge {
            arret,
            fil: Some(fil),
        })
    }
}

impl Drop for Horloge {
    fn drop(&mut self) {
        self.arret.store(true, Ordering::Relaxed);
        if let Some(fil) = self.fil.take() {
            // Le fil s'arrête au plus un tic plus tard ; une panique du fil ne change
            // rien à ce qui reste à libérer.
            let _: thread::Result<()> = fil.join();
        }
    }
}

/// L'échéance d'un appel, en tics : au moins la durée voulue, plus le tic en cours.
pub(crate) fn tics(garde_fou_ms: u32) -> u64 {
    u64::from(garde_fou_ms.div_ceil(TIC_MS)).saturating_add(1)
}
