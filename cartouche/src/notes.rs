//! Noter un cas qui n'aurait pas dû arriver, sans arrêter la soirée.

use core::panic::Location;

use spin::Mutex;

/// Une note laissée par [`oups!`](crate::oups).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Note {
    /// Ce qui n'aurait pas dû arriver.
    pub what: &'static str,
    /// Le fichier et la ligne qui l'ont noté.
    pub file: &'static str,
    pub line: u32,
    /// Combien de notes ont été laissées depuis la dernière lecture.
    pub count: u32,
}

/// La dernière note, et le compte depuis la dernière lecture.
static LAST: Mutex<Option<Note>> = Mutex::new(None);

/// Note un cas qui n'aurait pas dû arriver. Passe par [`oups!`](crate::oups).
///
/// Dans les tests et sur le banc, la note arrête tout. Ailleurs, la partie
/// continue et la console lit la note par [`take_notes`].
#[track_caller]
pub fn note(what: &'static str) {
    let place = Location::caller();
    if cfg!(any(debug_assertions, feature = "bench")) {
        stop(what, place);
    }
    let mut last = LAST.lock();
    let count = last.map_or(0, |note| note.count).saturating_add(1);
    *last = Some(Note {
        what,
        file: place.file(),
        line: place.line(),
        count,
    });
}

#[expect(
    clippy::panic,
    reason = "dans les tests et sur le banc, un cas impossible doit se voir : la console \
              rattrape l'arrêt et dit quelle partie rejouer"
)]
fn stop(what: &'static str, place: &Location<'_>) -> ! {
    panic!("oups : {what} ({}:{})", place.file(), place.line())
}

/// La dernière note laissée depuis la dernière lecture, s'il y en a une.
pub fn take_notes() -> Option<Note> {
    LAST.lock().take()
}
