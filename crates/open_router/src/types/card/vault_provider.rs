```rust
use serde::{Serialize, Deserialize};
use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use std::fmt;

#[derive(Debug, PartialEq, Eq, Ord)]
pub enum VaultProvider {
    Juspay,
    PayU,
    Sodexo,
    Cof,
    NetworkToken,
    IssuerToken,
}

impl fmt::Display for VaultProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            VaultProvider::Juspay => "JUSPAY",
            VaultProvider::PayU => "PAYU",
            VaultProvider::Sodexo => "SODEXO",
            VaultProvider::Cof => "COF_ISSUER",
            VaultProvider::NetworkToken => "NETWORK_TOKEN",
            VaultProvider::IssuerToken => "ISSUER_TOKEN",
        };
        write!(f, "{}", value)
    }
}

impl Serialize for VaultProvider {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = match self {
            VaultProvider::Juspay => "JUSPAY",
            VaultProvider::PayU => "PAYU",
            VaultProvider::Sodexo => "SODEXO",
            VaultProvider::Cof => "COF_ISSUER",
            VaultProvider::NetworkToken => "NETWORK_TOKEN",
            VaultProvider::IssuerToken => "ISSUER_TOKEN",
        };
        serializer.serialize_str(value)
    }
}

impl<'de> Deserialize<'de> for VaultProvider {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "JUSPAY" => Ok(VaultProvider::Juspay),
            "PAYU" => Ok(VaultProvider::PayU),
            "SODEXO" => Ok(VaultProvider::Sodexo),
            "COF_ISSUER" => Ok(VaultProvider::Cof),
            "NETWORK_TOKEN" => Ok(VaultProvider::NetworkToken),
            "ISSUER_TOKEN" => Ok(VaultProvider::IssuerToken),
            _ => Err(de::Error::custom("Invalid value")),
        }
    }
}
```