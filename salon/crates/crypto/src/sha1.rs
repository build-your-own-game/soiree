//! HMAC-SHA1, for STUN's message integrity.

use hmac::{Hmac, Mac};
use sha1::Sha1;
use str0m_proto::crypto::Sha1HmacProvider;

type HmacSha1 = Hmac<Sha1>;

#[derive(Debug)]
pub(super) struct RustCryptoSha1HmacProvider;

impl Sha1HmacProvider for RustCryptoSha1HmacProvider {
    fn sha1_hmac(&self, key: &[u8], payloads: &[&[u8]]) -> [u8; 20] {
        // HMAC takes a key of any length: this never fails. If it ever did,
        // a zero MAC fails STUN's integrity check instead of panicking.
        let Ok(mut mac) = HmacSha1::new_from_slice(key) else {
            return [0; 20];
        };
        for payload in payloads {
            mac.update(payload);
        }
        mac.finalize().into_bytes().into()
    }
}
