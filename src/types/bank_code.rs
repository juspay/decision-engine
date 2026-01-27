use crate::app::get_tenant_app_state;
use diesel::associations::HasTable;
use diesel::ExpressionMethods;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankCodeId(pub i64);
#[cfg(feature = "mysql")]
use crate::storage::{schema::juspay_bank_code::dsl, types::JuspayBankCode as DBBankCode};
#[cfg(feature = "postgres")]
use crate::storage::{schema_pg::juspay_bank_code::dsl, types::JuspayBankCode as DBBankCode};

pub fn to_bank_code_id(id: i64) -> BankCodeId {
    BankCodeId(id)
}
pub struct BankCode {
    pub id: BankCodeId,
    pub bank_code: String,
    pub bank_name: String,
}

impl From<DBBankCode> for BankCode {
    fn from(value: DBBankCode) -> Self {
        Self {
            id: to_bank_code_id(value.id),
            bank_code: value.bank_code,
            bank_name: value.bank_name,
        }
    }
}

// pub fn parse_juspay_bank_code(
//     db_record: &DBBankCode,
// ) -> Result<JuspayBankCode, Box<dyn Error>> {
//     Ok(JuspayBankCode {
//         id: JuspayBankCodeId(db_record.id),
//         bank_code: db_record.bank_code.clone(),
//         bank_name: db_record.bank_name.clone(),
//     })
// }

// #TOD implement db calls

pub async fn find_bank_code(bank_code: String) -> Option<BankCode> {
    let app_state = get_tenant_app_state().await;
    match crate::generics::generic_find_one::<<DBBankCode as HasTable>::Table, _, DBBankCode>(
        &app_state.db,
        dsl::bank_code.eq(bank_code),
    )
    .await
    {
        Ok(db_record) => parse_juspay_bank_code(db_record).ok(),
        Err(e) => None,
    }
}

pub fn parse_juspay_bank_code(db_record: DBBankCode) -> crate::generics::StorageResult<BankCode> {
    Ok(db_record.into())
}
