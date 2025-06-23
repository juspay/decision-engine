use crate::app::get_tenant_app_state;
use crate::types::payment_flow::*;
use serde::{Deserialize, Serialize};
use std::string::String;
use std::time::SystemTime;
use std::vec::Vec;
use crate::storage::types::{MicroPaymentFlow as DBMicroPaymentFlow};

use super::payment_flow::{FlowLevel, MicroPaymentFlowName, FlowLevelId, MicroPaymentFlowType, to_flow_level_id, text_to_flow_level, text_to_micro_payment_flow_name, text_to_micro_payment_flow_type
  };
#[cfg(feature = "mysql")]
use crate::storage::schema::micro_payment_flow::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::micro_payment_flow::dsl;
use diesel::associations::HasTable;
use diesel::*;
use crate::logger;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MicroPaymentFlowId {
    pub microPaymentFlowId: String,
}

pub fn to_micro_payment_flow_id(id: String) -> MicroPaymentFlowId {
    MicroPaymentFlowId {
        microPaymentFlowId: id,
    }
}

pub fn micro_payment_flow_id_text(id: MicroPaymentFlowId) -> String {
    id.microPaymentFlowId
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicroPaymentFlowF {
    #[serde(rename = "id")]
    pub id: MicroPaymentFlowId,
    #[serde(rename = "flowLevel")]
    pub flowLevel: FlowLevel,
    #[serde(rename = "flowLevelId")]
    pub flowLevelId: FlowLevelId,
    #[serde(rename = "value")]
    pub value: String,
    #[serde(rename = "microPaymentFlowName")]
    pub microPaymentFlowName: MicroPaymentFlowName,
    #[serde(rename = "valueType")]
    pub valueType: MicroPaymentFlowType,
    #[serde(rename = "dateCreated")]
    pub dateCreated: SystemTime,
    #[serde(rename = "lastUpdated")]
    pub lastUpdated: SystemTime,
}

impl TryFrom<DBMicroPaymentFlow> for MicroPaymentFlowF {
    type Error = crate::error::ApiError;

    fn try_from(db: DBMicroPaymentFlow) -> Result<Self, Self::Error> {
        // Convert string IDs to domain types
        let id = to_micro_payment_flow_id(db.id);
        let flow_level = text_to_flow_level(db.flow_level)?;
        let flow_level_id = to_flow_level_id(db.flow_level_id);
        let micro_payment_flow_name = text_to_micro_payment_flow_name(db.micro_payment_flow_name)?;
        let value_type = text_to_micro_payment_flow_type(db.value_type)?;

        // Construct the GatewayPaymentMethodFlow instance
        Ok(Self {
            id,
            flowLevel:flow_level,
            flowLevelId:flow_level_id,
            dateCreated: db.date_created.assume_utc().into(),
            lastUpdated: db.last_updated.assume_utc().into(),
            microPaymentFlowName:micro_payment_flow_name,
            valueType:value_type,
            value: db.value,
        })
    }
}

pub async fn find_mpf_by_flow_level_flow_level_ids_mpf_name_db(
    flow_level: FlowLevel,
    flow_level_ids: Vec<FlowLevelId>,
    mpf_name: MicroPaymentFlowName,
) -> Result<Vec<DBMicroPaymentFlow>, crate::generics::MeshError> {
    let app_state = get_tenant_app_state().await;

    // Extract and convert various IDs to their string representations
    let flow_level_text = flow_level_to_text(&flow_level);
    let mpfn_text = micro_payment_flow_name_to_text(&mpf_name);
    let flow_level_ids_text: Vec<String> = flow_level_ids.into_iter().map(|fl_id| fl_id.flowLevelId).collect();


    // Use Diesel's query builder with multiple conditions
    crate::generics::generic_find_all::<
        <DBMicroPaymentFlow as HasTable>::Table,
        _,
        DBMicroPaymentFlow,
    >(
        &app_state.db,
        dsl::flow_level_id
            .eq_any(flow_level_ids_text)
            .and(dsl::flow_level.eq(flow_level_text))
            .and(dsl::micro_payment_flow_name.eq(mpfn_text)),
    )
    .await
}

pub async fn find_mpf_by_flow_level_flow_level_ids_mpf_name(
    flow_level: FlowLevel,
    flow_level_ids: Vec<FlowLevelId>,
    mpf_name: MicroPaymentFlowName,
) -> Vec<MicroPaymentFlowF> {
    // Call the DB function and handle the results
    match find_mpf_by_flow_level_flow_level_ids_mpf_name_db(flow_level, flow_level_ids, mpf_name)
        .await
    {
        Ok(db_results) => db_results
            .into_iter()
            .filter_map(|db_record: DBMicroPaymentFlow| {
                MicroPaymentFlowF::try_from(db_record).ok()
            })
            .collect(),
        Err(err) => {
            logger::info!("Error in find_mpf_by_flow_level_flow_level_ids_mpf_name: {:?}", err);
            Vec::new() // Silently handle any errors by returning empty vec
        }
    }
}

