//! SRTP ciphers. Salon carries no media, but DTLS may still negotiate an
//! SRTP profile, so str0m asks for them.
//!
//! Adapted from str0m-rust-crypto: every slice is checked instead of
//! indexed, and a wrong size is an error rather than a panic.

use aes::cipher::BlockEncrypt;
use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes128Gcm, Aes256Gcm, Nonce};
use ctr::cipher::{KeyIvInit, StreamCipher};
use str0m_proto::crypto::{
    AeadAes128Gcm, AeadAes128GcmCipher, AeadAes256Gcm, AeadAes256GcmCipher, Aes128CmSha1_80Cipher,
    CryptoError, SrtpProvider, SupportedAeadAes128Gcm, SupportedAeadAes256Gcm,
    SupportedAes128CmSha1_80,
};

type Aes128Ctr = ctr::Ctr128BE<aes::Aes128>;

fn too_small() -> CryptoError {
    CryptoError::Other("SRTP buffer too small".to_owned())
}

/// Copies `data` to the start of `output`.
fn put(output: &mut [u8], data: &[u8]) -> Result<(), CryptoError> {
    output
        .get_mut(..data.len())
        .ok_or_else(too_small)?
        .copy_from_slice(data);
    Ok(())
}

/// The associated data as one slice.
fn joined<'a>(aads: &[&'a [u8]], storage: &'a mut Vec<u8>) -> &'a [u8] {
    if let [only] = aads {
        only
    } else {
        *storage = aads.concat();
        storage
    }
}

#[derive(Debug)]
struct RustCryptoAes128CmSha1_80Cipher {
    key: [u8; 16],
}

impl Aes128CmSha1_80Cipher for RustCryptoAes128CmSha1_80Cipher {
    fn encrypt(
        &mut self,
        iv: &[u8; 16],
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(), CryptoError> {
        let mut cipher = Aes128Ctr::new(&self.key.into(), iv.into());
        let output = output.get_mut(..input.len()).ok_or_else(too_small)?;
        output.copy_from_slice(input);
        cipher.apply_keystream(output);
        Ok(())
    }

    fn decrypt(
        &mut self,
        iv: &[u8; 16],
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(), CryptoError> {
        // Counter mode is its own inverse.
        self.encrypt(iv, input, output)
    }
}

struct RustCryptoAeadAes128GcmCipher {
    cipher: Aes128Gcm,
}

impl std::fmt::Debug for RustCryptoAeadAes128GcmCipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RustCryptoAeadAes128GcmCipher")
    }
}

impl AeadAes128GcmCipher for RustCryptoAeadAes128GcmCipher {
    fn encrypt(
        &mut self,
        iv: &[u8; AeadAes128Gcm::IV_LEN],
        aad: &[u8],
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(), CryptoError> {
        if aad.len() < 12 {
            return Err(CryptoError::Other(
                "SRTP associated data under 12 bytes".to_owned(),
            ));
        }
        let sealed = self
            .cipher
            .encrypt(Nonce::from_slice(iv), Payload { msg: input, aad })
            .map_err(|error| CryptoError::Other(format!("AES-GCM encrypt failed: {error:?}")))?;
        put(output, &sealed)
    }

    fn decrypt(
        &mut self,
        iv: &[u8; AeadAes128Gcm::IV_LEN],
        aads: &[&[u8]],
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, CryptoError> {
        if input.len() < AeadAes128Gcm::TAG_LEN {
            return Err(CryptoError::Other(
                "SRTP packet shorter than its tag".to_owned(),
            ));
        }
        let mut storage = Vec::new();
        let aad = joined(aads, &mut storage);
        let plain = self
            .cipher
            .decrypt(Nonce::from_slice(iv), Payload { msg: input, aad })
            .map_err(|error| CryptoError::Other(format!("AES-GCM decrypt failed: {error:?}")))?;
        put(output, &plain)?;
        Ok(plain.len())
    }
}

struct RustCryptoAeadAes256GcmCipher {
    cipher: Aes256Gcm,
}

impl std::fmt::Debug for RustCryptoAeadAes256GcmCipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RustCryptoAeadAes256GcmCipher")
    }
}

impl AeadAes256GcmCipher for RustCryptoAeadAes256GcmCipher {
    fn encrypt(
        &mut self,
        iv: &[u8; AeadAes256Gcm::IV_LEN],
        aad: &[u8],
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(), CryptoError> {
        if aad.len() < 12 {
            return Err(CryptoError::Other(
                "SRTP associated data under 12 bytes".to_owned(),
            ));
        }
        let sealed = self
            .cipher
            .encrypt(Nonce::from_slice(iv), Payload { msg: input, aad })
            .map_err(|error| CryptoError::Other(format!("AES-GCM encrypt failed: {error:?}")))?;
        put(output, &sealed)
    }

    fn decrypt(
        &mut self,
        iv: &[u8; AeadAes256Gcm::IV_LEN],
        aads: &[&[u8]],
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, CryptoError> {
        if input.len() < AeadAes256Gcm::TAG_LEN {
            return Err(CryptoError::Other(
                "SRTP packet shorter than its tag".to_owned(),
            ));
        }
        let mut storage = Vec::new();
        let aad = joined(aads, &mut storage);
        let plain = self
            .cipher
            .decrypt(Nonce::from_slice(iv), Payload { msg: input, aad })
            .map_err(|error| CryptoError::Other(format!("AES-GCM decrypt failed: {error:?}")))?;
        put(output, &plain)?;
        Ok(plain.len())
    }
}

#[derive(Debug)]
struct RustCryptoSupportedAes128CmSha1_80;

impl SupportedAes128CmSha1_80 for RustCryptoSupportedAes128CmSha1_80 {
    fn create_cipher(&self, key: [u8; 16], _encrypt: bool) -> Box<dyn Aes128CmSha1_80Cipher> {
        Box::new(RustCryptoAes128CmSha1_80Cipher { key })
    }
}

#[derive(Debug)]
struct RustCryptoSupportedAeadAes128Gcm;

impl SupportedAeadAes128Gcm for RustCryptoSupportedAeadAes128Gcm {
    fn create_cipher(&self, key: [u8; 16], _encrypt: bool) -> Box<dyn AeadAes128GcmCipher> {
        Box::new(RustCryptoAeadAes128GcmCipher {
            cipher: Aes128Gcm::new(GenericArray::from_slice(&key)),
        })
    }
}

#[derive(Debug)]
struct RustCryptoSupportedAeadAes256Gcm;

impl SupportedAeadAes256Gcm for RustCryptoSupportedAeadAes256Gcm {
    fn create_cipher(&self, key: [u8; 32], _encrypt: bool) -> Box<dyn AeadAes256GcmCipher> {
        Box::new(RustCryptoAeadAes256GcmCipher {
            cipher: Aes256Gcm::new(GenericArray::from_slice(&key)),
        })
    }
}

/// One ECB round of SRTP's key derivation: the 16-byte input, then a full
/// block of PKCS#7 padding, each encrypted, into the first 32 bytes of
/// `output`. Wrong sizes leave `output` untouched.
fn ecb_round(encrypt: impl Fn(&mut aes::Block), input: &[u8], output: &mut [u8]) {
    let (Ok(block), Some(out)) = (<[u8; 16]>::try_from(input), output.get_mut(..32)) else {
        return;
    };
    let mut first = aes::Block::from(block);
    encrypt(&mut first);
    let mut padding = aes::Block::from([0x10_u8; 16]);
    encrypt(&mut padding);
    for (byte, value) in out.iter_mut().zip(first.iter().chain(padding.iter())) {
        *byte = *value;
    }
}

#[derive(Debug)]
pub(super) struct RustCryptoSrtpProvider;

impl SrtpProvider for RustCryptoSrtpProvider {
    fn aes_128_cm_sha1_80(&self) -> &'static dyn SupportedAes128CmSha1_80 {
        &RustCryptoSupportedAes128CmSha1_80
    }

    fn aead_aes_128_gcm(&self) -> &'static dyn SupportedAeadAes128Gcm {
        &RustCryptoSupportedAeadAes128Gcm
    }

    fn aead_aes_256_gcm(&self) -> &'static dyn SupportedAeadAes256Gcm {
        &RustCryptoSupportedAeadAes256Gcm
    }

    fn srtp_aes_128_ecb_round(&self, key: &[u8], input: &[u8], output: &mut [u8]) {
        if let Ok(cipher) = aes::Aes128::new_from_slice(key) {
            ecb_round(|block| cipher.encrypt_block(block), input, output);
        }
    }

    fn srtp_aes_256_ecb_round(&self, key: &[u8], input: &[u8], output: &mut [u8]) {
        if let Ok(cipher) = aes::Aes256::new_from_slice(key) {
            ecb_round(|block| cipher.encrypt_block(block), input, output);
        }
    }
}
