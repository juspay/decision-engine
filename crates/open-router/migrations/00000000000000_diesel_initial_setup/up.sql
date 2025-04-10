DROP DATABASE IF EXISTS jdb;
CREATE DATABASE jdb;
USE jdb;

SET FOREIGN_KEY_CHECKS=0;

DROP TABLE IF EXISTS gateway_bank_emi_support_v2;
CREATE TABLE gateway_bank_emi_support_v2 (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    version BIGINT NOT NULL,
    gateway VARCHAR(255) NOT NULL,
    juspay_bank_code_id BIGINT NOT NULL,
    card_type VARCHAR(255) NOT NULL,
    tenure INT NOT NULL,
    gateway_emi_code VARCHAR(255) NOT NULL,
    gateway_plan_id VARCHAR(255),
    scope VARCHAR(255) NOT NULL,
    metadata TEXT,
    date_created DATETIME,
    last_updated DATETIME
);

DROP TABLE IF EXISTS gateway_outage;
CREATE TABLE gateway_outage (
    id VARCHAR(255) PRIMARY KEY,
    version INT NOT NULL,
    end_time DATETIME NOT NULL,
    gateway VARCHAR(255),
    merchant_id VARCHAR(255),
    start_time DATETIME NOT NULL,
    bank VARCHAR(255),
    payment_method_type VARCHAR(255),
    payment_method VARCHAR(255),
    description TEXT,
    date_created DATETIME,
    last_updated DATETIME,
    juspay_bank_code_id BIGINT,
    metadata TEXT
);

DROP TABLE IF EXISTS merchant_priority_logic;
CREATE TABLE merchant_priority_logic (
    id VARCHAR(255) PRIMARY KEY,
    version BIGINT NOT NULL,
    date_created DATETIME NOT NULL,
    last_updated DATETIME NOT NULL,
    merchant_account_id BIGINT NOT NULL,
    status VARCHAR(255) NOT NULL,
    priority_logic TEXT NOT NULL,
    name VARCHAR(255),
    description TEXT,
    priority_logic_rules TEXT,
    is_active_logic bit(1) NOT NULL
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
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    card_brand TEXT NOT NULL,
    date_created DATETIME NOT NULL,
    last_updated DATETIME NOT NULL,
    merchant_account_id BIGINT NOT NULL,
    preference_score DOUBLE NOT NULL,
    preferred_gateway TEXT NOT NULL
);

DROP TABLE IF EXISTS issuer_routes;
CREATE TABLE issuer_routes (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    issuer TEXT NOT NULL,
    merchant_id TEXT NOT NULL,
    preferred_gateway TEXT NOT NULL,
    preference_score DOUBLE NOT NULL,
    date_created DATETIME NOT NULL,
    last_updated DATETIME NOT NULL
);

DROP TABLE IF EXISTS merchant_gateway_account_sub_info;
CREATE TABLE merchant_gateway_account_sub_info (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    merchant_gateway_account_id BIGINT NOT NULL,
    sub_info_type TEXT NOT NULL,
    sub_id_type TEXT NOT NULL,
    juspay_sub_account_id TEXT NOT NULL,
    gateway_sub_account_id TEXT NOT NULL,
    disabled bit(1) NOT NULL
);

DROP TABLE IF EXISTS gateway_payment_method_flow;
CREATE TABLE gateway_payment_method_flow (
    id TEXT NOT NULL,
    gateway_payment_flow_id TEXT NOT NULL,
    payment_method_id BIGINT,
    date_created DATETIME NOT NULL,
    last_updated DATETIME NOT NULL,
    gateway TEXT NOT NULL,
    payment_flow_id TEXT NOT NULL,
    juspay_bank_code_id BIGINT,
    gateway_bank_code TEXT,
    currency_configs TEXT,
    dsl TEXT,
    non_combination_flows TEXT,
    country_code_alpha3 TEXT,
    disabled bit(1) NOT NULL,
    payment_method_type TEXT,
    PRIMARY KEY (id(255))
);

DROP TABLE IF EXISTS merchant_iframe_preferences;
CREATE TABLE merchant_iframe_preferences (
    id INT AUTO_INCREMENT PRIMARY KEY,
    merchant_id TEXT NOT NULL,
    dynamic_switching_enabled bit(1),
    isin_routing_enabled bit(1),
    issuer_routing_enabled bit(1),
    txn_failure_gateway_penality bit(1),
    card_brand_routing_enabled bit(1)
);

DROP TABLE IF EXISTS token_bin_info;
CREATE TABLE token_bin_info (
    token_bin TEXT NOT NULL,
    card_bin TEXT NOT NULL,
    provider TEXT NOT NULL,
    date_created DATETIME,
    last_updated DATETIME
);

DROP TABLE IF EXISTS txn_offer_detail;
CREATE TABLE txn_offer_detail (
    id TEXT NOT NULL,
    txn_detail_id TEXT NOT NULL,
    offer_id TEXT NOT NULL,
    status TEXT NOT NULL,
    date_created DATETIME,
    last_updated DATETIME,
    gateway_info TEXT,
    internal_metadata TEXT,
    partition_key DATETIME,
    PRIMARY KEY (id(255))
);

DROP TABLE IF EXISTS merchant_gateway_card_info;
CREATE TABLE merchant_gateway_card_info (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    disabled bit(1) NOT NULL,
    gateway_card_info_id BIGINT NOT NULL,
    merchant_account_id BIGINT NOT NULL,
    emandate_register_max_amount DOUBLE,
    merchant_gateway_account_id BIGINT
);

DROP TABLE IF EXISTS merchant_account;
CREATE TABLE merchant_account (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    merchant_id TEXT,
    date_created DATETIME NOT NULL,
    gateway_decided_by_health_enabled bit(1),
    gateway_priority TEXT,
    gateway_priority_logic TEXT,
    internal_hash_key TEXT,
    locker_id TEXT,
    token_locker_id TEXT,
    user_id BIGINT,
    settlement_account_id BIGINT,
    secondary_merchant_account_id BIGINT,
    use_code_for_gateway_priority bit(1) NOT NULL,
    enable_gateway_reference_id_based_routing bit(1),
    gateway_success_rate_based_decider_input TEXT,
    internal_metadata TEXT,
    enabled bit(1) NOT NULL,
    country TEXT,
    installment_enabled bit(1),
    tenant_account_id TEXT,
    priority_logic_config TEXT,
    merchant_category_code TEXT
);

DROP TABLE IF EXISTS merchant_gateway_payment_method_flow;
CREATE TABLE merchant_gateway_payment_method_flow (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    gateway_payment_method_flow_id TEXT NOT NULL,
    merchant_gateway_account_id BIGINT NOT NULL,
    currency_configs TEXT,
    date_created DATETIME NOT NULL,
    last_updated DATETIME NOT NULL,
    disabled bit(1),
    gateway_bank_code TEXT
);

DROP TABLE IF EXISTS txn_offer;
CREATE TABLE txn_offer (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    version BIGINT NOT NULL,
    discount_amount BIGINT NOT NULL,
    offer_id TEXT NOT NULL,
    signature TEXT NOT NULL,
    txn_detail_id BIGINT NOT NULL
);

DROP TABLE IF EXISTS isin_routes;
CREATE TABLE isin_routes (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    isin TEXT NOT NULL,
    merchant_id TEXT NOT NULL,
    preferred_gateway TEXT NOT NULL,
    preference_score DOUBLE NOT NULL,
    date_created DATETIME NOT NULL,
    last_updated DATETIME NOT NULL
);

DROP TABLE IF EXISTS feature;
CREATE TABLE feature (
    id INT AUTO_INCREMENT PRIMARY KEY,
    enabled bit(1) NOT NULL,
    name TEXT NOT NULL,
    merchant_id TEXT NULL
);

DROP TABLE IF EXISTS merchant_config;
CREATE TABLE merchant_config (
    id TEXT NOT NULL,
    merchant_account_id BIGINT NOT NULL,
    config_category TEXT NOT NULL,
    config_name TEXT NOT NULL,
    status TEXT NOT NULL,
    config_value TEXT NULL,
    date_created DATETIME NOT NULL,
    last_updated DATETIME NOT NULL,
    PRIMARY KEY (id(255))
);

DROP TABLE IF EXISTS service_configuration;
CREATE TABLE service_configuration (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    name TEXT NOT NULL,
    value TEXT NULL,
    new_value TEXT NULL,
    previous_value TEXT NULL,
    new_value_status TEXT NULL
);

DROP TABLE IF EXISTS payment_method;
CREATE TABLE payment_method (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    date_created DATETIME NOT NULL,
    last_updated DATETIME NOT NULL,
    name TEXT NOT NULL,
    pm_type TEXT NOT NULL,
    description TEXT NULL,
    juspay_bank_code_id BIGINT NULL,
    display_name TEXT NULL,
    nick_name TEXT NULL,
    sub_type TEXT NULL,
    dsl TEXT NULL
);

DROP TABLE IF EXISTS txn_card_info;
CREATE TABLE txn_card_info (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    txn_id TEXT NOT NULL,
    card_isin TEXT NULL,
    card_issuer_bank_name TEXT NULL,
    card_switch_provider TEXT NULL,
    card_type TEXT NULL,
    name_on_card TEXT NULL,
    txn_detail_id BIGINT NULL,
    date_created DATETIME NULL,
    payment_method_type TEXT NULL,
    payment_method TEXT NULL,
    payment_source TEXT NULL,
    auth_type TEXT NULL,
    partition_key DATETIME NULL
);

DROP TABLE IF EXISTS txn_detail;
CREATE TABLE txn_detail (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    order_id TEXT NOT NULL,
    status TEXT NOT NULL,
    txn_id TEXT NOT NULL,
    txn_type TEXT NOT NULL,
    date_created DATETIME,
    add_to_locker bit(1),
    merchant_id TEXT,
    gateway TEXT,
    express_checkout bit(1),
    is_emi bit(1),
    emi_bank TEXT,
    emi_tenure INT,
    txn_uuid TEXT,
    merchant_gateway_account_id BIGINT,
    net_amount DOUBLE,
    txn_amount DOUBLE,
    txn_object_type TEXT,
    source_object TEXT,
    source_object_id TEXT,
    currency TEXT,
    surcharge_amount DOUBLE,
    tax_amount DOUBLE,
    internal_metadata TEXT,
    metadata TEXT,
    offer_deduction_amount DOUBLE,
    internal_tracking_info TEXT,
    partition_key DATETIME,
    txn_amount_breakup TEXT
);

DROP TABLE IF EXISTS card_info;
CREATE TABLE card_info (
    card_isin TEXT NOT NULL,
    card_switch_provider TEXT NOT NULL,
    card_type TEXT,
    card_sub_type TEXT,
    card_sub_type_category TEXT,
    card_issuer_country TEXT,
    country_code TEXT,
    extended_card_type TEXT,
    PRIMARY KEY (card_isin(255))
);

DROP TABLE IF EXISTS gateway_card_info;
CREATE TABLE gateway_card_info (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    isin TEXT,
    gateway TEXT,
    card_issuer_bank_name TEXT,
    auth_type TEXT,
    juspay_bank_code_id BIGINT,
    disabled bit(1),
    validation_type TEXT,
    payment_method_type TEXT
);

DROP TABLE IF EXISTS juspay_bank_code;
CREATE TABLE juspay_bank_code (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    bank_code TEXT NOT NULL,
    bank_name TEXT NOT NULL
);

DROP TABLE IF EXISTS emi_bank_code;
CREATE TABLE emi_bank_code (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    emi_bank TEXT NOT NULL,
    juspay_bank_code_id BIGINT NOT NULL,
    last_updated DATETIME
);

DROP TABLE IF EXISTS gateway_bank_emi_support;
CREATE TABLE gateway_bank_emi_support (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    gateway TEXT NOT NULL,
    bank TEXT NOT NULL,
    juspay_bank_code_id BIGINT,
    scope TEXT
);

DROP TABLE IF EXISTS user_eligibility_info;
CREATE TABLE user_eligibility_info (
    id TEXT NOT NULL,
    flow_type TEXT NOT NULL,
    identifier_name TEXT NOT NULL,
    identifier_value TEXT NOT NULL,
    provider_name TEXT NOT NULL,
    disabled bit(1),
    PRIMARY KEY (id(255))
);

DROP TABLE IF EXISTS merchant_gateway_account;
CREATE TABLE merchant_gateway_account (
    id BIGINT PRIMARY KEY AUTO_INCREMENT,
    account_details TEXT NOT NULL,
    gateway TEXT NOT NULL,
    merchant_id TEXT NOT NULL,
    payment_methods TEXT,
    supported_payment_flows TEXT,
    disabled bit(1),
    reference_id TEXT,
    supported_currencies TEXT,
    gateway_identifier TEXT,
    gateway_type TEXT,
    supported_txn_type TEXT
);

DROP TABLE IF EXISTS tenant_config_filter;
CREATE TABLE tenant_config_filter (
    id VARCHAR(255) NOT NULL PRIMARY KEY,
    filter_group_id VARCHAR(255) NOT NULL,
    dimension_value VARCHAR(255) NOT NULL,
    config_value VARCHAR(255) NOT NULL,
    tenant_config_id VARCHAR(255) NOT NULL
);

SET FOREIGN_KEY_CHECKS=1;