//! L'heure du banc : le seul endroit où il la lit.
//!
//! Le banc attend des navigateurs et des processus : ses délais sont muraux, et ne
//! touchent à aucune partie. Rien de ce qu'il juge ne dépend de l'heure.

use std::time::{Duration, Instant};

/// Maintenant.
#[expect(
    clippy::disallowed_methods,
    reason = "le banc attend un navigateur ou un processus : un délai mural, hors de toute partie"
)]
pub(crate) fn maintenant() -> Instant {
    Instant::now()
}

/// Ce qui reste avant `echeance`, zéro si elle est passée.
pub(crate) fn reste(echeance: Instant) -> Duration {
    echeance.saturating_duration_since(maintenant())
}

/// L'heure murale, en microsecondes depuis 1970 : l'heure que la page d'un écran donne à
/// son Worker (`performance.timeOrigin + performance.now()`), pour l'autorité native de
/// `soiree-native`. Lue une fois, puis avancée par l'horloge monotone.
pub(crate) fn heure_us() -> u64 {
    use std::sync::OnceLock;
    static ANCRE: OnceLock<(Instant, u64)> = OnceLock::new();
    let (depart, mur) = *ANCRE.get_or_init(|| {
        #[expect(
            clippy::disallowed_methods,
            reason = "l'heure murale de l'écran simulé du banc, lue une fois"
        )]
        let mur = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_micros()).unwrap_or(u64::MAX));
        (maintenant(), mur)
    });
    mur.saturating_add(
        u64::try_from(maintenant().saturating_duration_since(depart).as_micros())
            .unwrap_or(u64::MAX),
    )
}
