use serde::{Deserialize, Serialize};
use time::PrimitiveDateTime;

use crate::types::country::country_iso::CountryISO;

use super::payment_flow::{FlowStatus, PaymentFlow, UiAccessMode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayPaymentFlowId {
    #[serde(rename = "unGatewaypaymentMethodFlowId")]
    pub gatewaypaymentMethodFlowId: String,
}

pub fn to_gateway_payment_flow_id(id: String) -> GatewayPaymentFlowId {
    GatewayPaymentFlowId {
        gatewaypaymentMethodFlowId: id,
    }
}

pub fn gateway_payment_flow_id_text(id: GatewayPaymentFlowId) -> String {
    id.gatewaypaymentMethodFlowId
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayPaymentFlowF {
    #[serde(rename = "id")]
    pub id: GatewayPaymentFlowId,
    #[serde(rename = "gateway")]
    pub gateway: String,
    #[serde(rename = "paymentFlowId")]
    pub paymentFlowId: PaymentFlow,
    #[serde(rename = "flowStatus")]
    pub flowStatus: FlowStatus,
    #[serde(rename = "disabled")]
    pub disabled: Option<bool>,
    #[serde(rename = "dateCreated")]
    pub dateCreated: PrimitiveDateTime,
    #[serde(rename = "lastUpdated")]
    pub lastUpdated: PrimitiveDateTime,
    #[serde(rename = "supportedPmt")]
    pub supportedPmt: Option<String>,
    #[serde(rename = "paymentFlowCombination")]
    pub paymentFlowCombination: Option<String>,
    #[serde(rename = "uiAccessMode")]
    pub uiAccessMode: Option<UiAccessMode>,
    #[serde(rename = "defaultValue")]
    pub defaultValue: Option<String>,
    #[serde(rename = "nonCombinationFlows")]
    pub nonCombinationFlows: Option<String>,
    #[serde(rename = "countryCodeAlpha3")]
    pub countryCodeAlpha3: Option<CountryISO>,
}
