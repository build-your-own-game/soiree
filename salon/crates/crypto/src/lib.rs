//! WebRTC's cryptography for str0m, in Rust only.
//!
//! str0m takes its cryptography from a provider. Its pure-Rust one,
//! `str0m-rust-crypto`, still generates the DTLS certificate with `rcgen` on
//! top of `aws-lc-rs`, which is C. This crate is that provider with the
//! certificate built by RustCrypto (`p256`, `x509-cert`): salon compiles with
//! the Rust toolchain alone.
//!
//! - DTLS 1.2 and 1.3 through `dimpl` and its RustCrypto backend;
//! - SRTP ciphers (str0m may negotiate them even without media);
//! - HMAC-SHA1 for STUN, SHA-256 for certificate fingerprints.
//!
//! Adapted from `str0m-rust-crypto` 0.6.0 (Martin Algesten, MIT OR
//! Apache-2.0), to salon's lints: nothing here can panic.

// The host's: on wasm32 the crate is empty (see Cargo.toml).
#![cfg(not(target_arch = "wasm32"))]

mod certificate;
mod dtls;
mod sha1;
mod sha256;
mod srtp;

use dtls::RustCryptoDtlsProvider;
use sha1::RustCryptoSha1HmacProvider;
use sha256::RustCryptoSha256Provider;
use srtp::RustCryptoSrtpProvider;
pub use str0m_proto::crypto::CryptoProvider;

pub use certificate::self_signed;
pub use str0m_proto::crypto::dtls::DtlsCert;

/// A certificate for `RtcConfig::set_dtls_cert`. Make one at start and give
/// it to every connection: generating one per connection is slow, and str0m
/// panics if it has to generate one and cannot.
pub fn dtls_cert() -> Option<DtlsCert> {
    self_signed().map(|cert| DtlsCert {
        certificate: cert.certificate,
        private_key: cert.private_key,
    })
}

/// The provider to install once, before the first connection:
/// `salon_crypto::provider().install_process_default()`.
pub fn provider() -> CryptoProvider {
    static SRTP: RustCryptoSrtpProvider = RustCryptoSrtpProvider;
    static SHA1_HMAC: RustCryptoSha1HmacProvider = RustCryptoSha1HmacProvider;
    static SHA256: RustCryptoSha256Provider = RustCryptoSha256Provider;
    static DTLS: RustCryptoDtlsProvider = RustCryptoDtlsProvider;

    CryptoProvider {
        srtp_provider: &SRTP,
        sha1_hmac_provider: &SHA1_HMAC,
        sha256_provider: &SHA256,
        dtls_provider: &DTLS,
    }
}
