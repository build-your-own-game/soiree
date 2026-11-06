//! L'heure du relais : le seul endroit où la coque la lit (SAL-04). Le cœur la reçoit en
//! paramètre, en microsecondes depuis le lancement, monotone.

use std::sync::OnceLock;
use std::time::Instant;

static DEPART: OnceLock<Instant> = OnceLock::new();

#[expect(
    clippy::disallowed_methods,
    reason = "la coque du relais lit l'heure ici seulement, et la donne au cœur"
)]
fn maintenant() -> Instant {
    Instant::now()
}

/// Les microsecondes depuis le lancement du relais.
pub fn maintenant_us() -> u64 {
    let depart = *DEPART.get_or_init(maintenant);
    u64::try_from(maintenant().saturating_duration_since(depart).as_micros()).unwrap_or(u64::MAX)
}
