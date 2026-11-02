//! Any bytes a transport delivers: to a stranger's link, to a welcomed link,
//! to a phone. Never a panic, never an oversized packet in reply. Whatever
//! `codec::decode` reads, the journal types included, writes back to bytes
//! that read the same, and a name stays a name.
#![no_main]

use libfuzzer_sys::fuzz_target;
use salon_core::journal::{Chunk, Diagnostic, PanicReport, Stored, Submission};
use salon_core::{Caps, LinkId, Ms, Name, Qr, codec};
use serde::{Serialize, de::DeserializeOwned};

mod common;

use common::{HONEST, drain_room, joined, room};
use salon_core::Token;

const NOW: Ms = Ms(1_000);

fn stable<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(bytes: &[u8]) {
    if let Ok(message) = codec::decode::<T>(bytes) {
        let once = postcard::to_allocvec(&message).expect("a decoded message encodes");
        let again = codec::decode::<T>(&once).expect("an encoded message decodes");
        assert_eq!(message, again);
    }
}

fuzz_target!(|bytes: &[u8]| {
    let stranger = LinkId(2);
    let mut host = room(NOW);
    host.opened(NOW, stranger, Caps::DATAGRAM);
    host.received(NOW, stranger, bytes);
    let _ = drain_room(&mut host, NOW);

    let (mut host, mut client, _) = joined(NOW, Token([1; 16]));
    host.received(NOW, HONEST, bytes);
    let _ = drain_room(&mut host, NOW);
    client.received(NOW, bytes);
    while client.poll(NOW).is_some() {}

    stable::<Stored<u32, String>>(bytes);
    stable::<Submission<u32, String>>(bytes);
    stable::<Diagnostic>(bytes);
    stable::<PanicReport>(bytes);
    stable::<Chunk>(bytes);
    stable::<Qr>(bytes);

    if let Some(name) = Name::new(&String::from_utf8_lossy(bytes)) {
        let name = name.as_str();
        assert!(!name.is_empty() && name.chars().count() <= Name::MAX_CHARS);
        assert!(!name.chars().any(char::is_control));
        assert_eq!(name, name.trim());
    }
});
