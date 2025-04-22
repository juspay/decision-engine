use masking::{PeekInterface, Secret};

use crate::redis::types as SC;

pub struct ENABLE_OPTIMIZATION_DURING_DOWNTIME;
impl SC::ServiceConfigKey for ENABLE_OPTIMIZATION_DURING_DOWNTIME {
    fn get_key(&self) -> String {
        "enable_optimization_during_downtime".to_string()
    }
}

pub const enableOptimizationDuringDowntime: ENABLE_OPTIMIZATION_DURING_DOWNTIME =
    ENABLE_OPTIMIZATION_DURING_DOWNTIME;

pub struct DEFAULT_SR_BASED_GATEWAY_ELIMINATION_INPUT;
impl SC::ServiceConfigKey for DEFAULT_SR_BASED_GATEWAY_ELIMINATION_INPUT {
    fn get_key(&self) -> String {
        "DEFAULT_SR_BASED_GATEWAY_ELIMINATION_INPUT".to_string()
    }
}

pub const defaultSRBasedGatewayEliminationInput: DEFAULT_SR_BASED_GATEWAY_ELIMINATION_INPUT =
    DEFAULT_SR_BASED_GATEWAY_ELIMINATION_INPUT;

//TODO : This is duplicate and is same key is already there in decider constants.rs
pub struct SR_V3_INPUT_CONFIG(String);
impl SC::ServiceConfigKey for SR_V3_INPUT_CONFIG {
    fn get_key(&self) -> String {
        format!("SR_V3_INPUT_CONFIG_{}", self.0)
    }
}

pub fn srV3InputConfig(mid: String) -> SR_V3_INPUT_CONFIG {
    SR_V3_INPUT_CONFIG(mid)
}

//TODO : This is duplicate and is same key is already there in decider constants.rs
pub struct SR_V3_INPUT_CONFIG_DEFAULT;
impl SC::ServiceConfigKey for SR_V3_INPUT_CONFIG_DEFAULT {
    fn get_key(&self) -> String {
        "SR_V3_INPUT_CONFIG_DEFAULT".to_string()
    }
}

pub const srV3DefaultInputConfig: SR_V3_INPUT_CONFIG_DEFAULT = SR_V3_INPUT_CONFIG_DEFAULT;

pub const DEFAULT_SR_V3_BASED_BUCKET_SIZE: i32 = 125;
pub const defaultSrV3BasedUpperResetFactor: f64 = 3.0;
pub const defaultSrV3BasedLowerResetFactor: f64 = 3.0;
pub const defaultSrV3BasedHedgingPercent: f64 = 5.0;
pub const DEFAULT_SR_V3_BASED_GATEWAY_SIGMA_FACTOR: f64 = 0.0;

pub struct ALT_ID_ENABLED_GATEWAY_FOR_EMIBANK;
impl SC::ServiceConfigKey for ALT_ID_ENABLED_GATEWAY_FOR_EMIBANK {
    fn get_key(&self) -> String {
        "ALT_ID_ENABLED_GATEWAY_FOR_EMIBANK".to_string()
    }
}

pub const altIdEnabledGatewayEmiBank: ALT_ID_ENABLED_GATEWAY_FOR_EMIBANK = ALT_ID_ENABLED_GATEWAY_FOR_EMIBANK;


pub struct SR_BASED_TRANSACTION_RESET_COUNT;
impl SC::ServiceConfigKey for SR_BASED_TRANSACTION_RESET_COUNT {
    fn get_key(&self) -> String {
        "SR_BASED_TRANSACTION_RESET_COUNT".to_string()
    }
}

pub const srBasedTxnResetCount: SR_BASED_TRANSACTION_RESET_COUNT = SR_BASED_TRANSACTION_RESET_COUNT;

pub struct SCHEDULED_OUTAGE_VALIDATION_DURATION;
impl SC::ServiceConfigKey for SCHEDULED_OUTAGE_VALIDATION_DURATION {
    fn get_key(&self) -> String {
        "SCHEDULED_OUTAGE_VALIDATION_DURATION".to_string()
    }
}

pub struct ENABLE_ELIMINATION_V2;
impl SC::ServiceConfigKey for ENABLE_ELIMINATION_V2 {
    fn get_key(&self) -> String {
        "ENABLE_ELIMINATION_V2".to_string()
    }
}

pub const enableEliminationV2: ENABLE_ELIMINATION_V2 = ENABLE_ELIMINATION_V2;

pub struct ENABLE_OUTAGE_V2;
impl SC::ServiceConfigKey for ENABLE_OUTAGE_V2 {
    fn get_key(&self) -> String {
        "ENABLE_OUTAGE_V2".to_string()
    }
}

pub const enableEliminationV2ForOutage: ENABLE_OUTAGE_V2 = ENABLE_OUTAGE_V2;

pub const thresholdWeightSr1: &str = "THRESHOLD_WEIGHT_SR1";
pub const thresholdWeightSr2: &str = "THRESHOLD_WEIGHT_SR2";

pub struct DEFAULT_SR1(String);
impl SC::ServiceConfigKey for DEFAULT_SR1 {
    fn get_key(&self) -> String {
        format!("DEFAULT_SR1_{}", self.0)
    }
}

pub fn defaultSr1SConfigPrefix(val: String) -> DEFAULT_SR1 {
    DEFAULT_SR1(val)
}

pub struct DEFAULT_N(String);
impl SC::ServiceConfigKey for DEFAULT_N {
    fn get_key(&self) -> String {
        format!("DEFAULT_N_{}", self.0)
    }
}

pub fn defaultNSConfigPrefix(val: String) -> DEFAULT_N {
    DEFAULT_N(val)
}

pub struct INTERNAL_DEFAULT_ELIMINATION_V2_SUCCESS_RATE_1_AND_N(String);
impl SC::ServiceConfigKey for INTERNAL_DEFAULT_ELIMINATION_V2_SUCCESS_RATE_1_AND_N {
    fn get_key(&self) -> String {
        format!(
            "INTERNAL_DEFAULT_ELIMINATION_V2_SUCCESS_RATE_1_AND_N_{}",
            self.0
        )
    }
}

pub fn internalDefaultEliminationV2SuccessRate1AndNPrefix(
    val: String,
) -> INTERNAL_DEFAULT_ELIMINATION_V2_SUCCESS_RATE_1_AND_N {
    INTERNAL_DEFAULT_ELIMINATION_V2_SUCCESS_RATE_1_AND_N(val)
}

pub const defaultFieldNameForSR1AndN: &str = "default";
pub const sr1KeyPrefix: &str = "sr1_";
pub const nKeyPrefix: &str = "n_";

pub const gwDefaultTxnSoftResetCount: i64 = 10;
pub const defaultGlobalSelectionVolumeThreshold: i64 = 20;

pub struct GATEWAY_RESET_SCORE_ENABLED;
impl SC::ServiceConfigKey for GATEWAY_RESET_SCORE_ENABLED {
    fn get_key(&self) -> String {
        "gateway_reset_score_enabled".to_string()
    }
}

pub const gwResetScoreEnabled: GATEWAY_RESET_SCORE_ENABLED = GATEWAY_RESET_SCORE_ENABLED;

pub const defSRBasedGwLevelEliminationThreshold: f64 = 0.02;
pub const defaultGlobalSelectionMaxCountThreshold: i64 = 5;

pub struct GATEWAY_SCORE_FIRST_DIMENSION_SOFT_TTL;
impl SC::ServiceConfigKey for GATEWAY_SCORE_FIRST_DIMENSION_SOFT_TTL {
    fn get_key(&self) -> String {
        "gateway_score_first_dimension_soft_ttl".to_string()
    }
}

pub const gwScoreFirstDimensionTtl: GATEWAY_SCORE_FIRST_DIMENSION_SOFT_TTL =
    GATEWAY_SCORE_FIRST_DIMENSION_SOFT_TTL;

pub struct GATEWAY_SCORE_SECOND_DIMENSION_SOFT_TTL;
impl SC::ServiceConfigKey for GATEWAY_SCORE_SECOND_DIMENSION_SOFT_TTL {
    fn get_key(&self) -> String {
        "gateway_score_second_dimension_soft_ttl".to_string()
    }
}

pub const gwScoreSecondDimensionTtl: GATEWAY_SCORE_SECOND_DIMENSION_SOFT_TTL =
    GATEWAY_SCORE_SECOND_DIMENSION_SOFT_TTL;

pub struct GATEWAY_SCORE_THIRD_DIMENSION_SOFT_TTL;
impl SC::ServiceConfigKey for GATEWAY_SCORE_THIRD_DIMENSION_SOFT_TTL {
    fn get_key(&self) -> String {
        "gateway_score_third_dimension_soft_ttl".to_string()
    }
}

pub const gwScoreThirdDimensionTtl: GATEWAY_SCORE_THIRD_DIMENSION_SOFT_TTL =
    GATEWAY_SCORE_THIRD_DIMENSION_SOFT_TTL;

pub struct GATEWAY_SCORE_FOURTH_DIMENSION_SOFT_TTL;
impl SC::ServiceConfigKey for GATEWAY_SCORE_FOURTH_DIMENSION_SOFT_TTL {
    fn get_key(&self) -> String {
        "gateway_score_fourth_dimension_soft_ttl".to_string()
    }
}

pub const gwScoreFourthDimensionTtl: GATEWAY_SCORE_FOURTH_DIMENSION_SOFT_TTL =
    GATEWAY_SCORE_FOURTH_DIMENSION_SOFT_TTL;

pub const defScoreKeysTtl: f64 = 900000.0;

pub struct IS_GBESV2_ENABLED;
impl SC::ServiceConfigKey for IS_GBESV2_ENABLED {
    fn get_key(&self) -> String {
        "IS_GBESV2_ENABLED".to_string()
    }
}

pub const gbesV2Enabled: IS_GBESV2_ENABLED = IS_GBESV2_ENABLED;

pub struct ENABLE_GATEWAY_LEVEL_SR_ELIMINATION;
impl SC::ServiceConfigKey for ENABLE_GATEWAY_LEVEL_SR_ELIMINATION {
    fn get_key(&self) -> String {
        "enable_gateway_level_sr_elimination".to_string()
    }
}

pub const enableGwLevelSrElimination: ENABLE_GATEWAY_LEVEL_SR_ELIMINATION =
    ENABLE_GATEWAY_LEVEL_SR_ELIMINATION;

pub struct SR_BASED_GATEWAY_ELIMINATION_THRESHOLD;
impl SC::ServiceConfigKey for SR_BASED_GATEWAY_ELIMINATION_THRESHOLD {
    fn get_key(&self) -> String {
        "SR_BASED_GATEWAY_ELIMINATION_THRESHOLD".to_string()
    }
}

pub const srBasedGatewayEliminationThreshold: SR_BASED_GATEWAY_ELIMINATION_THRESHOLD =
    SR_BASED_GATEWAY_ELIMINATION_THRESHOLD;

pub const defaultSrBasedGatewayEliminationThreshold: f64 = 0.05;

pub struct OTP_CARD_INFO_RESTRICTED_GATEWAYS;
impl SC::ServiceConfigKey for OTP_CARD_INFO_RESTRICTED_GATEWAYS {
    fn get_key(&self) -> String {
        "OTP_CARD_INFO_RESTRICTED_GATEWAYS".to_string()
    }
}

pub struct AUTH_TYPE_RESTRICTED_GATEWAYS;
impl SC::ServiceConfigKey for AUTH_TYPE_RESTRICTED_GATEWAYS {
    fn get_key(&self) -> String {
        "AUTH_TYPE_RESTRICTED_GATEWAYS".to_string()
    }
}

pub struct CARD_EMI_EXPLICIT_GATEWAYS;
impl SC::ServiceConfigKey for CARD_EMI_EXPLICIT_GATEWAYS {
    fn get_key(&self) -> String {
        "CARD_EMI_EXPLICIT_GATEWAYS".to_string()
    }
}

pub struct CONSUMER_FINANCE_ONLY_GATEWAYS;
impl SC::ServiceConfigKey for CONSUMER_FINANCE_ONLY_GATEWAYS {
    fn get_key(&self) -> String {
        "CONSUMER_FINANCE_ONLY_GATEWAYS".to_string()
    }
}

pub struct CONSUMER_FINANCE_ALSO_GATEWAYS;
impl SC::ServiceConfigKey for CONSUMER_FINANCE_ALSO_GATEWAYS {
    fn get_key(&self) -> String {
        "CONSUMER_FINANCE_ALSO_GATEWAYS".to_string()
    }
}

pub struct MUTUAL_FUND_FLOW_SUPPORTED_GATEWAYS;
impl SC::ServiceConfigKey for MUTUAL_FUND_FLOW_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        "MUTUAL_FUND_FLOW_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct CROSS_BORDER_FLOW_SUPPORTED_GATEWAYS;
impl SC::ServiceConfigKey for CROSS_BORDER_FLOW_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        "CROSS_BORDER_FLOW_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct SBMD_SUPPORTED_GATEWAYS;
impl SC::ServiceConfigKey for SBMD_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        "SBMD_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct SPLIT_SETTLEMENT_SUPPORTED_GATEWAYS;
impl SC::ServiceConfigKey for SPLIT_SETTLEMENT_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        "SPLIT_SETTLEMENT_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct TPV_ONLY_SUPPORTED_GATEWAYS;
impl SC::ServiceConfigKey for TPV_ONLY_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        "TPV_ONLY_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct NB_ONLY_GATEWAYS;
impl SC::ServiceConfigKey for NB_ONLY_GATEWAYS {
    fn get_key(&self) -> String {
        "NB_ONLY_GATEWAYS".to_string()
    }
}

pub struct UPI_ONLY_GATEWAYS;
impl SC::ServiceConfigKey for UPI_ONLY_GATEWAYS {
    fn get_key(&self) -> String {
        "UPI_ONLY_GATEWAYS".to_string()
    }
}

pub struct UPI_ALSO_GATEWAYS;
impl SC::ServiceConfigKey for UPI_ALSO_GATEWAYS {
    fn get_key(&self) -> String {
        "UPI_ALSO_GATEWAYS".to_string()
    }
}

pub struct WALLET_ONLY_GATEWAYS;
impl SC::ServiceConfigKey for WALLET_ONLY_GATEWAYS {
    fn get_key(&self) -> String {
        "WALLET_ONLY_GATEWAYS".to_string()
    }
}

pub struct WALLET_ALSO_GATEWAYS;
impl SC::ServiceConfigKey for WALLET_ALSO_GATEWAYS {
    fn get_key(&self) -> String {
        "WALLET_ALSO_GATEWAYS".to_string()
    }
}

pub struct NO_OR_LOW_COST_EMI_SUPPORTED_GATEWAYS;
impl SC::ServiceConfigKey for NO_OR_LOW_COST_EMI_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        "NO_OR_LOW_COST_EMI_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct SI_ON_EMI_CARD_SUPPORTED_GATEWAYS;
impl SC::ServiceConfigKey for SI_ON_EMI_CARD_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        "SI_ON_EMI_CARD_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct TOKEN_PROVIDER_GATEWAY_MAPPING;
impl SC::ServiceConfigKey for TOKEN_PROVIDER_GATEWAY_MAPPING {
    fn get_key(&self) -> String {
        "TOKEN_PROVIDER_GATEWAY_MAPPING".to_string()
    }
}

pub struct TXN_TYPE_GATEWAY_MAPPING;
impl SC::ServiceConfigKey for TXN_TYPE_GATEWAY_MAPPING {
    fn get_key(&self) -> String {
        "TXN_TYPE_GATEWAY_MAPPING".to_string()
    }
}

pub struct TXN_DETAIL_TYPE_RESTRICTED_GATEWAYS;
impl SC::ServiceConfigKey for TXN_DETAIL_TYPE_RESTRICTED_GATEWAYS {
    fn get_key(&self) -> String {
        "TXN_DETAIL_TYPE_RESTRICTED_GATEWAYS".to_string()
    }
}

pub struct REWARD_ONLY_GATEWAYS;
impl SC::ServiceConfigKey for REWARD_ONLY_GATEWAYS {
    fn get_key(&self) -> String {
        "REWARD_ONLY_GATEWAYS".to_string()
    }
}

pub struct REWARD_ALSO_GATEWAYS;
impl SC::ServiceConfigKey for REWARD_ALSO_GATEWAYS {
    fn get_key(&self) -> String {
        "REWARD_ALSO_GATEWAYS".to_string()
    }
}

pub struct SODEXO_ONLY_GATEWAYS;
impl SC::ServiceConfigKey for SODEXO_ONLY_GATEWAYS {
    fn get_key(&self) -> String {
        "SODEXO_ONLY_GATEWAYS".to_string()
    }
}

pub struct SODEXO_ALSO_GATEWAYS;
impl SC::ServiceConfigKey for SODEXO_ALSO_GATEWAYS {
    fn get_key(&self) -> String {
        "SODEXO_ALSO_GATEWAYS".to_string()
    }
}

pub struct CASH_ONLY_GATEWAYS;
impl SC::ServiceConfigKey for CASH_ONLY_GATEWAYS {
    fn get_key(&self) -> String {
        "CASH_ONLY_GATEWAYS".to_string()
    }
}

pub struct AMEX_SUPPORTED_GATEWAYS;
impl SC::ServiceConfigKey for AMEX_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        "AMEX_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct CARD_BRAND_TO_CVVLESS_TXN_SUPPORTED_GATEWAYS;
impl SC::ServiceConfigKey for CARD_BRAND_TO_CVVLESS_TXN_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        "CARD_BRAND_TO_CVVLESS_TXN_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct CVVLESS_TXN_SUPPORTED_COMMON_GATEWAYS;
impl SC::ServiceConfigKey for CVVLESS_TXN_SUPPORTED_COMMON_GATEWAYS {
    fn get_key(&self) -> String {
        "CVVLESS_TXN_SUPPORTED_COMMON_GATEWAYS".to_string()
    }
}

pub struct MERCHANT_CONTAINER_SUPPORTED_GATEWAYS;
impl SC::ServiceConfigKey for MERCHANT_CONTAINER_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        "MERCHANT_CONTAINER_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct TOKEN_SUPPORTED_GATEWAYS(pub String, pub Option<String>, pub String, pub String);
impl SC::ServiceConfigKey for TOKEN_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        format!(
            "{}_{}_{}_{}_SUPPORTED_GATEWAYS",
            self.0,
            self.1.clone().unwrap_or_default(),
            self.2,
            self.3
        )
    }
}

pub struct TOKEN_REPEAT_OTP_SUPPORTED_GATEWAYS(String);
impl SC::ServiceConfigKey for TOKEN_REPEAT_OTP_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        format!("{}_TOKEN_REPEAT_OTP_SUPPORTED_GATEWAYS", self.0)
    }
}

pub fn getTokenRepeatOtpGatewayKey(val: String) -> TOKEN_REPEAT_OTP_SUPPORTED_GATEWAYS {
    TOKEN_REPEAT_OTP_SUPPORTED_GATEWAYS(val)
}

pub struct TOKEN_REPEAT_CVVLESS_SUPPORTED_GATEWAYS(String);
impl SC::ServiceConfigKey for TOKEN_REPEAT_CVVLESS_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        format!("{}_TOKEN_REPEAT_CVVLESS_SUPPORTED_GATEWAYS", self.0)
    }
}
pub fn getTokenRepeatCvvLessGatewayKey(val: String) -> TOKEN_REPEAT_CVVLESS_SUPPORTED_GATEWAYS {
    TOKEN_REPEAT_CVVLESS_SUPPORTED_GATEWAYS(val)
}

pub struct TOKEN_REPEAT_MANDATE_SUPPORTED_GATEWAYS(String);
impl SC::ServiceConfigKey for TOKEN_REPEAT_MANDATE_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        format!("{}_TOKEN_REPEAT_MANDATE_SUPPORTED_GATEWAYS", self.0)
    }
}

pub fn getTokenRepeatMandateGatewayKey(val: String) -> TOKEN_REPEAT_MANDATE_SUPPORTED_GATEWAYS {
    TOKEN_REPEAT_MANDATE_SUPPORTED_GATEWAYS(val)
}

pub struct TOKEN_REPEAT_SUPPORTED_GATEWAYS(String);
impl SC::ServiceConfigKey for TOKEN_REPEAT_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        format!("{}_TOKEN_REPEAT_SUPPORTED_GATEWAYS", self.0)
    }
}

pub fn getTokenRepeatGatewayKey(val: String) -> TOKEN_REPEAT_SUPPORTED_GATEWAYS {
    TOKEN_REPEAT_SUPPORTED_GATEWAYS(val)
}

pub struct MANDATE_GUEST_CHECKOUT_SUPPORTED_GATEWAYS(Option<Secret<String>>);
impl SC::ServiceConfigKey for MANDATE_GUEST_CHECKOUT_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        format!(
            "{}_MANDATE_GUEST_CHECKOUT_SUPPORTED_GATEWAYS",
            self.0
                .as_ref()
                .map_or("DEFAULT".to_string(), |s| s.peek().to_string())
        )
    }
}

pub fn getmandateGuestCheckoutKey(
    val: Option<Secret<String>>,
) -> MANDATE_GUEST_CHECKOUT_SUPPORTED_GATEWAYS {
    MANDATE_GUEST_CHECKOUT_SUPPORTED_GATEWAYS(val)
}

pub struct TOKEN_REPEAT_CVVLESS_SUPPORTED_BANKS(Option<Secret<String>>);
impl SC::ServiceConfigKey for TOKEN_REPEAT_CVVLESS_SUPPORTED_BANKS {
    fn get_key(&self) -> String {
        format!(
            "{}_TOKEN_REPEAT_CVVLESS_SUPPORTED_BANKS",
            self.0
                .as_ref()
                .map_or("DEFAULT".to_string(), |s| s.peek().to_string())
        )
    }
}

pub fn getTokenRepeatCvvLessBankCodeKey(
    val: Option<Secret<String>>,
) -> TOKEN_REPEAT_CVVLESS_SUPPORTED_BANKS {
    TOKEN_REPEAT_CVVLESS_SUPPORTED_BANKS(val)
}

pub struct EMI_BIN_VALIDATION_SUPPORTED_BANKS;
impl SC::ServiceConfigKey for EMI_BIN_VALIDATION_SUPPORTED_BANKS {
    fn get_key(&self) -> String {
        "EMI_BIN_VALIDATION_SUPPORTED_BANKS".to_string()
    }
}

pub const getEmiBinValidationSupportedBanksKey: EMI_BIN_VALIDATION_SUPPORTED_BANKS =
    EMI_BIN_VALIDATION_SUPPORTED_BANKS;

pub struct METRIC_TRACKING_LOG;
impl SC::ServiceConfigKey for METRIC_TRACKING_LOG {
    fn get_key(&self) -> String {
        "METRIC_TRACKING_LOG".to_string()
    }
}
pub const metricTrackingLogDataKey: METRIC_TRACKING_LOG = METRIC_TRACKING_LOG;

pub struct V2_ROUTING_HANDLE_LIST;
impl SC::ServiceConfigKey for V2_ROUTING_HANDLE_LIST {
    fn get_key(&self) -> String {
        "V2_ROUTING_HANDLE_LIST".to_string()
    }
}
pub const v2RoutingHandleList: V2_ROUTING_HANDLE_LIST = V2_ROUTING_HANDLE_LIST;

pub struct V2_ROUTING_PSP_LIST;
impl SC::ServiceConfigKey for V2_ROUTING_PSP_LIST {
    fn get_key(&self) -> String {
        "V2_ROUTING_PSP_LIST".to_string()
    }
}
pub const v2RoutingPspList: V2_ROUTING_PSP_LIST = V2_ROUTING_PSP_LIST;

pub struct V2_ROUTING_TOP_BANK_LIST;
impl SC::ServiceConfigKey for V2_ROUTING_TOP_BANK_LIST {
    fn get_key(&self) -> String {
        "V2_ROUTING_TOP_BANK_LIST".to_string()
    }
}
pub const v2RoutingTopBankList: V2_ROUTING_TOP_BANK_LIST = V2_ROUTING_TOP_BANK_LIST;

pub struct V2_ROUTING_PSP_PACKAGE_LIST;
impl SC::ServiceConfigKey for V2_ROUTING_PSP_PACKAGE_LIST {
    fn get_key(&self) -> String {
        "V2_ROUTING_PSP_PACKAGE_LIST".to_string()
    }
}
pub const v2RoutingPspPackageList: V2_ROUTING_PSP_PACKAGE_LIST = V2_ROUTING_PSP_PACKAGE_LIST;

pub struct OPTIMIZATION_ROUTING_CONFIG(pub String);
impl SC::ServiceConfigKey for OPTIMIZATION_ROUTING_CONFIG {
    fn get_key(&self) -> String {
        format!("{}_optimization_routing_config", self.0)
    }
}

pub struct DEFAULT_OPTIMIZATION_ROUTING_CONFIG;
impl SC::ServiceConfigKey for DEFAULT_OPTIMIZATION_ROUTING_CONFIG {
    fn get_key(&self) -> String {
        "default_optimization_routing_config".to_string()
    }
}

pub struct ATM_PIN_CARD_INFO_RESTRICTED_GATEWAYS;
impl SC::ServiceConfigKey for ATM_PIN_CARD_INFO_RESTRICTED_GATEWAYS {
    fn get_key(&self) -> String {
        "ATM_PIN_CARD_INFO_RESTRICTED_GATEWAYS".to_string()
    }
}

pub struct OTP_CARD_INFO_SUPPORTED_GATEWAYS;
impl SC::ServiceConfigKey for OTP_CARD_INFO_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        "OTP_CARD_INFO_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct MOTO_CARD_INFO_SUPPORTED_GATEWAYS;
impl SC::ServiceConfigKey for MOTO_CARD_INFO_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        "MOTO_CARD_INFO_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct TA_OFFLINE_ENABLED_GATEWAYS;
impl SC::ServiceConfigKey for TA_OFFLINE_ENABLED_GATEWAYS {
    fn get_key(&self) -> String {
        "TA_OFFLINE_ENABLED_GATEWAYS".to_string()
    }
}

pub struct MERCHANT_WISE_MANDATE_BIN_ENFORCED_GATEWAYS;
impl SC::ServiceConfigKey for MERCHANT_WISE_MANDATE_BIN_ENFORCED_GATEWAYS {
    fn get_key(&self) -> String {
        "MERCHANT_WISE_MANDATE_BIN_ENFORCED_GATEWAYS".to_string()
    }
}

pub struct MERCHANT_WISE_AUTH_TYPE_BIN_ENFORCED_GATEWAYS;
impl SC::ServiceConfigKey for MERCHANT_WISE_AUTH_TYPE_BIN_ENFORCED_GATEWAYS {
    fn get_key(&self) -> String {
        "MERCHANT_WISE_AUTH_TYPE_BIN_ENFORCED_GATEWAYS".to_string()
    }
}

pub struct CARD_MANDATE_BIN_FILTER_EXCLUDED_GATEWAYS;
impl SC::ServiceConfigKey for CARD_MANDATE_BIN_FILTER_EXCLUDED_GATEWAYS {
    fn get_key(&self) -> String {
        "CARD_MANDATE_BIN_FILTER_EXCLUDED_GATEWAYS".to_string()
    }
}

pub struct ENABLE_GATEWAY_SELECTION_BASED_ON_OPTIMIZED_SR_INPUT(pub String);
impl SC::ServiceConfigKey for ENABLE_GATEWAY_SELECTION_BASED_ON_OPTIMIZED_SR_INPUT {
    fn get_key(&self) -> String {
        format!(
            "ENABLE_GATEWAY_SELECTION_BASED_ON_OPTIMIZED_SR_INPUT_{}",
            self.0
        )
    }
}
pub fn enable_gateway_selection_based_on_optimized_sr_input(
    val: String,
) -> ENABLE_GATEWAY_SELECTION_BASED_ON_OPTIMIZED_SR_INPUT {
    ENABLE_GATEWAY_SELECTION_BASED_ON_OPTIMIZED_SR_INPUT(val)
}

pub struct ENABLE_BETA_DISTRIBUTION_ON_SR_V3;
impl SC::ServiceConfigKey for ENABLE_BETA_DISTRIBUTION_ON_SR_V3 {
    fn get_key(&self) -> String {
        "ENABLE_BETA_DISTRIBUTION_ON_SR_V3".to_string()
    }
}
pub const enable_beta_distribution_on_sr_v3: ENABLE_BETA_DISTRIBUTION_ON_SR_V3 =
    ENABLE_BETA_DISTRIBUTION_ON_SR_V3;

pub struct ENABLE_GATEWAY_SELECTION_BASED_ON_SR_V3_INPUT(pub String);
impl SC::ServiceConfigKey for ENABLE_GATEWAY_SELECTION_BASED_ON_SR_V3_INPUT {
    fn get_key(&self) -> String {
        format!("ENABLE_GATEWAY_SELECTION_BASED_ON_SR_V3_INPUT_{}", self.0)
    }
}
pub fn enable_gateway_selection_based_on_sr_v3_input(
    val: String,
) -> ENABLE_GATEWAY_SELECTION_BASED_ON_SR_V3_INPUT {
    ENABLE_GATEWAY_SELECTION_BASED_ON_SR_V3_INPUT(val)
}

pub struct ENABLE_BINOMIAL_DISTRIBUTION_ON_SR_V3;
impl SC::ServiceConfigKey for ENABLE_BINOMIAL_DISTRIBUTION_ON_SR_V3 {
    fn get_key(&self) -> String {
        "ENABLE_BINOMIAL_DISTRIBUTION_ON_SR_V3".to_string()
    }
}
pub const enable_binomial_distribution_on_sr_v3: ENABLE_BINOMIAL_DISTRIBUTION_ON_SR_V3 =
    ENABLE_BINOMIAL_DISTRIBUTION_ON_SR_V3;

pub struct ENABLE_EXTRA_SCORE_ON_SR_V3;
impl SC::ServiceConfigKey for ENABLE_EXTRA_SCORE_ON_SR_V3 {
    fn get_key(&self) -> String {
        "ENABLE_EXTRA_SCORE_ON_SR_V3".to_string()
    }
}
pub const enable_extra_score_on_sr_v3: ENABLE_EXTRA_SCORE_ON_SR_V3 = ENABLE_EXTRA_SCORE_ON_SR_V3;

pub struct ENABLE_RESET_ON_SR_V3;
impl SC::ServiceConfigKey for ENABLE_RESET_ON_SR_V3 {
    fn get_key(&self) -> String {
        "ENABLE_RESET_ON_SR_V3".to_string()
    }
}
pub const enable_reset_on_sr_v3: ENABLE_RESET_ON_SR_V3 = ENABLE_RESET_ON_SR_V3;

pub struct GATEWAY_REFERENCE_ID_ENABLED_MERCHANT;
impl SC::ServiceConfigKey for GATEWAY_REFERENCE_ID_ENABLED_MERCHANT {
    fn get_key(&self) -> String {
        "gateway_reference_id_enabled_merchant".to_string()
    }
}
pub const gatewayReferenceIdEnabledMerchant: GATEWAY_REFERENCE_ID_ENABLED_MERCHANT =
    GATEWAY_REFERENCE_ID_ENABLED_MERCHANT;

pub struct GATEWAYDECIDER_SCORINGFLOW;
impl SC::ServiceConfigKey for GATEWAYDECIDER_SCORINGFLOW {
    fn get_key(&self) -> String {
        "GatewayDecider::scoringFlow".to_string()
    }
}

pub const paymentFlowsRequiredForGwFiltering: [&str; 12] = [
    "DOTP",
    "CARD_MOTO",
    "MANDATE_REGISTER",
    "MANDATE_PAYMENT",
    "EMANDATE_REGISTER",
    "EMANDATE_PAYMENT",
    "TA_FILE",
    "REVERSE_PENNY_DROP",
    "MUTUAL_FUND",
    "CROSS_BORDER_PAYMENT",
    "SINGLE_BLOCK_MULTIPLE_DEBIT",
    "ONE_TIME_MANDATE",
];

pub const getCardBrandCacheExpiry: i32 = 2 * 24 * 60 * 60;
pub const gatewayScoringData: &str = "gateway_scoring_data_";
pub const globalLevelOutageKeyPrefix: &str = "gw_score_global_outage";
pub const merchantLevelOutageKeyPrefix: &str = "gw_score_outage";

pub struct MERCHANTS_ENABLED_FOR_SCORE_KEYS_UNIFICATION;
impl SC::ServiceConfigKey for MERCHANTS_ENABLED_FOR_SCORE_KEYS_UNIFICATION {
    fn get_key(&self) -> String {
        "merchants_enabled_for_score_keys_unification".to_string()
    }
}
pub const merchantsEnabledForScoreKeysUnification: MERCHANTS_ENABLED_FOR_SCORE_KEYS_UNIFICATION =
    MERCHANTS_ENABLED_FOR_SCORE_KEYS_UNIFICATION;

pub const gateway_selection_order_type_key_prefix: &str = "gw_sr_score";
pub const gateway_selection_v3_order_type_key_prefix: &str = "{gw_sr_v3_score";
pub const gatewayScoreKeysTTL: i64 = 1800;
pub const elimination_based_routing_key_prefix: &str = "gw_score";
pub const elimination_based_routing_global_key_prefix: &str = "gw_score_global";

pub struct GW_REF_ID_SELECTION_BASED_ENABLED_MERCHANT;
impl SC::ServiceConfigKey for GW_REF_ID_SELECTION_BASED_ENABLED_MERCHANT {
    fn get_key(&self) -> String {
        "gw_ref_id_selection_based_enabled_merchant".to_string()
    }
}
pub const gwRefIdSelectionBasedEnabledMerchant: GW_REF_ID_SELECTION_BASED_ENABLED_MERCHANT =
    GW_REF_ID_SELECTION_BASED_ENABLED_MERCHANT;

pub struct ENABLE_SELECTION_BASED_AUTH_TYPE_EVALUATION;
impl SC::ServiceConfigKey for ENABLE_SELECTION_BASED_AUTH_TYPE_EVALUATION {
    fn get_key(&self) -> String {
        "ENABLE_SELECTION_BASED_AUTH_TYPE_EVALUATION".to_string()
    }
}
pub const selectionBasedAuthTypeEnabledMerchant: ENABLE_SELECTION_BASED_AUTH_TYPE_EVALUATION =
    ENABLE_SELECTION_BASED_AUTH_TYPE_EVALUATION;

pub struct ENABLE_SELECTION_BASED_BANK_LEVEL_EVALUATION;
impl SC::ServiceConfigKey for ENABLE_SELECTION_BASED_BANK_LEVEL_EVALUATION {
    fn get_key(&self) -> String {
        "ENABLE_SELECTION_BASED_BANK_LEVEL_EVALUATION".to_string()
    }
}
pub const selectionBasedBankLevelEnabledMerchant: ENABLE_SELECTION_BASED_BANK_LEVEL_EVALUATION =
    ENABLE_SELECTION_BASED_BANK_LEVEL_EVALUATION;

pub struct PUSH_DATA_TO_ROUTING_ETL_STREAM;
impl SC::ServiceConfigKey for PUSH_DATA_TO_ROUTING_ETL_STREAM {
    fn get_key(&self) -> String {
        "push_data_to_routing_ETL_stream".to_string()
    }
}
pub const pushDataToRoutingETLStream: PUSH_DATA_TO_ROUTING_ETL_STREAM =
    PUSH_DATA_TO_ROUTING_ETL_STREAM;

pub struct SR_VOLUME_CHECK_ENABLED_MERCHANT;
impl SC::ServiceConfigKey for SR_VOLUME_CHECK_ENABLED_MERCHANT {
    fn get_key(&self) -> String {
        "SR_VOLUME_CHECK_ENABLED_MERCHANT".to_string()
    }
}
pub const isMerchantEnabledForVolumeCheck: SR_VOLUME_CHECK_ENABLED_MERCHANT =
    SR_VOLUME_CHECK_ENABLED_MERCHANT;

pub const defaultSelectionBucketTxnVolumeThrehold: i64 = 5;

pub struct SR_SELECTION_BUCKET_VOLUME_THRESHOLD;
impl SC::ServiceConfigKey for SR_SELECTION_BUCKET_VOLUME_THRESHOLD {
    fn get_key(&self) -> String {
        "SR_SELECTION_BUCKET_VOLUME_THRESHOLD".to_string()
    }
}
pub const selectionBucketTxnVolumeThreshold: SR_SELECTION_BUCKET_VOLUME_THRESHOLD =
    SR_SELECTION_BUCKET_VOLUME_THRESHOLD;

pub struct ENABLE_MERCHANT_ON_VOLUME_DISTRIBUTION_FEATURE;
impl SC::ServiceConfigKey for ENABLE_MERCHANT_ON_VOLUME_DISTRIBUTION_FEATURE {
    fn get_key(&self) -> String {
        "ENABLE_MERCHANT_ON_VOLUME_DISTRIBUTION_FEATURE".to_string()
    }
}
pub const routeRandomTrafficEnabledMerchant: ENABLE_MERCHANT_ON_VOLUME_DISTRIBUTION_FEATURE =
    ENABLE_MERCHANT_ON_VOLUME_DISTRIBUTION_FEATURE;

pub struct ENABLE_MERCHANT_ON_VOLUME_DISTRIBUTION_FEATURE_SR_V3;
impl SC::ServiceConfigKey for ENABLE_MERCHANT_ON_VOLUME_DISTRIBUTION_FEATURE_SR_V3 {
    fn get_key(&self) -> String {
        "ENABLE_MERCHANT_ON_VOLUME_DISTRIBUTION_FEATURE_SR_V3".to_string()
    }
}
pub const routeRandomTrafficSrV3EnabledMerchant:
    ENABLE_MERCHANT_ON_VOLUME_DISTRIBUTION_FEATURE_SR_V3 =
    ENABLE_MERCHANT_ON_VOLUME_DISTRIBUTION_FEATURE_SR_V3;

pub struct ENABLE_EXPLORE_AND_EXPLOIT_ON_SRV3(pub String);
impl SC::ServiceConfigKey for ENABLE_EXPLORE_AND_EXPLOIT_ON_SRV3 {
    fn get_key(&self) -> String {
        format!("ENABLE_EXPLORE_AND_EXPLOIT_ON_SRV3_{}", self.0)
    }
}
pub fn enableExploreAndExploitOnSrV3(val: String) -> ENABLE_EXPLORE_AND_EXPLOIT_ON_SRV3 {
    ENABLE_EXPLORE_AND_EXPLOIT_ON_SRV3(val)
}

//TODO : This is duplicate and is same key is already there in decider constants.rs
pub struct ENABLE_DEBUG_MODE_ON_SR_V3;
impl SC::ServiceConfigKey for ENABLE_DEBUG_MODE_ON_SR_V3 {
    fn get_key(&self) -> String {
        "ENABLE_DEBUG_MODE_ON_SR_V3".to_string()
    }
}
pub const enableDebugModeOnSrV3: ENABLE_DEBUG_MODE_ON_SR_V3 = ENABLE_DEBUG_MODE_ON_SR_V3;

pub const pendingTxnsKeyPrefix: &str = "PENDING_TXNS_";

pub struct SR_ROUTING_RANDOM_DISTRIBUTION_PERCENTAGE;
impl SC::ServiceConfigKey for SR_ROUTING_RANDOM_DISTRIBUTION_PERCENTAGE {
    fn get_key(&self) -> String {
        "SR_ROUTING_RANDOM_DISTRIBUTION_PERCENTAGE".to_string()
    }
}
pub const srRoutingTrafficRandomDistribution: SR_ROUTING_RANDOM_DISTRIBUTION_PERCENTAGE =
    SR_ROUTING_RANDOM_DISTRIBUTION_PERCENTAGE;

pub const defaultSrRoutingTrafficRandomDistribution: f64 = 10.0;

pub struct WEIGHTED_BLOCK_SR_EVALUATION_ENABLED_MERCHANTS;
impl SC::ServiceConfigKey for WEIGHTED_BLOCK_SR_EVALUATION_ENABLED_MERCHANTS {
    fn get_key(&self) -> String {
        "WEIGHTED_BLOCK_SR_EVALUATION_ENABLED_MERCHANTS".to_string()
    }
}
pub const isWeightedSrEvaluationEnabledMerchant: WEIGHTED_BLOCK_SR_EVALUATION_ENABLED_MERCHANTS =
    WEIGHTED_BLOCK_SR_EVALUATION_ENABLED_MERCHANTS;

pub const defaultWeightsFactorForWeightedSrEvaluation: [(f64, i32); 4] =
    [(1.0, 1), (0.98, 6), (0.92, 18), (0.85, 0)];

pub struct SR_WEIGHT_FACTOR_FOR_WEIGHTED_EVALUATION;
impl SC::ServiceConfigKey for SR_WEIGHT_FACTOR_FOR_WEIGHTED_EVALUATION {
    fn get_key(&self) -> String {
        "SR_WEIGHT_FACTOR_FOR_WEIGHTED_EVALUATION".to_string()
    }
}
pub const selectionWeightsFactorForWeightedSrEvaluation: SR_WEIGHT_FACTOR_FOR_WEIGHTED_EVALUATION =
    SR_WEIGHT_FACTOR_FOR_WEIGHTED_EVALUATION;

pub struct MERCHANT_ENABLED_FOR_ROUTING_EXPERIMENT;
impl SC::ServiceConfigKey for MERCHANT_ENABLED_FOR_ROUTING_EXPERIMENT {
    fn get_key(&self) -> String {
        "MERCHANT_ENABLED_FOR_ROUTING_EXPERIMENT".to_string()
    }
}
pub const isPerformingExperiment: MERCHANT_ENABLED_FOR_ROUTING_EXPERIMENT =
    MERCHANT_ENABLED_FOR_ROUTING_EXPERIMENT;

pub struct HANDLE_PACKAGE_BASED_ROUTING_CUTOVER;
impl SC::ServiceConfigKey for HANDLE_PACKAGE_BASED_ROUTING_CUTOVER {
    fn get_key(&self) -> String {
        "HANDLE_PACKAGE_BASED_ROUTING_CUTOVER".to_string()
    }
}
pub const handleAndPackageBasedRouting: HANDLE_PACKAGE_BASED_ROUTING_CUTOVER =
    HANDLE_PACKAGE_BASED_ROUTING_CUTOVER;

pub struct EDCC_SUPPORTED_GATEWAYS;
impl SC::ServiceConfigKey for EDCC_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        "EDCC_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct MGA_ELIGIBLE_SEAMLESS_GATEWAYS;
impl SC::ServiceConfigKey for MGA_ELIGIBLE_SEAMLESS_GATEWAYS {
    fn get_key(&self) -> String {
        "MGA_ELIGIBLE_SEAMLESS_GATEWAYS".to_string()
    }
}

pub struct AMEX_NOT_SUPPORTED_GATEWAYS;
impl SC::ServiceConfigKey for AMEX_NOT_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        "AMEX_NOT_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct V2_INTEGRATION_NOT_SUPPORTED_GATEWAYS;
impl SC::ServiceConfigKey for V2_INTEGRATION_NOT_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        "V2_INTEGRATION_NOT_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct UPI_INTENT_NOT_SUPPORTED_GATEWAYS;
impl SC::ServiceConfigKey for UPI_INTENT_NOT_SUPPORTED_GATEWAYS {
    fn get_key(&self) -> String {
        "UPI_INTENT_NOT_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct ENABLED_CVVLESS_V2_ENABLED_MERCHANTS;
impl SC::ServiceConfigKey for ENABLED_CVVLESS_V2_ENABLED_MERCHANTS {
    fn get_key(&self) -> String {
        "ENABLED_CVVLESS_V2_ENABLED_MERCHANTS".to_string()
    }
}
pub const cvvLessV2Flow: ENABLED_CVVLESS_V2_ENABLED_MERCHANTS =
    ENABLED_CVVLESS_V2_ENABLED_MERCHANTS;

pub const gatewaysWithTenureBasedCreds: [&str; 3] = ["HDFC", "HDFC_CC_EMI", "ICICI"];

pub struct MERCHANT_CONFIG_ENTITY_LEVEL_LOOKUP_CUTOVER;
impl SC::ServiceConfigKey for MERCHANT_CONFIG_ENTITY_LEVEL_LOOKUP_CUTOVER {
    fn get_key(&self) -> String {
        "MERCHANT_CONFIG_ENTITY_LEVEL_LOOKUP_CUTOVER".to_string()
    }
}
