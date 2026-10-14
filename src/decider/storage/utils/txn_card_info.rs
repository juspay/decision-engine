// use euler_hs::prelude::*;
// use euler_hs::language::{MonadFlow, log_error_t, throw_exception};
// use sequelize::{Clause, Term};
// use db::storage::types::txncardinfo as DBTCI;
// use juspay::extra::parsing as P;
// use types::card as ETCa;
// use types::payment as ETP;

use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::payment_instrument::PaymentMethod;

pub fn is_google_pay_txn(tci: TxnCardInfo) -> bool {
    // Euler spells it `GOOGLEPAY`, Hyperswitch `google_pay`.
    tci.instrument.is(PaymentMethod::Wallet)
        && matches!(
            tci.instrument.method_type_code().as_ref(),
            "GOOGLEPAY" | "GOOGLE_PAY"
        )
}
