// use eulerhs::prelude::*;
// use eulerhs::language::MonadFlow;
use crate::types::emi_bank_code as EBC;
use crate::types::gateway_bank_emi_support as ETGBES;
use crate::types::gateway_bank_emi_support_v2 as ETGBESV2;
use std::option::Option;
use std::string::String;
use std::vec::Vec;
// use data::list::is_suffix_of;
// use data::text as T;

pub async fn get_gateway_bank_emi_support(
    emiBank: Option<String>,
    gws: Vec<String>,
    scope: String,
) -> Vec<ETGBES::GatewayBankEmiSupport> {
    match emiBank {
        None => vec![],
        Some(_) if gws.is_empty() => vec![],
        Some(emiBank) => {
            if scope == "CARDLESS" {
                ETGBES::get_gateway_bank_emi_support(emiBank, gws, "CARD".to_string()).await
            } else {
                ETGBES::get_gateway_bank_emi_support(emiBank, gws, scope).await
            }
        }
    }
}

pub fn is_suffix_of(suffix: &str, str: &str) -> bool {
    str.ends_with(suffix)
}

pub async fn get_gateway_bank_emi_support_v2(
    emiBank: Option<String>,
    gws: Vec<String>,
    scope: String,
    tenure: Option<i32>,
) -> Vec<ETGBESV2::GatewayBankEmiSupportV2> {
    match (emiBank, tenure) {
        (Some(emiBank), Some(tenure)) => {
            let emiBankCodeList = EBC::findEmiBankCodeByEMIBank(&trim_suffix(&emiBank)).await;
            match (emiBankCodeList.as_slice(), scope.as_str()) {
                ([emiBankCode], "CARDLESS") => {
                    ETGBESV2::get_gateway_bank_emi_support_v2(
                        emiBankCode.juspay_bank_code_id,
                        gws.clone(),
                        scope,
                        "CONSUMER_FINANCE".to_string(),
                        tenure,
                    )
                    .await
                }
                ([emiBankCode], _) => {
                    let cardType = if is_suffix_of("DC", &emiBank) {
                        "DEBIT".to_string()
                    } else {
                        "CREDIT".to_string()
                    };
                    ETGBESV2::get_gateway_bank_emi_support_v2(
                        emiBankCode.juspay_bank_code_id,
                        gws.clone(),
                        scope,
                        cardType,
                        tenure,
                    )
                    .await
                }
                _ => vec![],
            }
        }
        _ => vec![],
    }
}

fn trim_suffix(str: &str) -> String {
    if is_suffix_of("_CLEMI", str) {
        str[..str.len() - "_CLEMI".len()].to_string()
    } else if is_suffix_of("_CC", str) {
        str[..str.len() - "_CC".len()].to_string()
    } else if is_suffix_of("DC", str) {
        str[..str.len() - "DC".len()].to_string()
    } else {
        str.to_string()
    }
}
