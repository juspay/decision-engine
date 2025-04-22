use masking::{PeekInterface, Secret};
use ring::hmac;

use crate::{crypto::hash_manager::hash_interface::Encode, error};

///
/// Type providing encoding functional to perform hashing
///
pub struct Sha512;

impl Encode<Vec<u8>, Vec<u8>> for Sha512 {
    type ReturnType<T> = Result<T, error::ContainerError<error::CryptoError>>;

    fn encode(&self, input: Vec<u8>) -> Self::ReturnType<Vec<u8>> {
        let digest = ring::digest::digest(&ring::digest::SHA512, &input);
        Ok(digest.as_ref().to_vec())
    }
}

///
/// Type providing encoding functional to perform HMAC-SHA512 hashing
///
/// # Example
///
///```
/// use open_router::crypto::hash_manager::managers::sha::HmacSha512;
/// use open_router::crypto::hash_manager::hash_interface::Encode;
///
/// let data = "Hello, World!";
/// let key = "key";
/// let algo = HmacSha512::<1>::new(key.as_bytes().to_vec().into());
/// let hash = algo.encode(data.as_bytes().to_vec().into()).unwrap();
///
/// ```
///
/// This will not compile if `N` is less than or equal to 0.
///
/// ```compile_fail
///
/// use open_router::crypto::hash_manager::managers::sha::HmacSha512;
/// use open_router::crypto::hash_manager::hash_interface::Encode;
///
/// let key = "key";
/// let algo = HmacSha512::<0>::new(key.as_bytes().to_vec().into());
///
///
/// ```
///
///
pub struct HmacSha512<const N: usize = 1>(masking::Secret<Vec<u8>>);

impl<const N: usize> HmacSha512<N> {
    pub fn new(key: masking::Secret<Vec<u8>>) -> Self {
        #[allow(clippy::let_unit_value)]
        let _ = <Self as AssertGt0>::VALID;

        Self(key)
    }
}

impl<const N: usize> std::fmt::Display for HmacSha512<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HmacSha512<{}>", N)
    }
}

impl<const N: usize> Encode<Secret<Vec<u8>>, Secret<Vec<u8>>> for HmacSha512<N> {
    type ReturnType<T> = Result<T, error::ContainerError<error::CryptoError>>;

    fn encode(&self, input: Secret<Vec<u8>>) -> Self::ReturnType<Secret<Vec<u8>>> {
        let key = hmac::Key::new(ring::hmac::HMAC_SHA512, self.0.peek());
        let first = hmac::sign(&key, input.peek());

        let signature = (0..=(N - 1)).fold(first, |input, _| hmac::sign(&key, input.as_ref()));

        Ok(signature.as_ref().to_vec().into())
    }
}

trait AssertGt0 {
    const VALID: ();
}

impl<const N: usize> AssertGt0 for HmacSha512<N> {
    const VALID: () = assert!(N > 0);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    //!
    //! Testing HMAC-SHA512 encoding consists of 3 variables.
    //! 1. The input data
    //! 2. The Key
    //! 3. The `N` value
    //!

    use super::*;

    #[test]
    fn test_input_data_equal() {
        let data1 = "Hello, World!";
        let data2 = "Hello, World!";
        let key = "key";

        let algo = HmacSha512::<1>::new(key.as_bytes().to_vec().into());

        let hash1 = algo.encode(data1.as_bytes().to_vec().into()).unwrap();
        let hash2 = algo.encode(data2.as_bytes().to_vec().into()).unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_input_data_not_equal() {
        let data1 = "Hello, World!";
        let data2 = "Hello, world";
        let key = "key";

        let algo = HmacSha512::<1>::new(key.as_bytes().to_vec().into());

        let hash1 = algo.encode(data1.as_bytes().to_vec().into()).unwrap();
        let hash2 = algo.encode(data2.as_bytes().to_vec().into()).unwrap();

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_key_not_equal() {
        let data = "Hello, World!";
        let key1 = "key1";
        let key2 = "key2";

        let algo1 = HmacSha512::<1>::new(key1.as_bytes().to_vec().into());
        let algo2 = HmacSha512::<1>::new(key2.as_bytes().to_vec().into());

        let hash1 = algo1.encode(data.as_bytes().to_vec().into()).unwrap();
        let hash2 = algo2.encode(data.as_bytes().to_vec().into()).unwrap();

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_key_equal() {
        let data = "Hello, World!";
        let key1 = "key";
        let key2 = "key";

        let algo1 = HmacSha512::<1>::new(key1.as_bytes().to_vec().into());
        let algo2 = HmacSha512::<1>::new(key2.as_bytes().to_vec().into());

        let hash1 = algo1.encode(data.as_bytes().to_vec().into()).unwrap();
        let hash2 = algo2.encode(data.as_bytes().to_vec().into()).unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_n_equal() {
        let data = "Hello, World!";
        let key = "key";

        let algo1 = HmacSha512::<10>::new(key.as_bytes().to_vec().into());
        let algo2 = HmacSha512::<10>::new(key.as_bytes().to_vec().into());

        let hash1 = algo1.encode(data.as_bytes().to_vec().into()).unwrap();
        let hash2 = algo2.encode(data.as_bytes().to_vec().into()).unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_n_not_equal() {
        let data = "Hello, World!";
        let key = "key";

        let algo1 = HmacSha512::<10>::new(key.as_bytes().to_vec().into());
        let algo2 = HmacSha512::<20>::new(key.as_bytes().to_vec().into());

        let hash1 = algo1.encode(data.as_bytes().to_vec().into()).unwrap();
        let hash2 = algo2.encode(data.as_bytes().to_vec().into()).unwrap();

        assert_ne!(hash1, hash2);
    }
}
