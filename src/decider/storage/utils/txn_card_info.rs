// use euler_hs::prelude::*;
// use euler_hs::language::{MonadFlow, log_error_t, throw_exception};
// use sequelize::{Clause, Term};
// use db::storage::types::txncardinfo as DBTCI;
// use juspay::extra::parsing as P;
// use types::card as ETCa;
// use types::payment as ETP;

use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::payment_instrument::{PaymentMethod, PaymentMethodType};

pub fn is_google_pay_txn(tci: TxnCardInfo) -> bool {
    tci.instrument.is(PaymentMethod::Wallet) && tci.instrument.is_type(PaymentMethodType::GooglePay)
}
