pub mod encryption_manager;
pub mod hash_manager;
pub mod keymanager;
pub mod secrets_manager;

pub mod consts {
    #[cfg(any(feature = "external_key_manager", feature = "kms-aws"))]
    /// General purpose base64 engine
    pub(crate) const BASE64_ENGINE: base64::engine::GeneralPurpose =
        base64::engine::general_purpose::STANDARD;
}
