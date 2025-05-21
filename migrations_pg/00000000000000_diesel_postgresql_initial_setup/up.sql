-- DROP DATABASE IF EXISTS decision_engine_db;
-- CREATE DATABASE decision_engine_db;
-- \c decision_engine_db

DROP TABLE IF EXISTS gateway_bank_emi_support_v2;
CREATE TABLE gateway_bank_emi_support_v2 (
    id BIGSERIAL PRIMARY KEY,
    version BIGINT NOT NULL,
    gateway VARCHAR(255) NOT NULL,
    juspay_bank_code_id BIGINT NOT NULL,
    card_type VARCHAR(255) NOT NULL,
    tenure INTEGER NOT NULL,
    gateway_emi_code VARCHAR(255) NOT NULL,
    gateway_plan_id VARCHAR(255),
    scope VARCHAR(255) NOT NULL,
    metadata TEXT,
    date_created TIMESTAMP,
    last_updated TIMESTAMP
);

DROP TABLE IF EXISTS gateway_outage;
CREATE TABLE gateway_outage (
    id VARCHAR(255) PRIMARY KEY,
    version INTEGER NOT NULL,
    end_time TIMESTAMP NOT NULL,
    gateway VARCHAR(255),
    merchant_id VARCHAR(255),
    start_time TIMESTAMP NOT NULL,
    bank VARCHAR(255),
    payment_method_type VARCHAR(255),
    payment_method VARCHAR(255),
    description TEXT,
    date_created TIMESTAMP,
    last_updated TIMESTAMP,
    juspay_bank_code_id BIGINT,
    metadata TEXT
);

DROP TABLE IF EXISTS merchant_priority_logic;
CREATE TABLE merchant_priority_logic (
    id VARCHAR(255) PRIMARY KEY,
    version BIGINT NOT NULL,
    date_created TIMESTAMP NOT NULL,
    last_updated TIMESTAMP NOT NULL,
    merchant_account_id BIGINT NOT NULL,
    status VARCHAR(255) NOT NULL,
    priority_logic TEXT NOT NULL,
    name VARCHAR(255),
    description TEXT,
    priority_logic_rules TEXT,
    is_active_logic BOOLEAN NOT NULL
);

DROP TABLE IF EXISTS tenant_config;
CREATE TABLE tenant_config (
    id VARCHAR(255) PRIMARY KEY,
    type VARCHAR(255) NOT NULL,
    module_key VARCHAR(255) NOT NULL,
    module_name VARCHAR(255) NOT NULL,
    tenant_account_id VARCHAR(255) NOT NULL,
    config_value TEXT NOT NULL,
    filter_dimension VARCHAR(255),
    filter_group_id VARCHAR(255),
    status VARCHAR(255) NOT NULL,
    country_code_alpha3 VARCHAR(3)
);

DROP TABLE IF EXISTS card_brand_routes;
CREATE TABLE card_brand_routes (
    id BIGSERIAL PRIMARY KEY,
    card_brand TEXT NOT NULL,
    date_created TIMESTAMP NOT NULL,
    last_updated TIMESTAMP NOT NULL,
    merchant_account_id BIGINT NOT NULL,
    preference_score DOUBLE PRECISION NOT NULL,
    preferred_gateway TEXT NOT NULL
);

DROP TABLE IF EXISTS issuer_routes;
CREATE TABLE issuer_routes (
    id BIGSERIAL PRIMARY KEY,
    issuer TEXT NOT NULL,
    merchant_id TEXT NOT NULL,
    preferred_gateway TEXT NOT NULL,
    preference_score DOUBLE PRECISION NOT NULL,
    date_created TIMESTAMP NOT NULL,
    last_updated TIMESTAMP NOT NULL
);

DROP TABLE IF EXISTS merchant_gateway_account_sub_info;
CREATE TABLE merchant_gateway_account_sub_info (
    id BIGSERIAL PRIMARY KEY,
    merchant_gateway_account_id BIGINT NOT NULL,
    sub_info_type TEXT NOT NULL,
    sub_id_type TEXT NOT NULL,
    juspay_sub_account_id TEXT NOT NULL,
    gateway_sub_account_id TEXT NOT NULL,
    disabled BOOLEAN NOT NULL
);

DROP TABLE IF EXISTS gateway_payment_method_flow;
CREATE TABLE gateway_payment_method_flow (
    id TEXT PRIMARY KEY,
    gateway_payment_flow_id TEXT NOT NULL,
    payment_method_id BIGINT,
    date_created TIMESTAMP NOT NULL,
    last_updated TIMESTAMP NOT NULL,
    gateway TEXT NOT NULL,
    payment_flow_id TEXT NOT NULL,
    juspay_bank_code_id BIGINT,
    gateway_bank_code TEXT,
    currency_configs TEXT,
    gateway_dsl TEXT,
    non_combinable_flows TEXT,
    country_code_alpha_3 TEXT,
    disabled BOOLEAN NOT NULL,
    payment_method_type TEXT
);

DROP TABLE IF EXISTS merchant_iframe_preferences;
CREATE TABLE merchant_iframe_preferences (
    id SERIAL PRIMARY KEY,
    merchant_id TEXT NOT NULL,
    dynamic_switching_enabled BOOLEAN,
    isin_routing_enabled BOOLEAN,
    issuer_routing_enabled BOOLEAN,
    txn_failure_gateway_penality BOOLEAN,
    card_brand_routing_enabled BOOLEAN
);

DROP TABLE IF EXISTS token_bin_info;
CREATE TABLE token_bin_info (
    token_bin TEXT NOT NULL,
    card_bin TEXT NOT NULL,
    provider TEXT NOT NULL,
    date_created TIMESTAMP,
    last_updated TIMESTAMP
);

DROP TABLE IF EXISTS txn_offer_detail;
CREATE TABLE txn_offer_detail (
    id TEXT PRIMARY KEY,
    txn_detail_id TEXT NOT NULL,
    offer_id TEXT NOT NULL,
    status TEXT NOT NULL,
    date_created TIMESTAMP,
    last_updated TIMESTAMP,
    gateway_info TEXT,
    internal_metadata TEXT,
    partition_key TIMESTAMP
);

DROP TABLE IF EXISTS merchant_gateway_card_info;
CREATE TABLE merchant_gateway_card_info (
    id BIGSERIAL PRIMARY KEY,
    disabled BOOLEAN NOT NULL,
    gateway_card_info_id BIGINT NOT NULL,
    merchant_account_id BIGINT NOT NULL,
    emandate_register_max_amount DOUBLE PRECISION,
    merchant_gateway_account_id BIGINT
);

DROP TABLE IF EXISTS merchant_account;
CREATE TABLE merchant_account (
    id BIGSERIAL PRIMARY KEY,
    merchant_id TEXT,
    date_created TIMESTAMP NOT NULL,
    gateway_decided_by_health_enabled BOOLEAN,
    gateway_priority TEXT,
    gateway_priority_logic TEXT,
    internal_hash_key TEXT,
    locker_id TEXT,
    token_locker_id TEXT,
    user_id BIGINT,
    settlement_account_id BIGINT,
    secondary_merchant_account_id BIGINT,
    use_code_for_gateway_priority BOOLEAN NOT NULL,
    enable_gateway_reference_id_based_routing BOOLEAN,
    gateway_success_rate_based_decider_input TEXT,
    internal_metadata TEXT,
    enabled BOOLEAN NOT NULL,
    country TEXT,
    installment_enabled BOOLEAN,
    tenant_account_id TEXT,
    priority_logic_config TEXT,
    merchant_category_code TEXT
);

DROP TABLE IF EXISTS merchant_gateway_payment_method_flow;
CREATE TABLE merchant_gateway_payment_method_flow (
    id BIGSERIAL PRIMARY KEY,
    gateway_payment_method_flow_id TEXT NOT NULL,
    merchant_gateway_account_id BIGINT NOT NULL,
    currency_configs TEXT,
    date_created TIMESTAMP NOT NULL,
    last_updated TIMESTAMP NOT NULL,
    disabled BOOLEAN,
    gateway_bank_code TEXT
);

DROP TABLE IF EXISTS txn_offer;
CREATE TABLE txn_offer (
    id BIGSERIAL PRIMARY KEY,
    version BIGINT NOT NULL,
    discount_amount BIGINT NOT NULL,
    offer_id TEXT NOT NULL,
    signature TEXT NOT NULL,
    txn_detail_id BIGINT NOT NULL
);

DROP TABLE IF EXISTS isin_routes;
CREATE TABLE isin_routes (
    id BIGSERIAL PRIMARY KEY,
    isin TEXT NOT NULL,
    merchant_id TEXT NOT NULL,
    preferred_gateway TEXT NOT NULL,
    preference_score DOUBLE PRECISION NOT NULL,
    date_created TIMESTAMP NOT NULL,
    last_updated TIMESTAMP NOT NULL
);

DROP TABLE IF EXISTS feature;
CREATE TABLE feature (
    id SERIAL PRIMARY KEY,
    enabled BOOLEAN NOT NULL,
    name TEXT NOT NULL,
    merchant_id TEXT
);

DROP TABLE IF EXISTS merchant_config;
CREATE TABLE merchant_config (
    id TEXT PRIMARY KEY,
    merchant_account_id BIGINT NOT NULL,
    config_category TEXT NOT NULL,
    config_name TEXT NOT NULL,
    status TEXT NOT NULL,
    config_value TEXT,
    date_created TIMESTAMP NOT NULL,
    last_updated TIMESTAMP NOT NULL
);

DROP TABLE IF EXISTS service_configuration;
CREATE TABLE service_configuration (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    value TEXT,
    new_value TEXT,
    previous_value TEXT,
    new_value_status TEXT
);

DROP TABLE IF EXISTS payment_method;
CREATE TABLE payment_method (
    id BIGSERIAL PRIMARY KEY,
    date_created TIMESTAMP NOT NULL,
    last_updated TIMESTAMP NOT NULL,
    name TEXT NOT NULL,
    type TEXT NOT NULL,
    description TEXT,
    juspay_bank_code_id BIGINT,
    display_name TEXT,
    nick_name TEXT,
    sub_type TEXT,
    payment_dsl TEXT
);

DROP TABLE IF EXISTS txn_card_info;
CREATE TABLE txn_card_info (
    id BIGSERIAL PRIMARY KEY,
    txn_id TEXT NOT NULL,
    card_isin TEXT,
    card_issuer_bank_name TEXT,
    card_switch_provider TEXT,
    card_type TEXT,
    name_on_card TEXT,
    txn_detail_id BIGINT,
    date_created TIMESTAMP,
    payment_method_type TEXT,
    payment_method TEXT,
    payment_source TEXT,
    auth_type TEXT,
    partition_key TIMESTAMP
);

DROP TABLE IF EXISTS txn_detail;
CREATE TABLE txn_detail (
    id BIGSERIAL PRIMARY KEY,
    order_id TEXT NOT NULL,
    status TEXT NOT NULL,
    txn_id TEXT NOT NULL,
    txn_type TEXT NOT NULL,
    date_created TIMESTAMP,
    add_to_locker BOOLEAN,
    merchant_id TEXT,
    gateway TEXT,
    express_checkout BOOLEAN,
    is_emi BOOLEAN,
    emi_bank TEXT,
    emi_tenure INT,
    txn_uuid TEXT,
    merchant_gateway_account_id BIGINT,
    net_amount DOUBLE PRECISION,
    txn_amount DOUBLE PRECISION,
    txn_object_type TEXT,
    source_object TEXT,
    source_object_id TEXT,
    currency TEXT,
    surcharge_amount DOUBLE PRECISION,
    tax_amount DOUBLE PRECISION,
    internal_metadata TEXT,
    metadata TEXT,
    offer_deduction_amount DOUBLE PRECISION,
    internal_tracking_info TEXT,
    partition_key TIMESTAMP,
    txn_amount_breakup TEXT
);

DROP TABLE IF EXISTS card_info;
CREATE TABLE card_info (
    card_isin TEXT PRIMARY KEY,
    card_switch_provider TEXT NOT NULL,
    card_type TEXT,
    card_sub_type TEXT,
    card_sub_type_category TEXT,
    card_issuer_country TEXT,
    country_code TEXT,
    extended_card_type TEXT
);

DROP TABLE IF EXISTS gateway_card_info;
CREATE TABLE gateway_card_info (
    id BIGSERIAL PRIMARY KEY,
    isin TEXT,
    gateway TEXT,
    card_issuer_bank_name TEXT,
    auth_type TEXT,
    juspay_bank_code_id BIGINT,
    disabled BOOLEAN,
    validation_type TEXT,
    payment_method_type TEXT
);

DROP TABLE IF EXISTS juspay_bank_code;
CREATE TABLE juspay_bank_code (
    id BIGSERIAL PRIMARY KEY,
    bank_code TEXT NOT NULL,
    bank_name TEXT NOT NULL
);

DROP TABLE IF EXISTS emi_bank_code;
CREATE TABLE emi_bank_code (
    id BIGSERIAL PRIMARY KEY,
    emi_bank TEXT NOT NULL,
    juspay_bank_code_id BIGINT NOT NULL,
    last_updated TIMESTAMP
);

DROP TABLE IF EXISTS gateway_bank_emi_support;
CREATE TABLE gateway_bank_emi_support (
    id BIGSERIAL PRIMARY KEY,
    gateway TEXT NOT NULL,
    bank TEXT NOT NULL,
    juspay_bank_code_id BIGINT,
    scope TEXT
);

DROP TABLE IF EXISTS user_eligibility_info;
CREATE TABLE user_eligibility_info (
    id TEXT PRIMARY KEY,
    flow_type TEXT NOT NULL,
    identifier_name TEXT NOT NULL,
    identifier_value TEXT NOT NULL,
    provider_name TEXT NOT NULL,
    disabled BOOLEAN
);

DROP TABLE IF EXISTS merchant_gateway_account;
CREATE TABLE merchant_gateway_account (
    id BIGSERIAL PRIMARY KEY,
    account_details TEXT NOT NULL,
    gateway TEXT NOT NULL,
    merchant_id TEXT NOT NULL,
    payment_methods TEXT,
    supported_payment_flows TEXT,
    disabled BOOLEAN,
    reference_id TEXT,
    supported_currencies TEXT,
    gateway_identifier TEXT,
    gateway_type TEXT,
    supported_txn_type TEXT
);

DROP TABLE IF EXISTS tenant_config_filter;
CREATE TABLE tenant_config_filter (
    id VARCHAR(255) PRIMARY KEY,
    filter_group_id VARCHAR(255) NOT NULL,
    dimension_value VARCHAR(255) NOT NULL,
    config_value VARCHAR(255) NOT NULL,
    tenant_config_id VARCHAR(255) NOT NULL
);

DROP TABLE IF EXISTS routing_algorithm;
CREATE TABLE routing_algorithm (
    id VARCHAR(255) PRIMARY KEY,
    created_by VARCHAR(255) NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    algorithm_data TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    modified_at TIMESTAMP NOT NULL
);

DROP TABLE IF EXISTS co_badged_cards_info;
CREATE TABLE co_badged_cards_info (
    id VARCHAR(64) PRIMARY KEY,
    card_bin_min BIGINT NOT NULL,
    card_bin_max BIGINT NOT NULL,
    issuing_bank_name TEXT,
    card_network VARCHAR(32) NOT NULL,
    country_code TEXT NOT NULL,
    card_type TEXT NOT NULL,
    regulated BOOLEAN NOT NULL,
    regulated_name TEXT,
    prepaid BOOLEAN NOT NULL,
    reloadable BOOLEAN NOT NULL,
    pan_or_token TEXT NOT NULL,
    card_bin_length SMALLINT NOT NULL,
    card_brand_is_additional BOOLEAN NOT NULL,
    domestic_only BOOLEAN NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    modified_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, 
    last_updated_provider VARCHAR(128)
);

CREATE INDEX co_badged_cards_card_bin_min_card_bin_max_index ON co_badged_cards_info (card_bin_min, card_bin_max);
