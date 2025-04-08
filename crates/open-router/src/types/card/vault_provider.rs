use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
            Self::Juspay => "JUSPAY",
            Self::PayU => "PAYU",
            Self::Sodexo => "SODEXO",
            Self::Cof => "COF_ISSUER",
            Self::NetworkToken => "NETWORK_TOKEN",
            Self::IssuerToken => "ISSUER_TOKEN",
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
            Self::Juspay => "JUSPAY",
            Self::PayU => "PAYU",
            Self::Sodexo => "SODEXO",
            Self::Cof => "COF_ISSUER",
            Self::NetworkToken => "NETWORK_TOKEN",
            Self::IssuerToken => "ISSUER_TOKEN",
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
        match value.to_uppercase().as_str() {
            "JUSPAY" => Ok(Self::Juspay),
            "PAYU" => Ok(Self::PayU),
            "SODEXO" => Ok(Self::Sodexo),
            "COF_ISSUER" => Ok(Self::Cof),
            "NETWORK_TOKEN" => Ok(Self::NetworkToken),
            "ISSUER_TOKEN" => Ok(Self::IssuerToken),
            _ => Err(de::Error::custom("Invalid value")),
        }
    }
}
