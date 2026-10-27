//! The DTLS certificate, built with RustCrypto alone.
//!
//! WebRTC peers do not check a chain: each side compares the certificate
//! with the fingerprint the other announced in its SDP. A self-signed
//! ECDSA P-256 certificate is what browsers and str0m expect, with two
//! details the browsers enforce:
//!
//! - it is an end-entity certificate whose key may sign (`digitalSignature`):
//!   the host signs its DTLS key exchange with it, and BoringSSL (Chrome,
//!   Safari's WebRTC) silently drops a handshake whose certificate only
//!   allows signing certificates. Found on 27/10/2026: with x509-cert's
//!   "root" profile, Chrome resent its ClientHello forever, while str0m
//!   against str0m connected fine;
//! - its serial number is random: Firefox refuses two certificates with the
//!   same one (str0m #517).

use std::str::FromStr;
use std::time::Duration;

use p256::ecdsa::{DerSignature, SigningKey};
use p256::pkcs8::EncodePrivateKey;
use rand_core::{OsRng, RngCore};
use x509_cert::builder::{Builder, CertificateBuilder, Profile};
use x509_cert::der::Encode;
use x509_cert::name::Name;
use x509_cert::serial_number::SerialNumber;
use x509_cert::spki::SubjectPublicKeyInfoOwned;
use x509_cert::time::Validity;

/// How long the certificate is valid: a party, with a wide margin.
const VALIDITY: Duration = Duration::from_hours(365 * 24);

/// A certificate and its private key, both DER: the certificate in X.509,
/// the key in PKCS#8, as `dimpl` reads them.
pub struct SelfSigned {
    pub certificate: Vec<u8>,
    pub private_key: Vec<u8>,
}

/// A fresh self-signed certificate, or `None` if the system gives no secure
/// random numbers or the encoding fails.
pub fn self_signed() -> Option<SelfSigned> {
    let key = SigningKey::random(&mut OsRng);
    let mut serial = [0_u8; 16];
    OsRng.try_fill_bytes(&mut serial).ok()?;
    // A positive integer: the first bit clear, and never zero.
    if let Some(first) = serial.first_mut() {
        *first = (*first & 0x7f) | 0x01;
    }
    let public = SubjectPublicKeyInfoOwned::from_key(*key.verifying_key()).ok()?;
    let name = Name::from_str("CN=salon").ok()?;
    // An end entity, issued by itself: its key may sign, not certify.
    let profile = Profile::Leaf {
        issuer: name.clone(),
        enable_key_agreement: false,
        enable_key_encipherment: false,
    };
    let builder = CertificateBuilder::new(
        profile,
        SerialNumber::new(&serial).ok()?,
        Validity::from_now(VALIDITY).ok()?,
        name,
        public,
        &key,
    )
    .ok()?;
    let certificate = builder.build::<DerSignature>().ok()?.to_der().ok()?;
    let private_key = key.to_pkcs8_der().ok()?.as_bytes().to_vec();
    Some(SelfSigned {
        certificate,
        private_key,
    })
}
