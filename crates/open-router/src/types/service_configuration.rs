use crate::app::get_tenant_app_state;
use crate::storage::schema::service_configuration::dsl;
use diesel::associations::HasTable;
use diesel::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as AValue;
use std::option::Option;
use std::string::String;
use std::time::SystemTime;
use std::vec::Vec;
use time::PrimitiveDateTime;
use crate::storage::types::MerchantGatewayPaymentMethodFlow as DBMerchantGatewayPaymentMethodFlow;
use crate::types::gateway_payment_method_flow::{
    gateway_payment_method_flow_id_text, to_gateway_payment_method_flow_id,
    GatewayPaymentMethodFlowId,
};
// use sequelize::{Clause::{Is, And}, Term::{Eq, In}};
use crate::storage::types::ServiceConfiguration;
use std::collections::HashMap;

pub async fn find_config_by_name(
    name: String,
) -> Result<Option<ServiceConfiguration>, crate::generics::MeshError> {
    // Extract IDs from GciPId objects
    let app_state = get_tenant_app_state().await;
    // Use Diesel's query builder with multiple conditions
    crate::generics::generic_find_one_optional::<
        <ServiceConfiguration as HasTable>::Table,
        _,
        ServiceConfiguration,
    >(&app_state.db, dsl::name.eq(name))
    .await
}
