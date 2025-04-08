use super::schema;
use diesel::mysql::Mysql;
use diesel::serialize::{IsNull, Output};
use diesel::sql_types::Binary;
use diesel::*;
use diesel::{
    backend::Backend, deserialize::FromSql, serialize::ToSql, AsExpression, Identifiable,
    Queryable, Selectable,
};
use serde::Serialize;
use serde::{self, Deserialize};
use std::io::Write;
use time::PrimitiveDateTime;

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::card_brand_routes)]
pub struct CardBrandRoutes {
    pub id: i64,
    pub card_brand: String,
    pub date_created: PrimitiveDateTime,
    pub last_updated: PrimitiveDateTime,
    pub merchant_account_id: i64,
    pub preference_score: f64,
    pub preferred_gateway: String,
}

#[derive(Debug, Clone, Queryable, Deserialize, Identifiable, Serialize, Selectable)]
#[diesel(table_name = schema::card_info, primary_key(card_isin), check_for_backend(diesel::mysql::Mysql))]
pub struct CardInfo {
    pub card_isin: String,
    pub card_switch_provider: String,
    pub card_type: Option<String>,
    pub card_sub_type: Option<String>,
    pub card_sub_type_category: Option<String>,
    pub card_issuer_country: Option<String>,
    pub country_code: Option<String>,
    pub extended_card_type: Option<String>,
}

#[derive(Debug, Clone, Identifiable, Queryable, Deserialize, Serialize, Selectable)]
#[diesel(table_name = schema::emi_bank_code)]
pub struct EmiBankCode {
    pub id: i64,
    pub emi_bank: String,
    pub juspay_bank_code_id: i64,
    pub last_updated: Option<PrimitiveDateTime>,
}

#[derive(Debug, Clone, Identifiable, Queryable, Serialize, Selectable)]
#[diesel(table_name = schema::feature)]
pub struct Feature {
    pub id: i64,
    pub enabled: BitBool,
    pub name: String,
    pub merchant_id: Option<String>,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::gateway_bank_emi_support)]
pub struct GatewayBankEmiSupport {
    pub id: i64,
    pub gateway: String,
    pub bank: String,
    pub juspay_bank_code_id: Option<i64>,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::gateway_bank_emi_support_v2)]
pub struct GatewayBankEmiSupportV2 {
    pub id: i64,
    pub version: i64,
    pub gateway: String,
    pub juspay_bank_code_id: i64,
    pub card_type: String,
    pub tenure: i32,
    pub gateway_emi_code: String,
    pub gateway_plan_id: Option<String>,
    pub scope: String,
    pub metadata: Option<String>,
    pub date_created: Option<PrimitiveDateTime>,
    pub last_updated: Option<PrimitiveDateTime>,
}

#[derive(Debug, Clone, Identifiable, Queryable, Serialize, Selectable)]
#[diesel(table_name = schema::gateway_card_info)]
pub struct GatewayCardInfo {
    pub id: i64,
    pub isin: Option<String>,
    pub gateway: Option<String>,
    pub card_issuer_bank_name: Option<String>,
    pub auth_type: Option<String>,
    pub juspay_bank_code_id: Option<i64>,
    pub disabled: Option<BitBool>,
    pub validation_type: Option<String>,
    pub payment_method_type: Option<String>,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::gateway_outage)]
pub struct GatewayOutage {
    pub id: String,
    pub version: i32,
    pub end_time: PrimitiveDateTime,
    pub gateway: Option<String>,
    pub merchant_id: Option<String>,
    pub start_time: PrimitiveDateTime,
    pub bank: Option<String>,
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
    pub description: Option<String>,
    pub date_created: Option<PrimitiveDateTime>,
    pub last_updated: Option<PrimitiveDateTime>,
    pub juspay_bank_code_id: Option<i64>,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::gateway_payment_method_flow)]
pub struct GatewayPaymentMethodFlow {
    pub id: String,
    pub gateway_payment_flow_id: String,
    pub payment_method_id: Option<i64>,
    pub date_created: PrimitiveDateTime,
    pub last_updated: PrimitiveDateTime,
    pub gateway: String,
    pub payment_flow_id: String,
    pub juspay_bank_code_id: Option<i64>,
    pub gateway_bank_code: Option<String>,
    pub currency_configs: Option<String>,
    pub gateway_dsl: Option<String>,
    pub non_combination_flows: Option<String>,
    pub country_code_alpha3: Option<String>,
    pub disabled: BitBool,
    pub payment_method_type: Option<String>,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::isin_routes)]
pub struct IsinRoutes {
    pub id: i64,
    pub isin: String,
    pub merchant_id: String,
    pub preferred_gateway: String,
    pub preference_score: f64,
    pub date_created: PrimitiveDateTime,
    pub last_updated: PrimitiveDateTime,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::issuer_routes)]
pub struct IssuerRoutes {
    pub id: i64,
    pub issuer: String,
    pub merchant_id: String,
    pub preferred_gateway: String,
    pub preference_score: f64,
    pub date_created: PrimitiveDateTime,
    pub last_updated: PrimitiveDateTime,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::juspay_bank_code)]
pub struct JuspayBankCode {
    pub id: i64,
    pub bank_code: String,
    pub bank_name: String,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::merchant_account)]
pub struct MerchantAccount {
    pub id: i64,
    pub merchant_id: Option<String>,
    pub date_created: PrimitiveDateTime,
    pub gateway_decided_by_health_enabled: Option<BitBool>,
    pub gateway_priority: Option<String>,
    pub gateway_priority_logic: Option<String>,
    pub internal_hash_key: Option<String>,
    pub locker_id: Option<String>,
    pub token_locker_id: Option<String>,
    pub user_id: Option<i64>,
    pub settlement_account_id: Option<i64>,
    pub secondary_merchant_account_id: Option<i64>,
    pub use_code_for_gateway_priority: BitBool,
    pub enable_gateway_reference_id_based_routing: Option<BitBool>,
    pub gateway_success_rate_based_decider_input: Option<String>,
    pub internal_metadata: Option<String>,
    pub enabled: BitBool,
    pub country: Option<String>,
    pub installment_enabled: Option<BitBool>,
    pub tenant_account_id: Option<String>,
    pub priority_logic_config: Option<String>,
    pub merchant_category_code: Option<String>,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::merchant_config)]
pub struct MerchantConfig {
    pub id: String,
    pub merchant_account_id: i64,
    pub config_category: String,
    pub config_name: String,
    pub status: String,
    pub config_value: Option<String>,
    pub date_created: PrimitiveDateTime,
    pub last_updated: PrimitiveDateTime,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::merchant_gateway_account)]
pub struct MerchantGatewayAccount {
    pub id: i64,
    pub account_details: String,
    pub gateway: String,
    pub merchant_id: String,
    pub payment_methods: Option<String>,
    pub supported_payment_flows: Option<String>,
    pub disabled: Option<BitBool>,
    pub reference_id: Option<String>,
    pub supported_currencies: Option<String>,
    pub gateway_identifier: Option<String>,
    pub gateway_type: Option<String>,
    pub supported_txn_type: Option<String>,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::merchant_gateway_account_sub_info)]
pub struct MerchantGatewayAccountSubInfo {
    pub id: i64,
    pub merchant_gateway_account_id: i64,
    pub sub_info_type: String,
    pub sub_id_type: String,
    pub juspay_sub_account_id: String,
    pub gateway_sub_account_id: String,
    pub disabled: BitBool,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::merchant_gateway_card_info)]
pub struct MerchantGatewayCardInfo {
    pub id: i64,
    pub disabled: BitBool,
    pub gateway_card_info_id: i64,
    pub merchant_account_id: i64,
    pub emandate_register_max_amount: Option<f64>,
    pub merchant_gateway_account_id: Option<i64>,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::merchant_gateway_payment_method_flow)]
pub struct MerchantGatewayPaymentMethodFlow {
    pub id: i64,
    pub gateway_payment_method_flow_id: String,
    pub merchant_gateway_account_id: i64,
    pub currency_configs: Option<String>,
    pub date_created: PrimitiveDateTime,
    pub last_updated: PrimitiveDateTime,
    pub disabled: Option<BitBool>,
    pub gateway_bank_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, FromSqlRow, AsExpression, Serialize)]
#[diesel(sql_type = Binary)]
pub struct BitBool(pub bool);

impl ToSql<Binary, Mysql> for BitBool {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Mysql>) -> diesel::serialize::Result {
        match *self {
            BitBool(true) => {
                out.write_all(b"1")?;
            }
            BitBool(false) => {
                out.write_all(b"0")?;
            }
        }
            Ok(IsNull::No)
        }
}

impl FromSql<Binary, Mysql> for BitBool {
    fn from_sql(bytes: <Mysql as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        match bytes.as_bytes().first() {
            Some(&1) => Ok(Self(true)),
            _ => Ok(Self(false)),
        }
    }
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::merchant_iframe_preferences)]
pub struct MerchantIframePreferences {
    pub id: i64,
    pub merchant_id: String,
    pub dynamic_switching_enabled: Option<BitBool>,
    pub isin_routing_enabled: Option<BitBool>,
    pub issuer_routing_enabled: Option<BitBool>,
    pub txn_failure_gateway_penality: Option<BitBool>,
    pub card_brand_routing_enabled: Option<BitBool>,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::merchant_priority_logic)]
pub struct MerchantPriorityLogic {
    pub id: String,
    pub version: i64,
    pub date_created: PrimitiveDateTime,
    pub last_updated: PrimitiveDateTime,
    pub merchant_account_id: i64,
    pub status: String,
    pub priority_logic: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub priority_logic_rules: Option<String>,
    pub is_active_logic: BitBool,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::payment_method)]
pub struct PaymentMethod {
    pub id: i64,
    pub date_created: PrimitiveDateTime,
    pub last_updated: PrimitiveDateTime,
    pub name: String,
    pub pm_type: String,
    pub description: Option<String>,
    pub juspay_bank_code_id: Option<i64>,
    pub display_name: Option<String>,
    pub nick_name: Option<String>,
    pub sub_type: Option<String>,
    pub payment_dsl: Option<String>,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::service_configuration)]
pub struct ServiceConfiguration {
    pub id: i64,
    pub name: String,
    pub value: Option<String>,
    pub new_value: Option<String>,
    pub previous_value: Option<String>,
    pub new_value_status: Option<String>,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::tenant_config)]
pub struct TenantConfig {
    pub id: String,
    pub _type: String,
    pub module_key: String,
    pub module_name: String,
    pub tenant_account_id: String,
    pub config_value: String,
    pub filter_dimension: Option<String>,
    pub filter_group_id: Option<String>,
    pub status: String,
    pub country_code_alpha3: Option<String>,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::tenant_config_filter)]
pub struct TenantConfigFilter {
    pub id: String,
    pub filter_group_id: String,
    pub dimension_value: String,
    pub config_value: String,
    pub tenant_config_id: String,
}

#[derive(Debug, Clone, Queryable, Deserialize, Identifiable, Serialize, Selectable)]
#[diesel(table_name = schema::token_bin_info, primary_key(token_bin), check_for_backend(diesel::mysql::Mysql))]
pub struct TokenBinInfo {
    pub token_bin: String,
    pub card_bin: String,
    pub provider: String,
    pub date_created: Option<PrimitiveDateTime>,
    pub last_updated: Option<PrimitiveDateTime>,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::txn_card_info)]
pub struct TxnCardInfo {
    pub id: i64,
    pub txn_id: String,
    pub card_isin: Option<String>,
    pub card_issuer_bank_name: Option<String>,
    pub card_switch_provider: Option<String>,
    pub card_type: Option<String>,
    pub name_on_card: Option<String>,
    pub txn_detail_id: Option<i64>,
    pub date_created: Option<PrimitiveDateTime>,
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
    pub payment_source: Option<String>,
    pub auth_type: Option<String>,
    pub partition_key: Option<PrimitiveDateTime>,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::txn_detail)]
pub struct TxnDetail {
    pub id: i64,
    pub order_id: String,
    pub status: String,
    pub txn_id: String,
    pub txn_type: String,
    pub date_created: Option<PrimitiveDateTime>,
    pub add_to_locker: Option<BitBool>,
    pub merchant_id: Option<String>,
    pub gateway: Option<String>,
    pub express_checkout: Option<BitBool>,
    pub is_emi: Option<BitBool>,
    pub emi_bank: Option<String>,
    pub emi_tenure: Option<i32>,
    pub txn_uuid: Option<String>,
    pub merchant_gateway_account_id: Option<i64>,
    pub net_amount: Option<f64>,
    pub txn_amount: Option<f64>,
    pub txn_object_type: Option<String>,
    pub source_object: Option<String>,
    pub source_object_id: Option<String>,
    pub currency: Option<String>,
    pub surcharge_amount: Option<f64>,
    pub tax_amount: Option<f64>,
    pub internal_metadata: Option<String>,
    pub metadata: Option<String>,
    pub offer_deduction_amount: Option<f64>,
    pub internal_tracking_info: Option<String>,
    pub partition_key: Option<PrimitiveDateTime>,
    pub txn_amount_breakup: Option<String>,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::txn_offer)]
pub struct TxnOffer {
    pub id: i64,
    pub version: i64,
    pub discount_amount: i64,
    pub offer_id: String,
    pub signature: String,
    pub txn_detail_id: i64,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::txn_offer_detail)]
pub struct TxnOfferDetail {
    pub id: String,
    pub txn_detail_id: String,
    pub offer_id: String,
    pub status: String,
    pub date_created: Option<PrimitiveDateTime>,
    pub last_updated: Option<PrimitiveDateTime>,
    pub gateway_info: Option<String>,
    pub internal_metadata: Option<String>,
    pub partition_key: Option<PrimitiveDateTime>,
}

#[derive(Debug, Clone, Identifiable, Queryable)]
#[diesel(table_name = schema::user_eligibility_info)]
pub struct UserEligibilityInfo {
    pub id: String,
    pub flow_type: String,
    pub identifier_name: String,
    pub identifier_value: String,
    pub provider_name: String,
    pub disabled: Option<BitBool>,
}
