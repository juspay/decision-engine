use masking::{Secret, StrongSecret};

use crate::{
    error,
    storage::{
        self,
        types::{Encryptable, OpenRouter},
    },
    utils,
};

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Card {
    pub card_number: storage::types::CardNumber,
    name_on_card: Option<String>,
    card_exp_month: Option<String>,
    card_exp_year: Option<String>,
    card_brand: Option<String>,
    card_isin: Option<String>,
    nick_name: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DataDuplicationCheck {
    Duplicated,
    MetaDataChanged,
}
