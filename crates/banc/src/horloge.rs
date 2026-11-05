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
