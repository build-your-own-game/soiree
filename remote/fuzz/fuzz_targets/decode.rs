//! Any frame either side receives: never a panic, and what decodes encodes
//! back to bytes that decode to the same message.
#![no_main]

use libfuzzer_sys::fuzz_target;
use remote_protocol::{GameMessage, PhoneMessage, decode, encode};
use serde::{Serialize, de::DeserializeOwned};

fn stable<T: Serialize + DeserializeOwned>(frame: &[u8]) {
    if let Ok(message) = decode::<T>(frame) {
        let once = encode(&message);
        let again = decode::<T>(&once).expect("an encoded message decodes");
        assert_eq!(once, encode(&again), "encoding is not stable");
    }
}

fuzz_target!(|frame: &[u8]| {
    stable::<PhoneMessage>(frame);
    stable::<GameMessage>(frame);
});
