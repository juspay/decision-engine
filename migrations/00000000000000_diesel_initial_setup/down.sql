USE jdb;

DROP TABLE IF EXISTS co_badged_cards_info_test;
DROP TABLE IF EXISTS routing_algorithm;
DROP TABLE IF EXISTS tenant_config_filter;
DROP TABLE IF EXISTS merchant_gateway_account;
DROP TABLE IF EXISTS user_eligibility_info;
DROP TABLE IF EXISTS gateway_bank_emi_support;
DROP TABLE IF EXISTS emi_bank_code;
DROP TABLE IF EXISTS juspay_bank_code;
DROP TABLE IF EXISTS gateway_card_info;
DROP TABLE IF EXISTS card_info;
DROP TABLE IF EXISTS txn_detail;
DROP TABLE IF EXISTS txn_card_info;
DROP TABLE IF EXISTS payment_method;
DROP TABLE IF EXISTS service_configuration;
DROP TABLE IF EXISTS merchant_config;
DROP TABLE IF EXISTS feature;
DROP TABLE IF EXISTS isin_routes;
DROP TABLE IF EXISTS txn_offer;
DROP TABLE IF EXISTS merchant_gateway_payment_method_flow;
DROP TABLE IF EXISTS merchant_account;
DROP TABLE IF EXISTS merchant_gateway_card_info;
DROP TABLE IF EXISTS txn_offer_detail;
DROP TABLE IF EXISTS token_bin_info;
DROP TABLE IF EXISTS merchant_iframe_preferences;
DROP TABLE IF EXISTS gateway_payment_method_flow;
DROP TABLE IF EXISTS merchant_gateway_account_sub_info;
DROP TABLE IF EXISTS issuer_routes;
DROP TABLE IF EXISTS card_brand_routes;
DROP TABLE IF EXISTS tenant_config;
DROP TABLE IF EXISTS merchant_priority_logic;
DROP TABLE IF EXISTS gateway_outage;
DROP TABLE IF EXISTS gateway_bank_emi_support_v2;

DROP DATABASE IF EXISTS jdb;

DROP FUNCTION IF EXISTS diesel_manage_updated_at(_tbl regclass);
DROP FUNCTION IF EXISTS diesel_set_updated_at();