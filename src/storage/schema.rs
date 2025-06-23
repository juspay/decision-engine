// @generated automatically by Diesel CLI.

diesel::table! {
    card_brand_routes (id) {
        id -> Bigint,
        card_brand -> Text,
        date_created -> Datetime,
        last_updated -> Datetime,
        merchant_account_id -> Bigint,
        preference_score -> Double,
        preferred_gateway -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    card_info (card_isin) {
        card_isin -> Text,
        card_switch_provider -> Text,
        card_type -> Nullable<Text>,
        card_sub_type -> Nullable<Text>,
        card_sub_type_category -> Nullable<Text>,
        card_issuer_country -> Nullable<Text>,
        country_code -> Nullable<Text>,
        extended_card_type -> Nullable<Text>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    co_badged_cards_info_test (id) {
        id -> Varchar,
        card_bin_min -> BigInt,
        card_bin_max -> BigInt,
        issuing_bank_name -> Nullable<Text>,
        card_network -> Varchar,
        country_code -> Nullable<Text>,
        card_type -> Nullable<Text>,
        regulated -> Nullable<Bool>,
        regulated_name -> Nullable<Text>,
        prepaid -> Nullable<Bool>,
        reloadable -> Nullable<Bool>,
        pan_or_token -> Text,
        card_bin_length -> SmallInt,
        bin_provider_bin_length -> SmallInt,
        card_brand_is_additional -> Bool,
        domestic_only -> Nullable<Bool>,
        created_at -> Timestamp,
        modified_at -> Timestamp,
        last_updated_provider -> Nullable<Varchar>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    emi_bank_code (id) {
        id -> Bigint,
        emi_bank -> Text,
        juspay_bank_code_id -> Bigint,
        last_updated -> Nullable<Datetime>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    feature (id) {
        id -> BigInt,
        enabled -> Bit,
        name -> Text,
        merchant_id -> Nullable<Text>,
    }
}

diesel::table! {
    gateway_bank_emi_support (id) {
        id -> Bigint,
        gateway -> Text,
        bank -> Text,
        juspay_bank_code_id -> Nullable<Bigint>,
        scope -> Nullable<Text>,
    }
}

diesel::table! {
    gateway_bank_emi_support_v2 (id) {
        id -> Bigint,
        version -> Bigint,
        #[max_length = 255]
        gateway -> Varchar,
        juspay_bank_code_id -> Bigint,
        #[max_length = 255]
        card_type -> Varchar,
        tenure -> Integer,
        #[max_length = 255]
        gateway_emi_code -> Varchar,
        #[max_length = 255]
        gateway_plan_id -> Nullable<Varchar>,
        #[max_length = 255]
        scope -> Varchar,
        metadata -> Nullable<Text>,
        date_created -> Nullable<Datetime>,
        last_updated -> Nullable<Datetime>,
    }
}

diesel::table! {
    gateway_card_info (id) {
        id -> Bigint,
        isin -> Nullable<Text>,
        gateway -> Nullable<Text>,
        card_issuer_bank_name -> Nullable<Text>,
        auth_type -> Nullable<Text>,
        juspay_bank_code_id -> Nullable<Bigint>,
        disabled -> Nullable<Bit>,
        validation_type -> Nullable<Text>,
        payment_method_type -> Nullable<Text>,
    }
}

diesel::table! {
    gateway_outage (id) {
        #[max_length = 255]
        id -> Varchar,
        version -> Integer,
        end_time -> Datetime,
        #[max_length = 255]
        gateway -> Nullable<Varchar>,
        #[max_length = 255]
        merchant_id -> Nullable<Varchar>,
        start_time -> Datetime,
        #[max_length = 255]
        bank -> Nullable<Varchar>,
        #[max_length = 255]
        payment_method_type -> Nullable<Varchar>,
        #[max_length = 255]
        payment_method -> Nullable<Varchar>,
        description -> Nullable<Text>,
        date_created -> Nullable<Datetime>,
        last_updated -> Nullable<Datetime>,
        juspay_bank_code_id -> Nullable<Bigint>,
        metadata -> Nullable<Text>,
    }
}

diesel::table! {
    gateway_payment_method_flow (id) {
        id -> Text,
        gateway_payment_flow_id -> Text,
        payment_method_id -> Nullable<Bigint>,
        date_created -> Datetime,
        last_updated -> Datetime,
        gateway -> Text,
        payment_flow_id -> Text,
        juspay_bank_code_id -> Nullable<Bigint>,
        gateway_bank_code -> Nullable<Text>,
        currency_configs -> Nullable<Text>,
        #[sql_name = "dsl"]
        gateway_dsl -> Nullable<Text>,
        #[sql_name = "non_combinable_flows"]
        non_combination_flows -> Nullable<Text>,
        #[sql_name = "country_code_alpha_3"]
        country_code_alpha3 -> Nullable<Text>,
        disabled -> Bit,
        payment_method_type -> Nullable<Text>,
    }
}

diesel::table! {
    isin_routes (id) {
        id -> Bigint,
        isin -> Text,
        merchant_id -> Text,
        preferred_gateway -> Text,
        preference_score -> Double,
        date_created -> Datetime,
        last_updated -> Datetime,
    }
}

diesel::table! {
    issuer_routes (id) {
        id -> Bigint,
        issuer -> Text,
        merchant_id -> Text,
        preferred_gateway -> Text,
        preference_score -> Double,
        date_created -> Datetime,
        last_updated -> Datetime,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    juspay_bank_code (id) {
        id -> Bigint,
        bank_code -> Text,
        bank_name -> Text,
    }
}

diesel::table! {
    merchant_account (id) {
        id -> Bigint,
        merchant_id -> Nullable<Text>,
        date_created -> Datetime,
        gateway_decided_by_health_enabled -> Nullable<Bit>,
        gateway_priority -> Nullable<Text>,
        gateway_priority_logic -> Nullable<Text>,
        internal_hash_key -> Nullable<Text>,
        locker_id -> Nullable<Text>,
        token_locker_id -> Nullable<Text>,
        user_id -> Nullable<Bigint>,
        settlement_account_id -> Nullable<Bigint>,
        secondary_merchant_account_id -> Nullable<Bigint>,
        use_code_for_gateway_priority -> Bit,
        enable_gateway_reference_id_based_routing -> Nullable<Bit>,
        gateway_success_rate_based_decider_input -> Nullable<Text>,
        internal_metadata -> Nullable<Text>,
        enabled -> Bit,
        country -> Nullable<Text>,
        installment_enabled -> Nullable<Bit>,
        tenant_account_id -> Nullable<Text>,
        priority_logic_config -> Nullable<Text>,
        merchant_category_code -> Nullable<Text>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    merchant_config (id) {
        id -> Text,
        merchant_account_id -> Bigint,
        config_category -> Text,
        config_name -> Text,
        status -> Text,
        config_value -> Nullable<Text>,
        date_created -> Datetime,
        last_updated -> Datetime,
    }
}

diesel::table! {
    merchant_gateway_account (id) {
        id -> Bigint,
        account_details -> Text,
        gateway -> Text,
        merchant_id -> Text,
        payment_methods -> Nullable<Text>,
        supported_payment_flows -> Nullable<Text>,
        disabled -> Nullable<Bit>,
        reference_id -> Nullable<Text>,
        supported_currencies -> Nullable<Text>,
        gateway_identifier -> Nullable<Text>,
        gateway_type -> Nullable<Text>,
        supported_txn_type -> Nullable<Text>,
    }
}

diesel::table! {
    merchant_gateway_account_sub_info (id) {
        id -> Bigint,
        merchant_gateway_account_id -> Bigint,
        sub_info_type -> Text,
        sub_id_type -> Text,
        juspay_sub_account_id -> Text,
        gateway_sub_account_id -> Text,
        disabled -> Bit,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    merchant_gateway_card_info (id) {
        id -> Bigint,
        disabled -> Bit,
        gateway_card_info_id -> Bigint,
        merchant_account_id -> Bigint,
        emandate_register_max_amount -> Nullable<Double>,
        merchant_gateway_account_id -> Nullable<Bigint>,
    }
}

diesel::table! {
    merchant_gateway_payment_method_flow (id) {
        id -> Bigint,
        gateway_payment_method_flow_id -> Text,
        merchant_gateway_account_id -> Bigint,
        currency_configs -> Nullable<Text>,
        date_created -> Datetime,
        last_updated -> Datetime,
        disabled -> Nullable<Bit>,
        gateway_bank_code -> Nullable<Text>,
    }
}

diesel::table! {
    merchant_iframe_preferences (id) {
        id -> Bigint,
        merchant_id -> Text,
        dynamic_switching_enabled -> Nullable<Bit>,
        isin_routing_enabled -> Nullable<Bit>,
        issuer_routing_enabled -> Nullable<Bit>,
        txn_failure_gateway_penality -> Nullable<Bit>,
        card_brand_routing_enabled -> Nullable<Bit>,
    }
}

diesel::table! {
    merchant_priority_logic (id) {
        #[max_length = 255]
        id -> Varchar,
        version -> Bigint,
        date_created -> Datetime,
        last_updated -> Datetime,
        merchant_account_id -> Bigint,
        #[max_length = 255]
        status -> Varchar,
        priority_logic -> Text,
        #[max_length = 255]
        name -> Nullable<Varchar>,
        description -> Nullable<Text>,
        priority_logic_rules -> Nullable<Text>,
        is_active_logic -> Bit,
    }
}

diesel::table! {
    payment_method (id) {
        id -> Bigint,
        date_created -> Datetime,
        last_updated -> Datetime,
        name -> Text,
        #[sql_name = "type"]
        pm_type -> Text,
        description -> Nullable<Text>,
        juspay_bank_code_id -> Nullable<Bigint>,
        display_name -> Nullable<Text>,
        nick_name -> Nullable<Text>,
        sub_type -> Nullable<Text>,
        #[sql_name = "dsl"]
        payment_dsl -> Nullable<Text>,
    }
}

diesel::table! {
    service_configuration (id) {
        id -> Bigint,
        name -> Text,
        value -> Nullable<Text>,
        new_value -> Nullable<Text>,
        previous_value -> Nullable<Text>,
        new_value_status -> Nullable<Text>,
    }
}

diesel::table! {
    tenant_config (id) {
        #[max_length = 255]
        id -> Varchar,
        #[max_length = 255]
        #[sql_name = "type"]
        tenant_type -> Varchar,
        #[max_length = 255]
        module_key -> Varchar,
        #[max_length = 255]
        module_name -> Varchar,
        #[max_length = 255]
        tenant_account_id -> Varchar,
        config_value -> Text,
        #[max_length = 255]
        filter_dimension -> Nullable<Varchar>,
        #[max_length = 255]
        filter_group_id -> Nullable<Varchar>,
        #[max_length = 255]
        status -> Varchar,
        #[max_length = 3]
        #[sql_name = "country_code_alpha_3"]
        country_code_alpha3 -> Nullable<Varchar>,
    }
}

diesel::table! {
    tenant_config_filter (id) {
        #[max_length = 255]
        id -> Varchar,
        #[max_length = 255]
        filter_group_id -> Varchar,
        #[max_length = 255]
        dimension_value -> Varchar,
        #[max_length = 255]
        config_value -> Varchar,
        #[max_length = 255]
        tenant_config_id -> Varchar
    }
}

diesel::table! {
    token_bin_info (token_bin) {
        token_bin -> Text,
        card_bin -> Text,
        provider -> Text,
        date_created -> Nullable<Datetime>,
        last_updated -> Nullable<Datetime>,
    }
}

diesel::table! {
    txn_card_info (id) {
        id -> Bigint,
        txn_id -> Text,
        card_isin -> Nullable<Text>,
        card_issuer_bank_name -> Nullable<Text>,
        card_switch_provider -> Nullable<Text>,
        card_type -> Nullable<Text>,
        name_on_card -> Nullable<Text>,
        txn_detail_id -> Nullable<Bigint>,
        date_created -> Nullable<Datetime>,
        payment_method_type -> Nullable<Text>,
        payment_method -> Nullable<Text>,
        payment_source -> Nullable<Text>,
        auth_type -> Nullable<Text>,
        partition_key -> Nullable<Datetime>,
    }
}

diesel::table! {
    txn_detail (id) {
        id -> Bigint,
        order_id -> Text,
        status -> Text,
        txn_id -> Text,
        txn_type -> Text,
        date_created -> Nullable<Datetime>,
        add_to_locker -> Nullable<Bit>,
        merchant_id -> Nullable<Text>,
        gateway -> Nullable<Text>,
        express_checkout -> Nullable<Bit>,
        is_emi -> Nullable<Bit>,
        emi_bank -> Nullable<Text>,
        emi_tenure -> Nullable<Integer>,
        txn_uuid -> Nullable<Text>,
        merchant_gateway_account_id -> Nullable<Bigint>,
        net_amount -> Nullable<Double>,
        txn_amount -> Nullable<Double>,
        txn_object_type -> Nullable<Text>,
        source_object -> Nullable<Text>,
        source_object_id -> Nullable<Text>,
        currency -> Nullable<Text>,
        surcharge_amount -> Nullable<Double>,
        tax_amount -> Nullable<Double>,
        internal_metadata -> Nullable<Text>,
        metadata -> Nullable<Text>,
        offer_deduction_amount -> Nullable<Double>,
        internal_tracking_info -> Nullable<Text>,
        partition_key -> Nullable<Datetime>,
        txn_amount_breakup -> Nullable<Text>,
    }
}

diesel::table! {
    txn_offer (id) {
        id -> Bigint,
        version -> Bigint,
        discount_amount -> Bigint,
        offer_id -> Text,
        signature -> Text,
        txn_detail_id -> Bigint,
    }
}

diesel::table! {
    txn_offer_detail (id) {
        id -> Text,
        txn_detail_id -> Text,
        offer_id -> Text,
        status -> Text,
        date_created -> Nullable<Datetime>,
        last_updated -> Nullable<Datetime>,
        gateway_info -> Nullable<Text>,
        internal_metadata -> Nullable<Text>,
        partition_key -> Nullable<Datetime>,
    }
}

diesel::table! {
    user_eligibility_info (id) {
        id -> Text,
        flow_type -> Text,
        identifier_name -> Text,
        identifier_value -> Text,
        provider_name -> Text,
        disabled -> Nullable<Bit>,
    }
}

diesel::table! {
    routing_algorithm (id) {
        id -> Text,
        created_by -> Text,
        name -> Text,
        description -> Text,
        algorithm_data -> Text,
        #[max_length = 64]
        algorithm_for -> Varchar,
        metadata -> Nullable<Text>,
        created_at -> Timestamp,
        modified_at -> Timestamp,
    }
}

diesel::table! {
    routing_algorithm_mapper (id) {
        id -> Int4,
        #[max_length = 255]
        created_by -> Varchar,
        #[max_length = 255]
        routing_algorithm_id -> Varchar,
        #[max_length = 64]
        algorithm_for -> Varchar,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    card_brand_routes,
    card_info,
    emi_bank_code,
    feature,
    gateway_bank_emi_support,
    gateway_bank_emi_support_v2,
    gateway_card_info,
    gateway_outage,
    gateway_payment_method_flow,
    isin_routes,
    issuer_routes,
    juspay_bank_code,
    merchant_account,
    merchant_config,
    merchant_gateway_account,
    merchant_gateway_account_sub_info,
    merchant_gateway_card_info,
    merchant_gateway_payment_method_flow,
    merchant_iframe_preferences,
    merchant_priority_logic,
    payment_method,
    service_configuration,
    tenant_config,
    token_bin_info,
    txn_card_info,
    txn_detail,
    txn_offer,
    txn_offer_detail,
    user_eligibility_info,
);
