use serde::{Serialize, Deserialize};
use std::option::Option;
use std::string::String;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
pub struct StoredCardVaultProvider {
    #[serde(rename = "storedCardVaultProvider")]
    pub storedCardVaultProvider: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
pub struct InternalMetadata {
    #[serde(rename = "storedCardVaultProvider")]
    pub storedCardVaultProvider: Option<String>,
    #[serde(rename = "tokenReference")]
    pub tokenReference: Option<String>,
    #[serde(rename = "issuerTokenReference")]
    pub issuerTokenReference: Option<String>,
}