use crate::{
    crypto::encryption_manager::encryption_interface::Encryption,
    error::{self, ContainerError},
};
use error_stack::ResultExt;
use ring::aead::{self, BoundKey};
///
/// GcmAes256
///
/// The algorithm use to perform GcmAes256 encryption/decryption. This is implemented for data
/// Vec<u8>
///
pub struct GcmAes256 {
    secret: Vec<u8>,
}

impl GcmAes256 {
    pub fn new(key: Vec<u8>) -> Self {
        Self { secret: key }
    }
}

struct NonceSequence(u128);

impl NonceSequence {
    /// Byte index at which sequence number starts in a 16-byte (128-bit) sequence.
    /// This byte index considers the big endian order used while encoding and decoding the nonce
    /// to/from a 128-bit unsigned integer.
    const SEQUENCE_NUMBER_START_INDEX: usize = 4;

    /// Generate a random nonce sequence.
    fn new() -> Result<Self, ring::error::Unspecified> {
        use ring::rand::{SecureRandom, SystemRandom};

        let rng = SystemRandom::new();

        // 96-bit sequence number, stored in a 128-bit unsigned integer in big-endian order
        let mut sequence_number = [0_u8; 128 / 8];
        rng.fill(&mut sequence_number[Self::SEQUENCE_NUMBER_START_INDEX..])?;
        let sequence_number = u128::from_be_bytes(sequence_number);

        Ok(Self(sequence_number))
    }

    /// Returns the current nonce value as bytes.
    fn current(&self) -> [u8; ring::aead::NONCE_LEN] {
        let mut nonce = [0_u8; ring::aead::NONCE_LEN];
        nonce.copy_from_slice(&self.0.to_be_bytes()[Self::SEQUENCE_NUMBER_START_INDEX..]);
        nonce
    }

    /// Constructs a nonce sequence from bytes
    fn from_bytes(bytes: [u8; ring::aead::NONCE_LEN]) -> Self {
        let mut sequence_number = [0_u8; 128 / 8];
        sequence_number[Self::SEQUENCE_NUMBER_START_INDEX..].copy_from_slice(&bytes);
        let sequence_number = u128::from_be_bytes(sequence_number);
        Self(sequence_number)
    }
}

impl ring::aead::NonceSequence for NonceSequence {
    fn advance(&mut self) -> Result<ring::aead::Nonce, ring::error::Unspecified> {
        let mut nonce = [0_u8; ring::aead::NONCE_LEN];
        nonce.copy_from_slice(&self.0.to_be_bytes()[Self::SEQUENCE_NUMBER_START_INDEX..]);

        // Increment sequence number
        self.0 = self.0.wrapping_add(1);

        // Return previous sequence number as bytes
        Ok(ring::aead::Nonce::assume_unique_for_key(nonce))
    }
}

impl Encryption<Vec<u8>, Vec<u8>> for GcmAes256 {
    type ReturnType<'b, T> = Result<T, ContainerError<error::CryptoError>>;
    fn encrypt(&self, mut input: Vec<u8>) -> Self::ReturnType<'_, Vec<u8>> {
        let nonce_sequence =
            NonceSequence::new().change_context(error::CryptoError::EncryptionError)?;
        let current_nonce = nonce_sequence.current();
        let key = aead::UnboundKey::new(&aead::AES_256_GCM, &self.secret)
            .change_context(error::CryptoError::EncryptionError)?;
        let mut key = aead::SealingKey::new(key, nonce_sequence);

        key.seal_in_place_append_tag(aead::Aad::empty(), &mut input)
            .change_context(error::CryptoError::EncryptionError)?;
        input.splice(0..0, current_nonce);

        Ok(input)
    }

    fn decrypt(&self, input: Vec<u8>) -> Self::ReturnType<'_, Vec<u8>> {
        let key = aead::UnboundKey::new(&aead::AES_256_GCM, &self.secret)
            .change_context(error::CryptoError::DecryptionError)?;

        let nonce_sequence = NonceSequence::from_bytes(
            input[..ring::aead::NONCE_LEN]
                .try_into()
                .map_err(error_stack::Report::from)
                .change_context(error::CryptoError::DecryptionError)?,
        );

        let mut key = aead::OpeningKey::new(key, nonce_sequence);
        let mut binding = input;
        let output = binding.as_mut_slice();

        let result = key
            .open_within(aead::Aad::empty(), output, ring::aead::NONCE_LEN..)
            .change_context(error::CryptoError::DecryptionError)?;
        Ok(result.to_vec())
    }
}

///
/// generates AES key to be used in the merchant accounts
///
/// # Panics
///
/// If random number generation fails
///
#[allow(clippy::unwrap_used)]
pub fn generate_aes256_key() -> [u8; 32] {
    use ring::rand::SecureRandom;

    let rng = ring::rand::SystemRandom::new();
    let mut key: [u8; 256 / 8] = [0_u8; 256 / 8];
    rng.fill(&mut key).unwrap();
    key
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn test_gcm_aes_256_encode_message() {
        let message = r#"{"type":"PAYMENT"}"#.as_bytes();
        let secret =
            hex::decode("000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f")
                .expect("Secret decoding");
        let algorithm = GcmAes256 { secret };

        let encoded_message = algorithm
            .encrypt(message.to_vec())
            .expect("Encoded message and tag");

        assert_eq!(
            algorithm.decrypt(encoded_message).expect("Decode Failed"),
            message
        );
    }

    #[test]
    fn test_gcm_aes_256_decode_message() {
        // Inputs taken from AES GCM test vectors provided by NIST
        // https://github.com/briansmith/ring/blob/95948b3977013aed16db92ae32e6b8384496a740/tests/aead_aes_256_gcm_tests.txt#L447-L452

        let right_secret =
            hex::decode("feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308")
                .expect("Secret decoding");
        let wrong_secret =
            hex::decode("feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308309")
                .expect("Secret decoding");
        let message =
            // The three parts of the message are the nonce, ciphertext and tag from the test vector
            hex::decode(
                "cafebabefacedbaddecaf888\
                 522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662898015ad\
                 b094dac5d93471bdec1a502270e3cc6c"
            ).expect("Message decoding");

        let algorithm1 = GcmAes256 {
            secret: right_secret,
        };
        let algorithm2 = GcmAes256 {
            secret: wrong_secret,
        };

        let decoded = algorithm1
            .decrypt(message.clone())
            .expect("Decoded message");

        assert_eq!(
            decoded,
            hex::decode("d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a721c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b391aafd255")
                .expect("Decoded plaintext message")
        );

        let err_decoded = algorithm2.decrypt(message);

        assert!(err_decoded.is_err());
    }
}
