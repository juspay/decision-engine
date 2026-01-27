// use euler_hs::prelude::*;
// use euler_hs::language::{MonadFlow, log_error_t, throw_exception};
// use sequelize::{Clause, Term};
// use db::storage::types::txncardinfo as DBTCI;
// use juspay::extra::parsing as P;
// use types::card as ETCa;
// use types::payment as ETP;

use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::payment::payment_method_type_const::*;

pub fn is_google_pay_txn(tci: TxnCardInfo) -> bool {
    tci.paymentMethodType == WALLET && tci.paymentMethod == "GOOGLEPAY"
}
