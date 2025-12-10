use masking::{PeekInterface, Secret};

use crate::redis::types as SC;

pub struct EnableOptimizationDuringDowntime;
impl SC::ServiceConfigKey for EnableOptimizationDuringDowntime {
    fn get_key(&self) -> String {
        "enable_optimization_during_downtime".to_string()
    }
}

pub const ENABLE_OPTIMIZATION_DURING_DOWNTIME: EnableOptimizationDuringDowntime =
    EnableOptimizationDuringDowntime;

pub struct DefaultSrBasedGatewayEliminationInput;
impl SC::ServiceConfigKey for DefaultSrBasedGatewayEliminationInput {
    fn get_key(&self) -> String {
        "DEFAULT_SR_BASED_GATEWAY_ELIMINATION_INPUT".to_string()
    }
}

pub const DEFAULT_SRBASED_GATEWAY_ELIMINATION_INPUT: DefaultSrBasedGatewayEliminationInput =
    DefaultSrBasedGatewayEliminationInput;

//TODO : This is duplicate and is same key is already there in decider constants.rs
pub struct SrV3InputConfig(String);
impl SC::ServiceConfigKey for SrV3InputConfig {
    fn get_key(&self) -> String {
        format!("SR_V3_INPUT_CONFIG_{}", self.0)
    }
}

pub fn srV3InputConfig(mid: String) -> SrV3InputConfig {
    SrV3InputConfig(mid)
}

//TODO : This is duplicate and is same key is already there in decider constants.rs
pub struct SrV3InputConfigDefault;
impl SC::ServiceConfigKey for SrV3InputConfigDefault {
    fn get_key(&self) -> String {
        "SR_V3_INPUT_CONFIG_DEFAULT".to_string()
    }
}

pub const SR_V3_DEFAULT_INPUT_CONFIG: SrV3InputConfigDefault = SrV3InputConfigDefault;

pub const DEFAULT_SR_V3_BASED_BUCKET_SIZE: i32 = 125;
pub const DEFAULT_SR_V3_BASED_UPPER_RESET_FACTOR: f64 = 3.0;
pub const DEFAULT_SR_V3_BASED_LOWER_RESET_FACTOR: f64 = 3.0;
pub const DEFAULT_SR_V3_BASED_HEDGING_PERCENT: f64 = 5.0;
pub const DEFAULT_SR_V3_BASED_GATEWAY_SIGMA_FACTOR: f64 = 0.0;

pub struct AltIdEnabledGatewayForEmibank;
impl SC::ServiceConfigKey for AltIdEnabledGatewayForEmibank {
    fn get_key(&self) -> String {
        "ALT_ID_ENABLED_GATEWAY_FOR_EMIBANK".to_string()
    }
}

pub const ALT_ID_ENABLED_GATEWAY_EMI_BANK: AltIdEnabledGatewayForEmibank =
    AltIdEnabledGatewayForEmibank;

pub struct SrBasedTransactionResetCount;
impl SC::ServiceConfigKey for SrBasedTransactionResetCount {
    fn get_key(&self) -> String {
        "SR_BASED_TRANSACTION_RESET_COUNT".to_string()
    }
}

pub const SR_BASED_TXN_RESET_COUNT: SrBasedTransactionResetCount = SrBasedTransactionResetCount;

pub struct ScheduledOutageValidationDuration;
impl SC::ServiceConfigKey for ScheduledOutageValidationDuration {
    fn get_key(&self) -> String {
        "SCHEDULED_OUTAGE_VALIDATION_DURATION".to_string()
    }
}

pub struct EnableEliminationV2;
impl SC::ServiceConfigKey for EnableEliminationV2 {
    fn get_key(&self) -> String {
        "ENABLE_ELIMINATION_V2".to_string()
    }
}

pub const ENABLE_ELIMINATION_V2: EnableEliminationV2 = EnableEliminationV2;

pub struct EnableOutageV2;
impl SC::ServiceConfigKey for EnableOutageV2 {
    fn get_key(&self) -> String {
        "ENABLE_OUTAGE_V2".to_string()
    }
}

pub const ENABLE_ELIMINATION_V2_FOR_OUTAGE: EnableOutageV2 = EnableOutageV2;

pub const THRESHOLD_WEIGHT_SR1: &str = "THRESHOLD_WEIGHT_SR1";
pub const THRESHOLD_WEIGHT_SR2: &str = "THRESHOLD_WEIGHT_SR2";

pub struct DefaultSr1(String);
impl SC::ServiceConfigKey for DefaultSr1 {
    fn get_key(&self) -> String {
        format!("DEFAULT_SR1_{}", self.0)
    }
}

pub fn defaultSr1SConfigPrefix(val: String) -> DefaultSr1 {
    DefaultSr1(val)
}

pub struct DefaultN(String);
impl SC::ServiceConfigKey for DefaultN {
    fn get_key(&self) -> String {
        format!("DEFAULT_N_{}", self.0)
    }
}

pub fn defaultNSConfigPrefix(val: String) -> DefaultN {
    DefaultN(val)
}

pub struct InternalDefaultEliminationV2SuccessRate1AndN(String);
impl SC::ServiceConfigKey for InternalDefaultEliminationV2SuccessRate1AndN {
    fn get_key(&self) -> String {
        format!(
            "INTERNAL_DEFAULT_ELIMINATION_V2_SUCCESS_RATE_1_AND_N_{}",
            self.0
        )
    }
}

pub fn internalDefaultEliminationV2SuccessRate1AndNPrefix(
    val: String,
) -> InternalDefaultEliminationV2SuccessRate1AndN {
    InternalDefaultEliminationV2SuccessRate1AndN(val)
}

pub const DEFAULT_FIELD_NAME_FOR_SR1_AND_N: &str = "default";
pub const SR1_KEY_PREFIX: &str = "sr1_";
pub const N_KEY_PREFIX: &str = "n_";

pub const GW_DEFAULT_TXN_SOFT_RESET_COUNT: i64 = 10;
pub const DEFAULT_GLOBAL_SELECTION_VOLUME_THRESHOLD: i64 = 20;

pub struct GatewayResetScoreEnabled;
impl SC::ServiceConfigKey for GatewayResetScoreEnabled {
    fn get_key(&self) -> String {
        "gateway_reset_score_enabled".to_string()
    }
}

pub const GW_RESET_SCORE_ENABLED: GatewayResetScoreEnabled = GatewayResetScoreEnabled;

pub const DEF_SRBASED_GW_LEVEL_ELIMINATION_THRESHOLD: f64 = 0.02;
pub const DEFAULT_GLOBAL_SELECTION_MAX_COUNT_THRESHOLD: i64 = 5;

pub struct GatewayScoreFirstDimensionSoftTtl;
impl SC::ServiceConfigKey for GatewayScoreFirstDimensionSoftTtl {
    fn get_key(&self) -> String {
        "gateway_score_first_dimension_soft_ttl".to_string()
    }
}

pub const GW_SCORE_FIRST_DIMENSION_TTL: GatewayScoreFirstDimensionSoftTtl =
    GatewayScoreFirstDimensionSoftTtl;

pub struct GatewayScoreSecondDimensionSoftTtl;
impl SC::ServiceConfigKey for GatewayScoreSecondDimensionSoftTtl {
    fn get_key(&self) -> String {
        "gateway_score_second_dimension_soft_ttl".to_string()
    }
}

pub const GW_SCORE_SECOND_DIMENSION_TTL: GatewayScoreSecondDimensionSoftTtl =
    GatewayScoreSecondDimensionSoftTtl;

pub struct ShouldConsumeResultFromRouter;
impl SC::ServiceConfigKey for ShouldConsumeResultFromRouter {
    fn get_key(&self) -> String {
        "SHOULD_CONSUME_RESULT_FROM_ROUTER".to_string()
    }
}

pub const SHOULD_CONSUME_RESULT_FROM_ROUTER: ShouldConsumeResultFromRouter =
    ShouldConsumeResultFromRouter;

pub struct GatewayScoreThirdDimensionSoftTtl;
impl SC::ServiceConfigKey for GatewayScoreThirdDimensionSoftTtl {
    fn get_key(&self) -> String {
        "gateway_score_third_dimension_soft_ttl".to_string()
    }
}

pub const GW_SCORE_THIRD_DIMENSION_TTL: GatewayScoreThirdDimensionSoftTtl =
    GatewayScoreThirdDimensionSoftTtl;

pub struct GatewayScoreFourthDimensionSoftTtl;
impl SC::ServiceConfigKey for GatewayScoreFourthDimensionSoftTtl {
    fn get_key(&self) -> String {
        "gateway_score_fourth_dimension_soft_ttl".to_string()
    }
}

pub const GW_SCORE_FOURTH_DIMENSION_TTL: GatewayScoreFourthDimensionSoftTtl =
    GatewayScoreFourthDimensionSoftTtl;

pub const DEF_SCORE_KEYS_TTL: f64 = 900000.0;

pub struct IsGbesv2Enabled;
impl SC::ServiceConfigKey for IsGbesv2Enabled {
    fn get_key(&self) -> String {
        "IS_GBESV2_ENABLED".to_string()
    }
}

pub const GBES_V2_ENABLED: IsGbesv2Enabled = IsGbesv2Enabled;

pub struct EnableGatewayLevelSrElimination;
impl SC::ServiceConfigKey for EnableGatewayLevelSrElimination {
    fn get_key(&self) -> String {
        "enable_gateway_level_sr_elimination".to_string()
    }
}

pub const ENABLE_GW_LEVEL_SR_ELIMINATION: EnableGatewayLevelSrElimination =
    EnableGatewayLevelSrElimination;

pub struct SrBasedGatewayEliminationThreshold;
impl SC::ServiceConfigKey for SrBasedGatewayEliminationThreshold {
    fn get_key(&self) -> String {
        "SR_BASED_GATEWAY_ELIMINATION_THRESHOLD".to_string()
    }
}

pub const SR_BASED_GATEWAY_ELIMINATION_THRESHOLD: SrBasedGatewayEliminationThreshold =
    SrBasedGatewayEliminationThreshold;

pub const DEFAULT_SR_BASED_GATEWAY_ELIMINATION_THRESHOLD: f64 = 0.05;

pub struct OtpCardInfoRestrictedGateways;
impl SC::ServiceConfigKey for OtpCardInfoRestrictedGateways {
    fn get_key(&self) -> String {
        "OTP_CARD_INFO_RESTRICTED_GATEWAYS".to_string()
    }
}

pub struct AuthTypeRestrictedGateways;
impl SC::ServiceConfigKey for AuthTypeRestrictedGateways {
    fn get_key(&self) -> String {
        "AUTH_TYPE_RESTRICTED_GATEWAYS".to_string()
    }
}

pub struct CardEmiExplicitGateways;
impl SC::ServiceConfigKey for CardEmiExplicitGateways {
    fn get_key(&self) -> String {
        "CARD_EMI_EXPLICIT_GATEWAYS".to_string()
    }
}

pub struct ConsumerFinanceOnlyGateways;
impl SC::ServiceConfigKey for ConsumerFinanceOnlyGateways {
    fn get_key(&self) -> String {
        "CONSUMER_FINANCE_ONLY_GATEWAYS".to_string()
    }
}

pub struct ConsumerFinanceAlsoGateways;
impl SC::ServiceConfigKey for ConsumerFinanceAlsoGateways {
    fn get_key(&self) -> String {
        "CONSUMER_FINANCE_ALSO_GATEWAYS".to_string()
    }
}

pub struct MutualFundFlowSupportedGateways;
impl SC::ServiceConfigKey for MutualFundFlowSupportedGateways {
    fn get_key(&self) -> String {
        "MUTUAL_FUND_FLOW_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct CrossBorderFlowSupportedGateways;
impl SC::ServiceConfigKey for CrossBorderFlowSupportedGateways {
    fn get_key(&self) -> String {
        "CROSS_BORDER_FLOW_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct SbmdSupportedGateways;
impl SC::ServiceConfigKey for SbmdSupportedGateways {
    fn get_key(&self) -> String {
        "SBMD_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct SplitSettlementSupportedGateways;
impl SC::ServiceConfigKey for SplitSettlementSupportedGateways {
    fn get_key(&self) -> String {
        "SPLIT_SETTLEMENT_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct TpvOnlySupportedGateways;
impl SC::ServiceConfigKey for TpvOnlySupportedGateways {
    fn get_key(&self) -> String {
        "TPV_ONLY_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct NbOnlyGateways;
impl SC::ServiceConfigKey for NbOnlyGateways {
    fn get_key(&self) -> String {
        "NB_ONLY_GATEWAYS".to_string()
    }
}

pub struct UpiOnlyGateways;
impl SC::ServiceConfigKey for UpiOnlyGateways {
    fn get_key(&self) -> String {
        "UPI_ONLY_GATEWAYS".to_string()
    }
}

pub struct UpiAlsoGateways;
impl SC::ServiceConfigKey for UpiAlsoGateways {
    fn get_key(&self) -> String {
        "UPI_ALSO_GATEWAYS".to_string()
    }
}

pub struct WalletOnlyGateways;
impl SC::ServiceConfigKey for WalletOnlyGateways {
    fn get_key(&self) -> String {
        "WALLET_ONLY_GATEWAYS".to_string()
    }
}

pub struct WalletAlsoGateways;
impl SC::ServiceConfigKey for WalletAlsoGateways {
    fn get_key(&self) -> String {
        "WALLET_ALSO_GATEWAYS".to_string()
    }
}

pub struct NoOrLowCostEmiSupportedGateways;
impl SC::ServiceConfigKey for NoOrLowCostEmiSupportedGateways {
    fn get_key(&self) -> String {
        "NO_OR_LOW_COST_EMI_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct SiOnEmiCardSupportedGateways;
impl SC::ServiceConfigKey for SiOnEmiCardSupportedGateways {
    fn get_key(&self) -> String {
        "SI_ON_EMI_CARD_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct SiOnEmiDisabledCardBrandGatewayMapping;
impl SC::ServiceConfigKey for SiOnEmiDisabledCardBrandGatewayMapping {
    fn get_key(&self) -> String {
        "SI_ON_EMI_DISABLED_CARD_BRAND_GATEWAY_MAPPING".to_string()
    }
}

pub struct TokenProviderGatewayMapping;
impl SC::ServiceConfigKey for TokenProviderGatewayMapping {
    fn get_key(&self) -> String {
        "TOKEN_PROVIDER_GATEWAY_MAPPING".to_string()
    }
}

pub struct TxnTypeGatewayMapping;
impl SC::ServiceConfigKey for TxnTypeGatewayMapping {
    fn get_key(&self) -> String {
        "TXN_TYPE_GATEWAY_MAPPING".to_string()
    }
}

pub struct TxnDetailTypeRestrictedGateways;
impl SC::ServiceConfigKey for TxnDetailTypeRestrictedGateways {
    fn get_key(&self) -> String {
        "TXN_DETAIL_TYPE_RESTRICTED_GATEWAYS".to_string()
    }
}

pub struct RewardOnlyGateways;
impl SC::ServiceConfigKey for RewardOnlyGateways {
    fn get_key(&self) -> String {
        "REWARD_ONLY_GATEWAYS".to_string()
    }
}

pub struct RewardAlsoGateways;
impl SC::ServiceConfigKey for RewardAlsoGateways {
    fn get_key(&self) -> String {
        "REWARD_ALSO_GATEWAYS".to_string()
    }
}

pub struct SodexoOnlyGateways;
impl SC::ServiceConfigKey for SodexoOnlyGateways {
    fn get_key(&self) -> String {
        "SODEXO_ONLY_GATEWAYS".to_string()
    }
}

pub struct SodexoAlsoGateways;
impl SC::ServiceConfigKey for SodexoAlsoGateways {
    fn get_key(&self) -> String {
        "SODEXO_ALSO_GATEWAYS".to_string()
    }
}

pub struct CashOnlyGateways;
impl SC::ServiceConfigKey for CashOnlyGateways {
    fn get_key(&self) -> String {
        "CASH_ONLY_GATEWAYS".to_string()
    }
}

pub struct AmexSupportedGateways;
impl SC::ServiceConfigKey for AmexSupportedGateways {
    fn get_key(&self) -> String {
        "AMEX_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct CardBrandToCvvlessTxnSupportedGateways;
impl SC::ServiceConfigKey for CardBrandToCvvlessTxnSupportedGateways {
    fn get_key(&self) -> String {
        "CARD_BRAND_TO_CVVLESS_TXN_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct CvvlessTxnSupportedCommonGateways;
impl SC::ServiceConfigKey for CvvlessTxnSupportedCommonGateways {
    fn get_key(&self) -> String {
        "CVVLESS_TXN_SUPPORTED_COMMON_GATEWAYS".to_string()
    }
}

pub struct MerchantContainerSupportedGateways;
impl SC::ServiceConfigKey for MerchantContainerSupportedGateways {
    fn get_key(&self) -> String {
        "MERCHANT_CONTAINER_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct TokenSupportedGateways(pub String, pub Option<String>, pub String, pub String);
impl SC::ServiceConfigKey for TokenSupportedGateways {
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

pub struct TokenRepeatOtpSupportedGateways(String);
impl SC::ServiceConfigKey for TokenRepeatOtpSupportedGateways {
    fn get_key(&self) -> String {
        format!("{}_TOKEN_REPEAT_OTP_SUPPORTED_GATEWAYS", self.0)
    }
}

pub fn getTokenRepeatOtpGatewayKey(val: String) -> TokenRepeatOtpSupportedGateways {
    TokenRepeatOtpSupportedGateways(val)
}

pub struct TokenRepeatCvvlessSupportedGateways(String);
impl SC::ServiceConfigKey for TokenRepeatCvvlessSupportedGateways {
    fn get_key(&self) -> String {
        format!("{}_TOKEN_REPEAT_CVVLESS_SUPPORTED_GATEWAYS", self.0)
    }
}
pub fn getTokenRepeatCvvLessGatewayKey(val: String) -> TokenRepeatCvvlessSupportedGateways {
    TokenRepeatCvvlessSupportedGateways(val)
}

pub struct TokenRepeatMandateSupportedGateways(String);
impl SC::ServiceConfigKey for TokenRepeatMandateSupportedGateways {
    fn get_key(&self) -> String {
        format!("{}_TOKEN_REPEAT_MANDATE_SUPPORTED_GATEWAYS", self.0)
    }
}

pub fn getTokenRepeatMandateGatewayKey(val: String) -> TokenRepeatMandateSupportedGateways {
    TokenRepeatMandateSupportedGateways(val)
}

pub struct TokenRepeatSupportedGateways(String);
impl SC::ServiceConfigKey for TokenRepeatSupportedGateways {
    fn get_key(&self) -> String {
        format!("{}_TOKEN_REPEAT_SUPPORTED_GATEWAYS", self.0)
    }
}

pub fn getTokenRepeatGatewayKey(val: String) -> TokenRepeatSupportedGateways {
    TokenRepeatSupportedGateways(val)
}

pub struct MandateGuestCheckoutSupportedGateways(Option<Secret<String>>);
impl SC::ServiceConfigKey for MandateGuestCheckoutSupportedGateways {
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
) -> MandateGuestCheckoutSupportedGateways {
    MandateGuestCheckoutSupportedGateways(val)
}

pub struct TokenRepeatCvvlessSupportedBanks(Option<Secret<String>>);
impl SC::ServiceConfigKey for TokenRepeatCvvlessSupportedBanks {
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
) -> TokenRepeatCvvlessSupportedBanks {
    TokenRepeatCvvlessSupportedBanks(val)
}

pub struct EmiBinValidationSupportedBanks;
impl SC::ServiceConfigKey for EmiBinValidationSupportedBanks {
    fn get_key(&self) -> String {
        "EMI_BIN_VALIDATION_SUPPORTED_BANKS".to_string()
    }
}

pub const GET_EMI_BIN_VALIDATION_SUPPORTED_BANKS_KEY: EmiBinValidationSupportedBanks =
    EmiBinValidationSupportedBanks;

pub struct MetricTrackingLog;
impl SC::ServiceConfigKey for MetricTrackingLog {
    fn get_key(&self) -> String {
        "METRIC_TRACKING_LOG".to_string()
    }
}
pub const METRIC_TRACKING_LOG_DATA_KEY: MetricTrackingLog = MetricTrackingLog;

pub struct V2RoutingHandleList;
impl SC::ServiceConfigKey for V2RoutingHandleList {
    fn get_key(&self) -> String {
        "V2_ROUTING_HANDLE_LIST".to_string()
    }
}
pub const V2_ROUTING_HANDLE_LIST: V2RoutingHandleList = V2RoutingHandleList;

pub struct V2RoutingPspList;
impl SC::ServiceConfigKey for V2RoutingPspList {
    fn get_key(&self) -> String {
        "V2_ROUTING_PSP_LIST".to_string()
    }
}
pub const V2_ROUTING_PSP_LIST: V2RoutingPspList = V2RoutingPspList;

pub struct V2RoutingTopBankList;
impl SC::ServiceConfigKey for V2RoutingTopBankList {
    fn get_key(&self) -> String {
        "V2_ROUTING_TOP_BANK_LIST".to_string()
    }
}
pub const V2_ROUTING_TOP_BANK_LIST: V2RoutingTopBankList = V2RoutingTopBankList;

pub struct V2RoutingPspPackageList;
impl SC::ServiceConfigKey for V2RoutingPspPackageList {
    fn get_key(&self) -> String {
        "V2_ROUTING_PSP_PACKAGE_LIST".to_string()
    }
}
pub const V2_ROUTING_PSP_PACKAGE_LIST: V2RoutingPspPackageList = V2RoutingPspPackageList;

pub struct OptimizationRoutingConfig(pub String);
impl SC::ServiceConfigKey for OptimizationRoutingConfig {
    fn get_key(&self) -> String {
        format!("{}_optimization_routing_config", self.0)
    }
}

pub struct DefaultOptimizationRoutingConfig;
impl SC::ServiceConfigKey for DefaultOptimizationRoutingConfig {
    fn get_key(&self) -> String {
        "default_optimization_routing_config".to_string()
    }
}

pub struct AtmPinCardInfoRestrictedGateways;
impl SC::ServiceConfigKey for AtmPinCardInfoRestrictedGateways {
    fn get_key(&self) -> String {
        "ATM_PIN_CARD_INFO_RESTRICTED_GATEWAYS".to_string()
    }
}

pub struct OtpCardInfoSupportedGateways;
impl SC::ServiceConfigKey for OtpCardInfoSupportedGateways {
    fn get_key(&self) -> String {
        "OTP_CARD_INFO_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct MotoCardInfoSupportedGateways;
impl SC::ServiceConfigKey for MotoCardInfoSupportedGateways {
    fn get_key(&self) -> String {
        "MOTO_CARD_INFO_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct TaOfflineEnabledGateways;
impl SC::ServiceConfigKey for TaOfflineEnabledGateways {
    fn get_key(&self) -> String {
        "TA_OFFLINE_ENABLED_GATEWAYS".to_string()
    }
}

pub struct MerchantWiseMandateBinEnforcedGateways;
impl SC::ServiceConfigKey for MerchantWiseMandateBinEnforcedGateways {
    fn get_key(&self) -> String {
        "MERCHANT_WISE_MANDATE_BIN_ENFORCED_GATEWAYS".to_string()
    }
}

pub struct MerchantWiseAuthTypeBinEnforcedGateways;
impl SC::ServiceConfigKey for MerchantWiseAuthTypeBinEnforcedGateways {
    fn get_key(&self) -> String {
        "MERCHANT_WISE_AUTH_TYPE_BIN_ENFORCED_GATEWAYS".to_string()
    }
}

pub struct CardMandateBinFilterExcludedGateways;
impl SC::ServiceConfigKey for CardMandateBinFilterExcludedGateways {
    fn get_key(&self) -> String {
        "CARD_MANDATE_BIN_FILTER_EXCLUDED_GATEWAYS".to_string()
    }
}

pub struct EnableGatewaySelectionBasedOnOptimizedSrInput(pub String);
impl SC::ServiceConfigKey for EnableGatewaySelectionBasedOnOptimizedSrInput {
    fn get_key(&self) -> String {
        format!(
            "ENABLE_GATEWAY_SELECTION_BASED_ON_OPTIMIZED_SR_INPUT_{}",
            self.0
        )
    }
}
pub fn enable_gateway_selection_based_on_optimized_sr_input(
    val: String,
) -> EnableGatewaySelectionBasedOnOptimizedSrInput {
    EnableGatewaySelectionBasedOnOptimizedSrInput(val)
}

pub struct EnableBetaDistributionOnSrV3;
impl SC::ServiceConfigKey for EnableBetaDistributionOnSrV3 {
    fn get_key(&self) -> String {
        "ENABLE_BETA_DISTRIBUTION_ON_SR_V3".to_string()
    }
}
pub const ENABLE_BETA_DISTRIBUTION_ON_SR_V3: EnableBetaDistributionOnSrV3 =
    EnableBetaDistributionOnSrV3;

pub struct EnableGatewaySelectionBasedOnSrV3Input(pub String);
impl SC::ServiceConfigKey for EnableGatewaySelectionBasedOnSrV3Input {
    fn get_key(&self) -> String {
        format!("ENABLE_GATEWAY_SELECTION_BASED_ON_SR_V3_INPUT_{}", self.0)
    }
}
pub fn enable_gateway_selection_based_on_sr_v3_input(
    val: String,
) -> EnableGatewaySelectionBasedOnSrV3Input {
    EnableGatewaySelectionBasedOnSrV3Input(val)
}

pub struct EnableBinomialDistributionOnSrV3;
impl SC::ServiceConfigKey for EnableBinomialDistributionOnSrV3 {
    fn get_key(&self) -> String {
        "ENABLE_BINOMIAL_DISTRIBUTION_ON_SR_V3".to_string()
    }
}
pub const ENABLE_BINOMIAL_DISTRIBUTION_ON_SR_V3: EnableBinomialDistributionOnSrV3 =
    EnableBinomialDistributionOnSrV3;

pub struct EnableExtraScoreOnSrV3;
impl SC::ServiceConfigKey for EnableExtraScoreOnSrV3 {
    fn get_key(&self) -> String {
        "ENABLE_EXTRA_SCORE_ON_SR_V3".to_string()
    }
}
pub const ENABLE_EXTRA_SCORE_ON_SR_V3: EnableExtraScoreOnSrV3 = EnableExtraScoreOnSrV3;

pub struct EnableResetOnSrV3;
impl SC::ServiceConfigKey for EnableResetOnSrV3 {
    fn get_key(&self) -> String {
        "ENABLE_RESET_ON_SR_V3".to_string()
    }
}
pub const ENABLE_RESET_ON_SR_V3: EnableResetOnSrV3 = EnableResetOnSrV3;

pub struct GatewayReferenceIdEnabledMerchant;
impl SC::ServiceConfigKey for GatewayReferenceIdEnabledMerchant {
    fn get_key(&self) -> String {
        "gateway_reference_id_enabled_merchant".to_string()
    }
}
pub const GATEWAY_REFERENCE_ID_ENABLED_MERCHANT: GatewayReferenceIdEnabledMerchant =
    GatewayReferenceIdEnabledMerchant;

pub struct GatewaydeciderScoringflow;
impl SC::ServiceConfigKey for GatewaydeciderScoringflow {
    fn get_key(&self) -> String {
        "GatewayDecider::scoringFlow".to_string()
    }
}

pub const PAYMENT_FLOWS_REQUIRED_FOR_GW_FILTERING: [&str; 13] = [
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
    "PIX_AUTOMATIC_REDIRECT",
];

pub const GET_CARD_BRAND_CACHE_EXPIRY: i32 = 2 * 24 * 60 * 60;
pub const GATEWAY_SCORING_DATA: &str = "gateway_scoring_data_";
pub const GLOBAL_LEVEL_OUTAGE_KEY_PREFIX: &str = "gw_score_global_outage";
pub const MERCHANT_LEVEL_OUTAGE_KEY_PREFIX: &str = "gw_score_outage";

pub struct MerchantsEnabledForScoreKeysUnification;
impl SC::ServiceConfigKey for MerchantsEnabledForScoreKeysUnification {
    fn get_key(&self) -> String {
        "merchants_enabled_for_score_keys_unification".to_string()
    }
}
pub const MERCHANTS_ENABLED_FOR_SCORE_KEYS_UNIFICATION: MerchantsEnabledForScoreKeysUnification =
    MerchantsEnabledForScoreKeysUnification;

pub const GATEWAY_SELECTION_ORDER_TYPE_KEY_PREFIX: &str = "gw_sr_score";
pub const GATEWAY_SELECTION_V3_ORDER_TYPE_KEY_PREFIX: &str = "{gw_sr_v3_score";
pub const GATEWAY_SCORE_KEYS_TTL: i64 = 1800;
pub const ELIMINATION_BASED_ROUTING_KEY_PREFIX: &str = "gw_score";
pub const ELIMINATION_BASED_ROUTING_GLOBAL_KEY_PREFIX: &str = "gw_score_global";

pub struct GwRefIdSelectionBasedEnabledMerchant;
impl SC::ServiceConfigKey for GwRefIdSelectionBasedEnabledMerchant {
    fn get_key(&self) -> String {
        "gw_ref_id_selection_based_enabled_merchant".to_string()
    }
}
pub const GW_REF_ID_SELECTION_BASED_ENABLED_MERCHANT: GwRefIdSelectionBasedEnabledMerchant =
    GwRefIdSelectionBasedEnabledMerchant;

pub struct EnableSelectionBasedAuthTypeEvaluation;
impl SC::ServiceConfigKey for EnableSelectionBasedAuthTypeEvaluation {
    fn get_key(&self) -> String {
        "ENABLE_SELECTION_BASED_AUTH_TYPE_EVALUATION".to_string()
    }
}
pub const SELECTION_BASED_AUTH_TYPE_ENABLED_MERCHANT: EnableSelectionBasedAuthTypeEvaluation =
    EnableSelectionBasedAuthTypeEvaluation;

pub struct EnableSelectionBasedBankLevelEvaluation;
impl SC::ServiceConfigKey for EnableSelectionBasedBankLevelEvaluation {
    fn get_key(&self) -> String {
        "ENABLE_SELECTION_BASED_BANK_LEVEL_EVALUATION".to_string()
    }
}
pub const SELECTION_BASED_BANK_LEVEL_ENABLED_MERCHANT: EnableSelectionBasedBankLevelEvaluation =
    EnableSelectionBasedBankLevelEvaluation;

pub struct PushDataToRoutingEtlStream;
impl SC::ServiceConfigKey for PushDataToRoutingEtlStream {
    fn get_key(&self) -> String {
        "push_data_to_routing_ETL_stream".to_string()
    }
}
pub const PUSH_DATA_TO_ROUTING_ETLSTREAM: PushDataToRoutingEtlStream = PushDataToRoutingEtlStream;

pub struct SrVolumeCheckEnabledMerchant;
impl SC::ServiceConfigKey for SrVolumeCheckEnabledMerchant {
    fn get_key(&self) -> String {
        "SR_VOLUME_CHECK_ENABLED_MERCHANT".to_string()
    }
}
pub const IS_MERCHANT_ENABLED_FOR_VOLUME_CHECK: SrVolumeCheckEnabledMerchant =
    SrVolumeCheckEnabledMerchant;

pub const DEFAULT_SELECTION_BUCKET_TXN_VOLUME_THREHOLD: i64 = 5;

pub struct SrSelectionBucketVolumeThreshold;
impl SC::ServiceConfigKey for SrSelectionBucketVolumeThreshold {
    fn get_key(&self) -> String {
        "SR_SELECTION_BUCKET_VOLUME_THRESHOLD".to_string()
    }
}
pub const SELECTION_BUCKET_TXN_VOLUME_THRESHOLD: SrSelectionBucketVolumeThreshold =
    SrSelectionBucketVolumeThreshold;

pub struct EnableMerchantOnVolumeDistributionFeature;
impl SC::ServiceConfigKey for EnableMerchantOnVolumeDistributionFeature {
    fn get_key(&self) -> String {
        "ENABLE_MERCHANT_ON_VOLUME_DISTRIBUTION_FEATURE".to_string()
    }
}
pub const ROUTE_RANDOM_TRAFFIC_ENABLED_MERCHANT: EnableMerchantOnVolumeDistributionFeature =
    EnableMerchantOnVolumeDistributionFeature;

pub struct EnableMerchantOnVolumeDistributionFeatureSrV3;
impl SC::ServiceConfigKey for EnableMerchantOnVolumeDistributionFeatureSrV3 {
    fn get_key(&self) -> String {
        "ENABLE_MERCHANT_ON_VOLUME_DISTRIBUTION_FEATURE_SR_V3".to_string()
    }
}
pub const ROUTE_RANDOM_TRAFFIC_SR_V3_ENABLED_MERCHANT:
    EnableMerchantOnVolumeDistributionFeatureSrV3 = EnableMerchantOnVolumeDistributionFeatureSrV3;

pub struct EnableExploreAndExploitOnSrv3(pub String);
impl SC::ServiceConfigKey for EnableExploreAndExploitOnSrv3 {
    fn get_key(&self) -> String {
        format!("ENABLE_EXPLORE_AND_EXPLOIT_ON_SRV3_{}", self.0)
    }
}
pub fn enableExploreAndExploitOnSrV3(val: String) -> EnableExploreAndExploitOnSrv3 {
    EnableExploreAndExploitOnSrv3(val)
}

//TODO : This is duplicate and is same key is already there in decider constants.rs
pub struct EnableDebugModeOnSrV3;
impl SC::ServiceConfigKey for EnableDebugModeOnSrV3 {
    fn get_key(&self) -> String {
        "ENABLE_DEBUG_MODE_ON_SR_V3".to_string()
    }
}
pub const ENABLE_DEBUG_MODE_ON_SR_V3: EnableDebugModeOnSrV3 = EnableDebugModeOnSrV3;

pub const PENDING_TXNS_KEY_PREFIX: &str = "PENDING_TXNS_";

pub struct SrRoutingRandomDistributionPercentage;
impl SC::ServiceConfigKey for SrRoutingRandomDistributionPercentage {
    fn get_key(&self) -> String {
        "SR_ROUTING_RANDOM_DISTRIBUTION_PERCENTAGE".to_string()
    }
}
pub const SR_ROUTING_TRAFFIC_RANDOM_DISTRIBUTION: SrRoutingRandomDistributionPercentage =
    SrRoutingRandomDistributionPercentage;

pub const DEFAULT_SR_ROUTING_TRAFFIC_RANDOM_DISTRIBUTION: f64 = 10.0;

pub struct WeightedBlockSrEvaluationEnabledMerchants;
impl SC::ServiceConfigKey for WeightedBlockSrEvaluationEnabledMerchants {
    fn get_key(&self) -> String {
        "WEIGHTED_BLOCK_SR_EVALUATION_ENABLED_MERCHANTS".to_string()
    }
}
pub const IS_WEIGHTED_SR_EVALUATION_ENABLED_MERCHANT: WeightedBlockSrEvaluationEnabledMerchants =
    WeightedBlockSrEvaluationEnabledMerchants;

pub const DEFAULT_WEIGHTS_FACTOR_FOR_WEIGHTED_SR_EVALUATION: [(f64, i32); 4] =
    [(1.0, 1), (0.98, 6), (0.92, 18), (0.85, 0)];

pub struct SrWeightFactorForWeightedEvaluation;
impl SC::ServiceConfigKey for SrWeightFactorForWeightedEvaluation {
    fn get_key(&self) -> String {
        "SR_WEIGHT_FACTOR_FOR_WEIGHTED_EVALUATION".to_string()
    }
}
pub const SELECTION_WEIGHTS_FACTOR_FOR_WEIGHTED_SR_EVALUATION: SrWeightFactorForWeightedEvaluation =
    SrWeightFactorForWeightedEvaluation;

pub struct MerchantEnabledForRoutingExperiment;
impl SC::ServiceConfigKey for MerchantEnabledForRoutingExperiment {
    fn get_key(&self) -> String {
        "MERCHANT_ENABLED_FOR_ROUTING_EXPERIMENT".to_string()
    }
}
pub const IS_PERFORMING_EXPERIMENT: MerchantEnabledForRoutingExperiment =
    MerchantEnabledForRoutingExperiment;

pub struct HandlePackageBasedRoutingCutover;
impl SC::ServiceConfigKey for HandlePackageBasedRoutingCutover {
    fn get_key(&self) -> String {
        "HANDLE_PACKAGE_BASED_ROUTING_CUTOVER".to_string()
    }
}
pub const HANDLE_AND_PACKAGE_BASED_ROUTING: HandlePackageBasedRoutingCutover =
    HandlePackageBasedRoutingCutover;

pub struct EdccSupportedGateways;
impl SC::ServiceConfigKey for EdccSupportedGateways {
    fn get_key(&self) -> String {
        "EDCC_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct MgaEligibleSeamlessGateways;
impl SC::ServiceConfigKey for MgaEligibleSeamlessGateways {
    fn get_key(&self) -> String {
        "MGA_ELIGIBLE_SEAMLESS_GATEWAYS".to_string()
    }
}

pub struct AmexNotSupportedGateways;
impl SC::ServiceConfigKey for AmexNotSupportedGateways {
    fn get_key(&self) -> String {
        "AMEX_NOT_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct V2IntegrationNotSupportedGateways;
impl SC::ServiceConfigKey for V2IntegrationNotSupportedGateways {
    fn get_key(&self) -> String {
        "V2_INTEGRATION_NOT_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct UpiIntentNotSupportedGateways;
impl SC::ServiceConfigKey for UpiIntentNotSupportedGateways {
    fn get_key(&self) -> String {
        "UPI_INTENT_NOT_SUPPORTED_GATEWAYS".to_string()
    }
}

pub struct EnabledCvvlessV2EnabledMerchants;
impl SC::ServiceConfigKey for EnabledCvvlessV2EnabledMerchants {
    fn get_key(&self) -> String {
        "ENABLED_CVVLESS_V2_ENABLED_MERCHANTS".to_string()
    }
}
pub const CVV_LESS_V2_FLOW: EnabledCvvlessV2EnabledMerchants = EnabledCvvlessV2EnabledMerchants;

pub const GATEWAYS_WITH_TENURE_BASED_CREDS: [&str; 3] = ["HDFC", "HDFC_CC_EMI", "ICICI"];

pub const PIX_PAYMENT_FLOWS: [&str; 1] = ["PIX_AUTOMATIC_REDIRECT"];

pub struct MerchantConfigEntityLevelLookupCutover;
impl SC::ServiceConfigKey for MerchantConfigEntityLevelLookupCutover {
    fn get_key(&self) -> String {
        "MERCHANT_CONFIG_ENTITY_LEVEL_LOOKUP_CUTOVER".to_string()
    }
}

