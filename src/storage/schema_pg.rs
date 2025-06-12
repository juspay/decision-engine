// @generated automatically by Diesel CLI.

diesel::table! {
    card_brand_routes (id) {
        id -> Int8,
        card_brand -> Text,
        date_created -> Timestamp,
        last_updated -> Timestamp,
        merchant_account_id -> Int8,
        preference_score -> Float8,
        preferred_gateway -> Text,
    }
}

diesel::table! {
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
    co_badged_cards_info_test (id) {
        #[max_length = 64]
        id -> Varchar,
        card_bin_min -> Int8,
        card_bin_max -> Int8,
        issuing_bank_name -> Nullable<Text>,
        #[max_length = 32]
        card_network -> Varchar,
        country_code -> Nullable<Text>,
        card_type -> Nullable<Text>,
        regulated -> Nullable<Bool>,
        regulated_name -> Nullable<Text>,
        prepaid -> Nullable<Bool>,
        reloadable -> Nullable<Bool>,
        pan_or_token -> Text,
        card_bin_length -> Int2,
        bin_provider_bin_length -> Int2,
        card_brand_is_additional -> Bool,
        domestic_only -> Nullable<Bool>,
        created_at -> Timestamp,
        modified_at -> Timestamp,
        #[max_length = 128]
        last_updated_provider -> Nullable<Varchar>,
    }
}

diesel::table! {
    emi_bank_code (id) {
        id -> Int8,
        emi_bank -> Text,
        juspay_bank_code_id -> Int8,
        last_updated -> Nullable<Timestamp>,
    }
}

diesel::table! {
    feature (id) {
        id -> Int4,
        enabled -> Bool,
        name -> Text,
        merchant_id -> Nullable<Text>,
    }
}

diesel::table! {
    gateway_bank_emi_support (id) {
        id -> Int8,
        gateway -> Text,
        bank -> Text,
        juspay_bank_code_id -> Nullable<Int8>,
        scope -> Nullable<Text>,
    }
}

diesel::table! {
    gateway_bank_emi_support_v2 (id) {
        id -> Int8,
        version -> Int8,
        #[max_length = 255]
        gateway -> Varchar,
        juspay_bank_code_id -> Int8,
        #[max_length = 255]
        card_type -> Varchar,
        tenure -> Int4,
        #[max_length = 255]
        gateway_emi_code -> Varchar,
        #[max_length = 255]
        gateway_plan_id -> Nullable<Varchar>,
        #[max_length = 255]
        scope -> Varchar,
        metadata -> Nullable<Text>,
        date_created -> Nullable<Timestamp>,
        last_updated -> Nullable<Timestamp>,
    }
}

diesel::table! {
    gateway_card_info (id) {
        id -> Int8,
        isin -> Nullable<Text>,
        gateway -> Nullable<Text>,
        card_issuer_bank_name -> Nullable<Text>,
        auth_type -> Nullable<Text>,
        juspay_bank_code_id -> Nullable<Int8>,
        disabled -> Nullable<Bool>,
        validation_type -> Nullable<Text>,
        payment_method_type -> Nullable<Text>,
    }
}

diesel::table! {
    gateway_outage (id) {
        #[max_length = 255]
        id -> Varchar,
        version -> Int4,
        end_time -> Timestamp,
        #[max_length = 255]
        gateway -> Nullable<Varchar>,
        #[max_length = 255]
        merchant_id -> Nullable<Varchar>,
        start_time -> Timestamp,
        #[max_length = 255]
        bank -> Nullable<Varchar>,
        #[max_length = 255]
        payment_method_type -> Nullable<Varchar>,
        #[max_length = 255]
        payment_method -> Nullable<Varchar>,
        description -> Nullable<Text>,
        date_created -> Nullable<Timestamp>,
        last_updated -> Nullable<Timestamp>,
        juspay_bank_code_id -> Nullable<Int8>,
        metadata -> Nullable<Text>,
    }
}

diesel::table! {
    gateway_payment_method_flow (id) {
        id -> Text,
        gateway_payment_flow_id -> Text,
        payment_method_id -> Nullable<Int8>,
        date_created -> Timestamp,
        last_updated -> Timestamp,
        gateway -> Text,
        payment_flow_id -> Text,
        juspay_bank_code_id -> Nullable<Int8>,
        gateway_bank_code -> Nullable<Text>,
        currency_configs -> Nullable<Text>,
        gateway_dsl -> Nullable<Text>,
        non_combination_flows -> Nullable<Text>,
        country_code_alpha3 -> Nullable<Text>,
        disabled -> Bool,
        payment_method_type -> Nullable<Text>,
    }
}

diesel::table! {
    isin_routes (id) {
        id -> Int8,
        isin -> Text,
        merchant_id -> Text,
        preferred_gateway -> Text,
        preference_score -> Float8,
        date_created -> Timestamp,
        last_updated -> Timestamp,
    }
}

diesel::table! {
    issuer_routes (id) {
        id -> Int8,
        issuer -> Text,
        merchant_id -> Text,
        preferred_gateway -> Text,
        preference_score -> Float8,
        date_created -> Timestamp,
        last_updated -> Timestamp,
    }
}

diesel::table! {
    juspay_bank_code (id) {
        id -> Int8,
        bank_code -> Text,
        bank_name -> Text,
    }
}

diesel::table! {
    merchant_account (id) {
        id -> Int8,
        merchant_id -> Nullable<Text>,
        date_created -> Timestamp,
        gateway_decided_by_health_enabled -> Nullable<Bool>,
        gateway_priority -> Nullable<Text>,
        gateway_priority_logic -> Nullable<Text>,
        internal_hash_key -> Nullable<Text>,
        locker_id -> Nullable<Text>,
        token_locker_id -> Nullable<Text>,
        user_id -> Nullable<Int8>,
        settlement_account_id -> Nullable<Int8>,
        secondary_merchant_account_id -> Nullable<Int8>,
        use_code_for_gateway_priority -> Bool,
        enable_gateway_reference_id_based_routing -> Nullable<Bool>,
        gateway_success_rate_based_decider_input -> Nullable<Text>,
        internal_metadata -> Nullable<Text>,
        enabled -> Bool,
        country -> Nullable<Text>,
        installment_enabled -> Nullable<Bool>,
        tenant_account_id -> Nullable<Text>,
        priority_logic_config -> Nullable<Text>,
        merchant_category_code -> Nullable<Text>,
    }
}

diesel::table! {
    merchant_config (id) {
        id -> Text,
        merchant_account_id -> Int8,
        config_category -> Text,
        config_name -> Text,
        status -> Text,
        config_value -> Nullable<Text>,
        date_created -> Timestamp,
        last_updated -> Timestamp,
    }
}

diesel::table! {
    merchant_gateway_account (id) {
        id -> Int8,
        account_details -> Text,
        gateway -> Text,
        merchant_id -> Text,
        payment_methods -> Nullable<Text>,
        supported_payment_flows -> Nullable<Text>,
        disabled -> Nullable<Bool>,
        reference_id -> Nullable<Text>,
        supported_currencies -> Nullable<Text>,
        gateway_identifier -> Nullable<Text>,
        gateway_type -> Nullable<Text>,
        supported_txn_type -> Nullable<Text>,
    }
}

diesel::table! {
    merchant_gateway_account_sub_info (id) {
        id -> Int8,
        merchant_gateway_account_id -> Int8,
        sub_info_type -> Text,
        sub_id_type -> Text,
        juspay_sub_account_id -> Text,
        gateway_sub_account_id -> Text,
        disabled -> Bool,
    }
}

diesel::table! {
    merchant_gateway_card_info (id) {
        id -> Int8,
        disabled -> Bool,
        gateway_card_info_id -> Int8,
        merchant_account_id -> Int8,
        emandate_register_max_amount -> Nullable<Float8>,
        merchant_gateway_account_id -> Nullable<Int8>,
    }
}

diesel::table! {
    merchant_gateway_payment_method_flow (id) {
        id -> Int8,
        gateway_payment_method_flow_id -> Text,
        merchant_gateway_account_id -> Int8,
        currency_configs -> Nullable<Text>,
        date_created -> Timestamp,
        last_updated -> Timestamp,
        disabled -> Nullable<Bool>,
        gateway_bank_code -> Nullable<Text>,
    }
}

diesel::table! {
    merchant_iframe_preferences (id) {
        id -> Int8,
        merchant_id -> Text,
        dynamic_switching_enabled -> Nullable<Bool>,
        isin_routing_enabled -> Nullable<Bool>,
        issuer_routing_enabled -> Nullable<Bool>,
        txn_failure_gateway_penality -> Nullable<Bool>,
        card_brand_routing_enabled -> Nullable<Bool>,
    }
}

diesel::table! {
    merchant_priority_logic (id) {
        #[max_length = 255]
        id -> Varchar,
        version -> Int8,
        date_created -> Timestamp,
        last_updated -> Timestamp,
        merchant_account_id -> Int8,
        #[max_length = 255]
        status -> Varchar,
        priority_logic -> Text,
        #[max_length = 255]
        name -> Nullable<Varchar>,
        description -> Nullable<Text>,
        priority_logic_rules -> Nullable<Text>,
        is_active_logic -> Bool,
    }
}

diesel::table! {
    payment_method (id) {
        id -> Int8,
        date_created -> Timestamp,
        last_updated -> Timestamp,
        name -> Text,
        #[sql_name = "type"]
        type_ -> Text,
        description -> Nullable<Text>,
        juspay_bank_code_id -> Nullable<Int8>,
        display_name -> Nullable<Text>,
        nick_name -> Nullable<Text>,
        sub_type -> Nullable<Text>,
        payment_dsl -> Nullable<Text>,
    }
}

diesel::table! {
    routing_algorithm (id) {
        #[max_length = 255]
        id -> Varchar,
        #[max_length = 255]
        created_by -> Varchar,
        #[max_length = 255]
        name -> Varchar,
        description -> Text,
        algorithm_data -> Text,
        metadata -> Nullable<Jsonb>,
        created_at -> Timestamp,
        modified_at -> Timestamp,
    }
}

diesel::table! {
    routing_algorithm_mapper (created_by) {
        #[max_length = 255]
        created_by -> Varchar,
        #[max_length = 255]
        routing_algorithm_id -> Varchar,
    }
}

diesel::table! {
    service_configuration (id) {
        id -> Int8,
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
        tenant_config_id -> Varchar,
    }
}

diesel::table! {
    token_bin_info (token_bin) {
        token_bin -> Text,
        card_bin -> Text,
        provider -> Text,
        date_created -> Nullable<Timestamp>,
        last_updated -> Nullable<Timestamp>,
    }
}

diesel::table! {
    txn_card_info (id) {
        id -> Int8,
        txn_id -> Text,
        card_isin -> Nullable<Text>,
        card_issuer_bank_name -> Nullable<Text>,
        card_switch_provider -> Nullable<Text>,
        card_type -> Nullable<Text>,
        name_on_card -> Nullable<Text>,
        txn_detail_id -> Nullable<Int8>,
        date_created -> Nullable<Timestamp>,
        payment_method_type -> Nullable<Text>,
        payment_method -> Nullable<Text>,
        payment_source -> Nullable<Text>,
        auth_type -> Nullable<Text>,
        partition_key -> Nullable<Timestamp>,
    }
}

diesel::table! {
    txn_detail (id) {
        id -> Int8,
        order_id -> Text,
        status -> Text,
        txn_id -> Text,
        txn_type -> Text,
        date_created -> Nullable<Timestamp>,
        add_to_locker -> Nullable<Bool>,
        merchant_id -> Nullable<Text>,
        gateway -> Nullable<Text>,
        express_checkout -> Nullable<Bool>,
        is_emi -> Nullable<Bool>,
        emi_bank -> Nullable<Text>,
        emi_tenure -> Nullable<Int4>,
        txn_uuid -> Nullable<Text>,
        merchant_gateway_account_id -> Nullable<Int8>,
        net_amount -> Nullable<Float8>,
        txn_amount -> Nullable<Float8>,
        txn_object_type -> Nullable<Text>,
        source_object -> Nullable<Text>,
        source_object_id -> Nullable<Text>,
        currency -> Nullable<Text>,
        surcharge_amount -> Nullable<Float8>,
        tax_amount -> Nullable<Float8>,
        internal_metadata -> Nullable<Text>,
        metadata -> Nullable<Text>,
        offer_deduction_amount -> Nullable<Float8>,
        internal_tracking_info -> Nullable<Text>,
        partition_key -> Nullable<Timestamp>,
        txn_amount_breakup -> Nullable<Text>,
    }
}

diesel::table! {
    txn_offer (id) {
        id -> Int8,
        version -> Int8,
        discount_amount -> Int8,
        offer_id -> Text,
        signature -> Text,
        txn_detail_id -> Int8,
    }
}

diesel::table! {
    txn_offer_detail (id) {
        id -> Text,
        txn_detail_id -> Text,
        offer_id -> Text,
        status -> Text,
        date_created -> Nullable<Timestamp>,
        last_updated -> Nullable<Timestamp>,
        gateway_info -> Nullable<Text>,
        internal_metadata -> Nullable<Text>,
        partition_key -> Nullable<Timestamp>,
    }
}

diesel::table! {
    user_eligibility_info (id) {
        id -> Text,
        flow_type -> Text,
        identifier_name -> Text,
        identifier_value -> Text,
        provider_name -> Text,
        disabled -> Nullable<Bool>,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    card_brand_routes,
    card_info,
    co_badged_cards_info_test,
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
    routing_algorithm,
    routing_algorithm_mapper,
    service_configuration,
    tenant_config,
    tenant_config_filter,
    token_bin_info,
    txn_card_info,
    txn_detail,
    txn_offer,
    txn_offer_detail,
    user_eligibility_info,
);
