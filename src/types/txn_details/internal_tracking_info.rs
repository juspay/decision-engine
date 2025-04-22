// use serde::{Serialize, Deserialize};
// // use serde_json::Value as AValue;
// // use types::card::tokenization as TokenizationTypes;
// use std::option::Option;
// use std::vec::Vec;
// use std::string::String;

// #[derive(Debug, PartialEq, Serialize, Deserialize)]
// pub struct InternalTrackingInfo {
//     #[serde(rename = "tokenizationConsentUIPresented")]
//     pub tokenizationConsentUIPresented: Option<bool>,
//     #[serde(rename = "tokenizationConsent")]
//     pub tokenizationConsent: Option<bool>,
//     #[serde(rename = "tokenizationConsentFailureReason")]
//     pub tokenizationConsentFailureReason: Option<String>,
//     #[serde(rename = "issuerTokenizationConsentFailureReason")]
//     pub issuerTokenizationConsentFailureReason: Option<String>,
//     #[serde(rename = "tokenizationFailureReason")]
//     pub tokenizationFailureReason: Option<String>,
//     #[serde(rename = "issuerTokenizationFailureReason")]
//     pub issuerTokenizationFailureReason: Option<String>,
//     // #[serde(rename = "tokenizationInfo")]
//     // pub tokenizationInfo: Option<TokenizationInfo>,
// }

// #[derive(Debug, PartialEq, Serialize, Deserialize)]
// pub struct TokenizationInfo {
//     #[serde(rename = "eligibleServices")]
//     pub eligibleServices: Vec<TokenizationTypes::TokenServices>,
//     #[serde(rename = "serviceInEligibleReasons")]
//     pub serviceInEligibleReasons: Vec<(TokenizationTypes::TokenServices, String)>,
//     #[serde(rename = "tokenizationConsent")]
//     pub tokenizationConsent: Option<bool>,
// }