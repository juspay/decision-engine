use crate::app::get_tenant_app_state;
use crate::types::country::country_iso::CountryISO;
use crate::types::payment::payment_method::{to_payment_method_id, PaymentMethodId};
use crate::types::payment_flow::*;
use serde::{Deserialize, Serialize};
use std::option::Option;
use std::string::String;
use std::time::SystemTime;
use std::vec::Vec;
// use types::payment::payment_method::to_payment_method_id;
// use types::utils::dbconfig::get_euler_db_conf;
use crate::storage::types::{BitBool, GatewayPaymentMethodFlow as DBGatewayPaymentMethodFlow};
// use db::common::types::paymentflows as PFTypes;
// use gpf::GatewayPaymentFlowId;
// use gpf::to_gateway_payment_flow_id;
// use juspay_extra::parsing::{Parsed, Step, around, non_empty_text, parse_field, project, to_utc};
// use juspay_extra::non_empty_text::NonEmptyText;
// use juspay_extra::non_empty_text::newtype_net_prism;
use crate::types::bank_code::{to_bank_code_id, BankCodeId};

use super::country::country_iso::country_iso_to_text;
use super::gateway_payment_flow::GatewayPaymentFlowId;
use super::payment_flow::PaymentFlow;
use crate::types::gateway_payment_flow::to_gateway_payment_flow_id;
// use eulerhs::language::MonadFlow;
// use db::mesh::internal::{MeshError, find_all_rows};
// use eulerhs::extra::combinators::to_domain_all;
// use db::eulermeshimpl::mesh_config;
#[cfg(feature = "mysql")]
use crate::storage::schema::gateway_payment_method_flow::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::gateway_payment_method_flow::dsl;
use diesel::associations::HasTable;
use diesel::*;

// #TODO type for nonempty text
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GatewayPaymentMethodFlowId {
    #[serde(rename = "unGatewaypaymentMethodFlowId")]
    pub gatewaypaymentMethodFlowId: String,
}

pub fn to_gateway_payment_method_flow_id(id: String) -> GatewayPaymentMethodFlowId {
    GatewayPaymentMethodFlowId {
        gatewaypaymentMethodFlowId: id,
    }
}

pub fn gateway_payment_method_flow_id_text(id: GatewayPaymentMethodFlowId) -> String {
    id.gatewaypaymentMethodFlowId
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayPaymentMethodFlowF {
    #[serde(rename = "id")]
    pub id: GatewayPaymentMethodFlowId,
    #[serde(rename = "gatewayPaymentFlowId")]
    pub gatewayPaymentFlowId: GatewayPaymentFlowId,
    #[serde(rename = "paymentMethodId")]
    pub paymentMethodId: Option<PaymentMethodId>,
    #[serde(rename = "dateCreated")]
    pub dateCreated: SystemTime,
    #[serde(rename = "lastUpdated")]
    pub lastUpdated: SystemTime,
    #[serde(rename = "gateway")]
    pub gateway: String,
    #[serde(rename = "paymentFlowId")]
    pub paymentFlowId: PaymentFlow,
    #[serde(rename = "juspayBankCodeId")]
    pub juspayBankCodeId: Option<BankCodeId>,
    #[serde(rename = "gatewayBankCode")]
    pub gatewayBankCode: Option<String>,
    #[serde(rename = "currencyConfigs")]
    pub currencyConfigs: Option<String>,
    #[serde(rename = "dsl")]
    pub dsl: Option<String>,
    #[serde(rename = "nonCombinationFlows")]
    pub nonCombinationFlows: Option<String>,
    #[serde(rename = "countryCodeAlpha3")]
    pub countryCodeAlpha3: Option<CountryISO>,
    #[serde(rename = "disabled")]
    pub disabled: bool,
    #[serde(rename = "paymentMethodType")]
    pub paymentMethodType: Option<String>,
}

impl TryFrom<DBGatewayPaymentMethodFlow> for GatewayPaymentMethodFlowF {
    type Error = crate::error::ApiError;

    fn try_from(db: DBGatewayPaymentMethodFlow) -> Result<Self, Self::Error> {
        // Convert string IDs to domain types
        let id = to_gateway_payment_method_flow_id(db.id);
        let gateway_payment_flow_id = to_gateway_payment_flow_id(db.gateway_payment_flow_id);

        // Convert optional payment method ID
        let payment_method_id = db.payment_method_id.map(to_payment_method_id);

        // Convert string representations to enum values
        let gateway = db.gateway;
        let payment_flow_id = text_to_payment_flows(db.payment_flow_id)?;

        // Convert optional bank code ID
        let juspay_bank_code_id = db.juspay_bank_code_id.map(to_bank_code_id);

        // Convert optional country code
        let country_code_alpha3 = if let Some(code) = db.country_code_alpha3 {
            Some(crate::types::country::country_iso::text_db_to_country_iso(
                &code,
            )?)
        } else {
            None
        };

        // Convert optional payment method type
        let payment_method_type = if let Some(pmt) = db.payment_method_type {
            Some(pmt)
        } else {
            None
        };

        // Construct the GatewayPaymentMethodFlow instance
        Ok(Self {
            id,
            gatewayPaymentFlowId: gateway_payment_flow_id,
            paymentMethodId: payment_method_id,
            dateCreated: db.date_created.assume_utc().into(),
            lastUpdated: db.last_updated.assume_utc().into(),
            gateway,
            paymentFlowId: payment_flow_id,
            juspayBankCodeId: juspay_bank_code_id,
            gatewayBankCode: db.gateway_bank_code,
            currencyConfigs: db.currency_configs,
            dsl: db.gateway_dsl,
            nonCombinationFlows: db.non_combination_flows,
            countryCodeAlpha3: country_code_alpha3,
            disabled: db.disabled.0,
            paymentMethodType: payment_method_type,
        })
    }
}

pub async fn find_all_gpmf_by_gateway_payment_flow_payment_method_db(
    gw_ls: Vec<String>,
    pm_id: PaymentMethodId,
    pf_id: PaymentFlow,
) -> Result<Vec<DBGatewayPaymentMethodFlow>, crate::generics::MeshError> {
    let app_state = get_tenant_app_state().await;
    // Extract payment method ID and payment flow ID
    let pm_id_text = pm_id.0;
    let pf_id_text = payment_flows_to_text(&pf_id);

    // Use Diesel's query builder with multiple conditions
    crate::generics::generic_find_all::<
        <DBGatewayPaymentMethodFlow as HasTable>::Table,
        _,
        DBGatewayPaymentMethodFlow,
    >(
        &app_state.db,
        dsl::gateway
            .eq_any(gw_ls)
            .and(dsl::payment_method_id.eq(Some(pm_id_text)))
            .and(dsl::payment_flow_id.eq(pf_id_text))
            .and(dsl::disabled.eq(BitBool(false))),
    )
    .await
}

pub async fn find_all_gpmf_by_gateway_payment_flow_payment_method(
    gw: Vec<String>,
    pm_id: PaymentMethodId,
    pf_id: PaymentFlow,
) -> Vec<GatewayPaymentMethodFlowF> {
    // Call the DB function and handle the results
    match find_all_gpmf_by_gateway_payment_flow_payment_method_db(gw, pm_id, pf_id).await {
        Ok(db_results) => db_results
            .into_iter()
            .filter_map(|db_record: DBGatewayPaymentMethodFlow| {
                GatewayPaymentMethodFlowF::try_from(db_record).ok()
            })
            .collect(),
        Err(_) => Vec::new(), // Silently handle any errors by returning empty vec
    }
}

// #TOD implement db calls (only 1st & 2nd function required)

// pub async fn find_all_gpmf_by_gateway_payment_flow_payment_method_db(
//     gw: Vec<Gateway>,
//     pm_id: PaymentMethodId,
//     pf_id: PFTypes::PaymentFlow,
// ) -> Result<Vec<DB::GatewayPaymentMethodFlow>, MeshError> {
//     let db_conf = get_euler_db_conf::<DB::GatewayPaymentMethodFlowT>().await?;
//     let gw_ls = gw.iter().map(|g| gateway_to_text(g)).collect::<Vec<_>>();
//     let pm_id_text = pm_id.un_payment_method_id();
//     let pf_id_text = review(GPF::payment_flow_text(), pf_id);

//     find_all_rows(
//         db_conf,
//         mesh_config(),
//         vec![And(vec![
//             Is(DB::gateway(), In(gw_ls)),
//             Is(DB::paymentMethodId(), Eq(Some(pm_id_text))),
//             Is(DB::paymentFlowId(), Eq(pf_id_text)),
//             Is(DB::disabled(), Eq(false)),
//         ])],
//     )
//     .await
// }

// pub async fn find_all_gpmf_by_gateway_payment_flow_payment_method(
//     gw: Vec<Gateway>,
//     pm_id: PaymentMethodId,
//     pf_id: PFTypes::PaymentFlow,
// ) -> Vec<GatewayPaymentMethodFlow> {
//     let db_res = find_all_gpmf_by_gateway_payment_flow_payment_method_db(gw, pm_id, pf_id).await?;
//     to_domain_all(
//         db_res,
//         parse_gateway_payment_method_flow,
//         "findGPMFByGatewayPaymentFlowPaymentMethod",
//         "parseGatewayPaymentMethodFlow",
//     )
//     .await
// }

pub async fn find_all_gpmf_by_country_code_gw_pf_id_pmt_jbcid_db(
    country_code: CountryISO,
    gw_ls: Vec<String>,
    pf_id: PaymentFlow,
    pmt: String,
    jbc_id: BankCodeId,
) -> Result<Vec<DBGatewayPaymentMethodFlow>, crate::generics::MeshError> {
    let app_state = get_tenant_app_state().await;

    // Extract and convert various IDs to their string representations
    let jbc_id_text = jbc_id.0;
    let pf_id_text = payment_flows_to_text(&pf_id);
    let country_code_text = country_iso_to_text(country_code);
    let pmt_text = pmt;

    // Use Diesel's query builder with multiple conditions
    crate::generics::generic_find_all::<
        <DBGatewayPaymentMethodFlow as HasTable>::Table,
        _,
        DBGatewayPaymentMethodFlow,
    >(
        &app_state.db,
        dsl::gateway
            .eq_any(gw_ls)
            .and(dsl::juspay_bank_code_id.eq(Some(jbc_id_text)))
            .and(dsl::payment_flow_id.eq(pf_id_text))
            .and(dsl::disabled.eq(BitBool(false)))
            .and(dsl::country_code_alpha3.eq(Some(country_code_text)))
            .and(dsl::payment_method_type.eq(Some(pmt_text))),
    )
    .await
}

pub async fn find_all_gpmf_by_country_code_gw_pf_id_pmt_jbcid(
    country_code: CountryISO,
    gw: Vec<String>,
    pf_id: PaymentFlow,
    pmt: String,
    jbc_id: BankCodeId,
) -> Vec<GatewayPaymentMethodFlowF> {
    // Call the DB function and handle the results
    match find_all_gpmf_by_country_code_gw_pf_id_pmt_jbcid_db(country_code, gw, pf_id, pmt, jbc_id)
        .await
    {
        Ok(db_results) => db_results
            .into_iter()
            .filter_map(|db_record: DBGatewayPaymentMethodFlow| {
                GatewayPaymentMethodFlowF::try_from(db_record).ok()
            })
            .collect(),
        Err(_) => Vec::new(), // Silently handle any errors by returning empty vec
    }
}
