//! SHA-256, for certificate fingerprints.

use sha2::{Digest, Sha256};
use str0m_proto::crypto::Sha256Provider;

#[derive(Debug)]
pub(super) struct RustCryptoSha256Provider;

impl Sha256Provider for RustCryptoSha256Provider {
    fn sha256(&self, data: &[u8]) -> [u8; 32] {
        Sha256::digest(data).into()
    }
}
