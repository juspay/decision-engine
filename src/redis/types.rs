use serde::Deserialize;
use serde::Serialize;

// Original Haskell data type: FeatureConf
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct FeatureConf {
    #[serde(rename = "enableAll")]
    pub enableAll: bool,

    #[serde(rename = "enableAllRollout")]
    pub enableAllRollout: Option<i32>,

    #[serde(rename = "disableAny")]
    pub disableAny: Option<Vec<String>>,

    #[serde(rename = "merchants")]
    pub merchants: Option<Vec<MerchantFeature>>,
}

// Original Haskell data type: MerchantFeature
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct MerchantFeature {
    #[serde(rename = "merchantId")]
    pub merchantId: String,

    #[serde(rename = "rollout")]
    pub rollout: i32,
}

// Original Haskell data type: DimensionConf
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct DimensionConf {
    #[serde(rename = "enableAll")]
    pub enableAll: bool,

    #[serde(rename = "enableAllRollout")]
    pub enableAllRollout: Option<i32>,

    #[serde(rename = "disableAny")]
    pub disableAny: Option<Vec<FeatureDimension>>,

    #[serde(rename = "dimensions")]
    pub dimensions: Option<Vec<FeatureDimension>>,

    #[serde(rename = "dimensionType")]
    pub dimensionType: DimensionType,
}

// Original Haskell data type: FeatureDimension
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct FeatureDimension {
    #[serde(rename = "dimension")]
    pub dimension: String,

    #[serde(rename = "rollout")]
    pub rollout: i32,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DimensionType {
    JuspayBankCode,
    Gateway,
    CardBrand,
    Scof,
    Fido,
}

pub trait ServiceConfigKey {
    fn get_key(&self) -> String;
}
