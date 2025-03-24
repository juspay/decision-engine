// @generated automatically by Diesel CLI.

diesel::table! {
    card_details (id) {
        #[max_length = 36]
        id -> Varchar,
        #[max_length = 50]
        merchant_id -> Varchar,
        #[max_length = 50]
        customer_id -> Nullable<Varchar>,
        #[max_length = 255]
        card_number -> Varchar,
        #[max_length = 2]
        card_exp_month -> Varchar,
        #[max_length = 4]
        card_exp_year -> Varchar,
        #[max_length = 255]
        card_holder_name -> Nullable<Varchar>,
        #[max_length = 50]
        card_network -> Nullable<Varchar>,
        #[max_length = 50]
        card_type -> Nullable<Varchar>,
        #[max_length = 100]
        card_issuer -> Nullable<Varchar>,
        #[max_length = 3]
        card_issuing_country -> Nullable<Varchar>,
        #[max_length = 64]
        card_fingerprint -> Varchar,
        last_used_at -> Nullable<Timestamp>,
        created_at -> Nullable<Timestamp>,
        updated_at -> Nullable<Timestamp>,
    }
}
