use crate::types::merchant::id::MerchantId;
use crate::types::payment::payment_method::PaymentMethodType;
use serde::{Deserialize, Serialize};
use std::option::Option;
use std::string::String;
use std::vec::Vec;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EliminationLevel {
    #[serde(rename = "GATEWAY")]
    GATEWAY,
    #[serde(rename = "PAYMENT_METHOD_TYPE")]
    PAYMENT_METHOD_TYPE,
    #[serde(rename = "PAYMENT_METHOD")]
    PAYMENT_METHOD,
    #[serde(rename = "NONE")]
    NONE,
    #[serde(rename = "FORCED_PAYMENT_METHOD")]
    FORCED_PAYMENT_METHOD,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SelectionLevel {
    #[serde(rename = "PAYMENT_MODE")]
    SL_PAYMENT_MODE,
    #[serde(rename = "PAYMENT_METHOD")]
    SL_PAYMENT_METHOD,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayScore {
    pub timestamp: i64,
    pub score: f64,
    pub transactionCount: i64,
    pub lastResetTimestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalGatewayScore {
    pub timestamp: i64,
    pub merchants: Vec<GlobalScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalScore {
    pub transactionCount: i64,
    pub score: f64,
    pub merchantId: MerchantId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalScoreLog {
    pub transactionCount: i64,
    pub currentScore: f64,
    pub merchantId: MerchantId,
    pub eliminationThreshold: f64,
    pub eliminationMaxCountThreshold: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayWiseSuccessRateBasedRoutingInput {
    pub gateway: String,
    #[serde(rename = "eliminationThreshold")]
    pub eliminationThreshold: Option<f64>,
    #[serde(rename = "eliminationMaxCountThreshold")]
    pub eliminationMaxCountThreshold: Option<i64>,
    #[serde(rename = "selectionMaxCountThreshold")]
    pub selectionMaxCountThreshold: Option<i64>,
    #[serde(rename = "softTxnResetCount")]
    pub softTxnResetCount: Option<i64>,
    #[serde(rename = "gatewayLevelEliminationThreshold")]
    pub gatewayLevelEliminationThreshold: Option<f64>,
    #[serde(rename = "eliminationLevel")]
    pub eliminationLevel: Option<EliminationLevel>,
    #[serde(rename = "currentScore")]
    pub currentScore: Option<f64>,
    #[serde(rename = "lastResetTimeStamp")]
    pub lastResetTimeStamp: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EliminationSuccessRateInput {
    pub successRate: f64,
    pub paymentMethodType: String,
    pub paymentMethod: Option<String>,
    pub txnObjectType: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewaySuccessRateBasedRoutingInput {
    #[serde(rename = "gatewayWiseInputs")]
    pub gatewayWiseInputs: Option<Vec<GatewayWiseSuccessRateBasedRoutingInput>>,
    #[serde(rename = "defaultEliminationThreshold")]
    pub defaultEliminationThreshold: f64,
    #[serde(rename = "defaultEliminationLevel")]
    pub defaultEliminationLevel: EliminationLevel,
    #[serde(rename = "defaultSelectionLevel")]
    pub defaultSelectionLevel: Option<SelectionLevel>,
    #[serde(rename = "enabledPaymentMethodTypes")]
    pub enabledPaymentMethodTypes: Vec<PaymentMethodType>,
    #[serde(rename = "eliminationV2SuccessRateInputs")]
    pub eliminationV2SuccessRateInputs: Option<Vec<EliminationSuccessRateInput>>,
    #[serde(rename = "globalGatewayWiseInputs")]
    pub globalGatewayWiseInputs: Option<Vec<GatewayWiseSuccessRateBasedRoutingInput>>,
    #[serde(rename = "defaultGlobalEliminationThreshold")]
    pub defaultGlobalEliminationThreshold: Option<f64>,
    #[serde(rename = "defaultGlobalEliminationMaxCountThreshold")]
    pub defaultGlobalEliminationMaxCountThreshold: Option<i64>,
    #[serde(rename = "defaultGlobalEliminationLevel")]
    pub defaultGlobalEliminationLevel: Option<EliminationLevel>,
    #[serde(rename = "defaultGlobalSelectionMaxCountThreshold")]
    pub defaultGlobalSelectionMaxCountThreshold: Option<i64>,
    #[serde(rename = "selectionTransactionCountThreshold")]
    pub selectionTransactionCountThreshold: Option<i64>,
    #[serde(rename = "defaultGlobalSoftTxnResetCount")]
    pub defaultGlobalSoftTxnResetCount: Option<i64>,
    #[serde(rename = "defaultGatewayLevelEliminationThreshold")]
    pub defaultGatewayLevelEliminationThreshold: Option<f64>,
    #[serde(rename = "defaultEliminationV2SuccessRate")]
    pub defaultEliminationV2SuccessRate: Option<f64>,
}

impl GatewaySuccessRateBasedRoutingInput {
    pub fn from_elimination_threshold(elimination_threshold: f64) -> Self {
        Self {
            gatewayWiseInputs: None,
            defaultEliminationThreshold: elimination_threshold,
            defaultEliminationLevel: EliminationLevel::PAYMENT_METHOD,
            defaultSelectionLevel: None,
            enabledPaymentMethodTypes: vec![],
            eliminationV2SuccessRateInputs: None,
            globalGatewayWiseInputs: None,
            defaultGlobalEliminationThreshold: None,
            defaultGlobalEliminationMaxCountThreshold: None,
            defaultGlobalEliminationLevel: None,
            defaultGlobalSelectionMaxCountThreshold: None,
            selectionTransactionCountThreshold: None,
            defaultGlobalSoftTxnResetCount: None,
            defaultGatewayLevelEliminationThreshold: None,
            defaultEliminationV2SuccessRate: None,
        }
    }
}
