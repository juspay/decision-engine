#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use crate::{
        config::{
            ConnectorFilters, CurrencyCountryFlowFilter, NotAvailableFlows, PaymentMethodFilters,
        },
        euclid::{
            ast::{ConnectorInfo, Output, ValueType, VolumeSplit},
            handlers::routing_rules::{
                apply_pm_filter_eligibility, compute_routing_evaluate_eligibility,
                extract_connectors_for_eligibility,
            },
            pm_filter_graph,
            types::{KeyConfig, KeyDataType, KeysConfig, TomlConfig},
        },
    };

    fn enum_value(value: &str) -> Option<ValueType> {
        Some(ValueType::EnumVariant(value.to_string()))
    }

    fn connectors(names: &[&str]) -> Vec<ConnectorInfo> {
        names
            .iter()
            .map(|name| ConnectorInfo {
                gateway_name: (*name).to_string(),
                gateway_id: None,
            })
            .collect()
    }

    fn routing_config_for_tests() -> TomlConfig {
        let mut keys = HashMap::new();
        keys.insert(
            "billing_country".to_string(),
            KeyConfig {
                data_type: KeyDataType::Enum,
                values: Some("US,IN".to_string()),
                min_value: None,
                max_value: None,
                min_length: None,
                max_length: None,
                exact_length: None,
                regex: None,
            },
        );
        TomlConfig {
            keys: KeysConfig { keys },
        }
    }

    fn build_bundle_for_tests() -> pm_filter_graph::PmFilterGraphBundle {
        let mut connector_configs = HashMap::new();
        connector_configs.insert(
            "stripe".to_string(),
            PaymentMethodFilters(
                [(
                    "credit".to_string(),
                    CurrencyCountryFlowFilter {
                        country: Some(HashSet::from(["US".to_string()])),
                        currency: None,
                        not_available_flows: None,
                    },
                )]
                .into_iter()
                .collect(),
            ),
        );
        connector_configs.insert(
            "default".to_string(),
            PaymentMethodFilters(
                [(
                    "debit".to_string(),
                    CurrencyCountryFlowFilter {
                        country: None,
                        currency: None,
                        not_available_flows: None,
                    },
                )]
                .into_iter()
                .collect(),
            ),
        );

        pm_filter_graph::build_pm_filter_graph_bundle(
            &ConnectorFilters(connector_configs),
            Some(&routing_config_for_tests()),
        )
        .expect("pm filter bundle should build")
    }

    fn build_priority_matrix_bundle() -> pm_filter_graph::PmFilterGraphBundle {
        let mut connector_configs = HashMap::new();
        connector_configs.insert(
            "razorpay".to_string(),
            PaymentMethodFilters(
                [(
                    "upi_collect".to_string(),
                    CurrencyCountryFlowFilter {
                        country: Some(HashSet::from(["IN".to_string()])),
                        currency: Some(HashSet::from(["INR".to_string()])),
                        not_available_flows: None,
                    },
                )]
                .into_iter()
                .collect(),
            ),
        );
        connector_configs.insert(
            "stripe".to_string(),
            PaymentMethodFilters(
                [(
                    "credit".to_string(),
                    CurrencyCountryFlowFilter {
                        country: Some(HashSet::from(["US".to_string()])),
                        currency: Some(HashSet::from(["USD".to_string()])),
                        not_available_flows: None,
                    },
                )]
                .into_iter()
                .collect(),
            ),
        );
        connector_configs.insert(
            "default".to_string(),
            PaymentMethodFilters(
                [(
                    "google_pay".to_string(),
                    CurrencyCountryFlowFilter {
                        country: Some(HashSet::from(["US".to_string()])),
                        currency: None,
                        not_available_flows: None,
                    },
                )]
                .into_iter()
                .collect(),
            ),
        );

        pm_filter_graph::build_pm_filter_graph_bundle(
            &ConnectorFilters(connector_configs),
            Some(&routing_config_for_tests()),
        )
        .expect("priority matrix bundle should build")
    }

    fn params_for_case(
        payment_method_type: Option<&str>,
        billing_country: Option<&str>,
        currency: Option<&str>,
    ) -> HashMap<String, Option<ValueType>> {
        let mut params = HashMap::new();
        if let Some(payment_method_type) = payment_method_type {
            params.insert(
                "payment_method_type".to_string(),
                enum_value(payment_method_type),
            );
        }
        if let Some(billing_country) = billing_country {
            params.insert("billing_country".to_string(), enum_value(billing_country));
        }
        if let Some(currency) = currency {
            params.insert("currency".to_string(), enum_value(currency));
        }
        params
    }

    fn connector_names(connectors: &[ConnectorInfo]) -> Vec<String> {
        connectors
            .iter()
            .map(|connector| connector.gateway_name.clone())
            .collect()
    }

    fn print_priority_test_setup(bundle: &pm_filter_graph::PmFilterGraphBundle) {
        println!("=== TEST SETUP: ROUTING + PM ELIGIBILITY ===");
        println!("Routing rule under test:");
        println!("  if payment_method == card => priority [razorpay, stripe]");
        println!("  else => default [adyen]");
        println!("Eligibility input:");
        println!("  all connectors from routing output (single/priority/volume variants)");
        println!("pm_filters under test:");
        println!("  razorpay.upi_collect => country=[IN], currency=[INR]");
        println!("  stripe.credit        => country=[US], currency=[USD]");
        println!("  default.google_pay   => country=[US]");
        println!("Resolution order:");
        println!("  explicit connector PMT rule -> default PMT rule -> pass-open");
        println!(
            "Derived billing_country -> ISO2 map: {:?}",
            bundle.billing_country_to_iso2
        );
        println!(
            "explicit connector PMT map: {:?}",
            bundle.explicit_connector_payment_method_types
        );
        println!("default PMT set: {:?}", bundle.default_payment_method_types);
        println!("=== END SETUP ===");
    }

    fn pm_filter_test_connectors(names: &[&str]) -> Vec<ConnectorInfo> {
        names
            .iter()
            .map(|name| ConnectorInfo {
                gateway_name: (*name).to_string(),
                gateway_id: None,
            })
            .collect()
    }

    fn pm_filter_test_routing_config() -> TomlConfig {
        let mut keys = HashMap::new();
        keys.insert(
            "billing_country".to_string(),
            KeyConfig {
                data_type: KeyDataType::Enum,
                values: Some("US,DE".to_string()),
                min_value: None,
                max_value: None,
                min_length: None,
                max_length: None,
                exact_length: None,
                regex: None,
            },
        );
        TomlConfig {
            keys: KeysConfig { keys },
        }
    }

    fn pm_filter_make_filter(
        country: Option<&[&str]>,
        currency: Option<&[&str]>,
        capture_method: Option<&str>,
    ) -> CurrencyCountryFlowFilter {
        CurrencyCountryFlowFilter {
            country: country.map(|values| values.iter().map(|v| (*v).to_string()).collect()),
            currency: currency.map(|values| values.iter().map(|v| (*v).to_string()).collect()),
            not_available_flows: capture_method.map(|capture_method| NotAvailableFlows {
                capture_method: Some(capture_method.to_string()),
            }),
        }
    }

    fn pm_filter_build_connector_filters() -> ConnectorFilters {
        let mut connector_filters = HashMap::new();

        connector_filters.insert(
            "stripe".to_string(),
            PaymentMethodFilters(
                [(
                    "credit".to_string(),
                    pm_filter_make_filter(Some(&["US"]), Some(&["USD"]), None),
                )]
                .into_iter()
                .collect(),
            ),
        );

        connector_filters.insert(
            "default".to_string(),
            PaymentMethodFilters(
                [
                    (
                        "credit".to_string(),
                        pm_filter_make_filter(Some(&["US"]), Some(&["USD"]), Some("manual")),
                    ),
                    (
                        "debit".to_string(),
                        pm_filter_make_filter(Some(&["US"]), Some(&["USD"]), None),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        );

        ConnectorFilters(connector_filters)
    }

    #[test]
    fn routing_evaluate_eligibility_applies_pm_filters_on_routing_output() {
        let bundle = build_bundle_for_tests();
        let params = HashMap::from([
            ("payment_method_type".to_string(), enum_value("credit")),
            ("billing_country".to_string(), enum_value("IN")),
        ]);
        let initial = connectors(&["stripe", "adyen"]);

        let filtered = compute_routing_evaluate_eligibility(Some(&bundle), &params, &initial);
        assert_eq!(filtered, connectors(&["adyen"]));
    }

    #[test]
    fn routing_evaluate_eligibility_skips_pm_filters_when_payment_method_type_missing() {
        let bundle = build_bundle_for_tests();
        let params = HashMap::from([(
            "billing_country".to_string(),
            enum_value("US"),
        )]);
        let initial = connectors(&["stripe", "adyen"]);

        let filtered = compute_routing_evaluate_eligibility(Some(&bundle), &params, &initial);
        assert_eq!(filtered, initial);
    }

    #[test]
    fn routing_evaluate_eligibility_fails_open_for_pm_filters_after_routing_pass() {
        let params = HashMap::from([("payment_method_type".to_string(), enum_value("credit"))]);
        let initial = connectors(&["stripe", "adyen", "checkout"]);

        let filtered = compute_routing_evaluate_eligibility(None, &params, &initial);
        assert_eq!(filtered, connectors(&["stripe", "adyen", "checkout"]));
    }

    #[test]
    fn apply_pm_filter_eligibility_returns_intersection() {
        let bundle = build_bundle_for_tests();
        let params = HashMap::from([
            ("payment_method_type".to_string(), enum_value("credit")),
            ("billing_country".to_string(), enum_value("IN")),
        ]);
        let initial = connectors(&["stripe", "adyen"]);

        let filtered = apply_pm_filter_eligibility(Some(&bundle), &params, &initial);
        assert_eq!(filtered, connectors(&["adyen"]));
    }

    #[test]
    fn apply_pm_filter_eligibility_skips_when_payment_method_type_missing() {
        let bundle = build_bundle_for_tests();
        let params = HashMap::from([(
            "billing_country".to_string(),
            enum_value("US"),
        )]);
        let initial = connectors(&["stripe", "adyen"]);

        let filtered = if pm_filter_graph::has_payment_method_type(&params) {
            apply_pm_filter_eligibility(Some(&bundle), &params, &initial)
        } else {
            initial.clone()
        };
        assert_eq!(filtered, initial);
    }

    #[test]
    fn apply_pm_filter_eligibility_fails_open_when_bundle_unavailable() {
        let params = HashMap::from([
            ("payment_method_type".to_string(), enum_value("credit")),
            (
                "billing_country".to_string(),
                enum_value("US"),
            ),
        ]);
        let initial = connectors(&["stripe", "adyen"]);

        let filtered = apply_pm_filter_eligibility(None, &params, &initial);
        assert_eq!(filtered, initial);
    }

    #[test]
    fn pm_filter_explicit_connector_pass_and_fail_for_country_currency_pmt() {
        let bundle = pm_filter_graph::build_pm_filter_graph_bundle(
            &pm_filter_build_connector_filters(),
            Some(&pm_filter_test_routing_config()),
        )
        .expect("bundle should build");

        let params_pass = HashMap::from([
            ("payment_method_type".to_string(), enum_value("credit")),
            (
                "billing_country".to_string(),
                enum_value("US"),
            ),
            ("currency".to_string(), enum_value("USD")),
        ]);
        let params_fail_country = HashMap::from([
            ("payment_method_type".to_string(), enum_value("credit")),
            ("billing_country".to_string(), enum_value("DE")),
            ("currency".to_string(), enum_value("USD")),
        ]);

        assert_eq!(
            pm_filter_graph::filter_eligible_connectors(
                &bundle,
                &params_pass,
                &pm_filter_test_connectors(&["stripe"])
            )
            .len(),
            1
        );
        assert!(pm_filter_graph::filter_eligible_connectors(
            &bundle,
            &params_fail_country,
            &pm_filter_test_connectors(&["stripe"])
        )
        .is_empty());
    }

    #[test]
    fn pm_filter_default_rule_applies_for_non_explicit_connector() {
        let bundle = pm_filter_graph::build_pm_filter_graph_bundle(
            &pm_filter_build_connector_filters(),
            Some(&pm_filter_test_routing_config()),
        )
        .expect("bundle should build");

        let params = HashMap::from([
            ("payment_method_type".to_string(), enum_value("credit")),
            (
                "billing_country".to_string(),
                enum_value("US"),
            ),
            ("currency".to_string(), enum_value("USD")),
            ("capture_method".to_string(), enum_value("automatic")),
        ]);

        assert_eq!(
            pm_filter_graph::filter_eligible_connectors(
                &bundle,
                &params,
                &pm_filter_test_connectors(&["adyen"])
            )
            .len(),
            1
        );
    }

    #[test]
    fn pm_filter_explicit_connector_without_specific_rule_falls_back_to_default() {
        let bundle = pm_filter_graph::build_pm_filter_graph_bundle(
            &pm_filter_build_connector_filters(),
            Some(&pm_filter_test_routing_config()),
        )
        .expect("bundle should build");

        let params = HashMap::from([
            ("payment_method_type".to_string(), enum_value("debit")),
            (
                "billing_country".to_string(),
                enum_value("US"),
            ),
            ("currency".to_string(), enum_value("USD")),
        ]);

        assert_eq!(
            pm_filter_graph::filter_eligible_connectors(
                &bundle,
                &params,
                &pm_filter_test_connectors(&["stripe"])
            )
            .len(),
            1
        );
    }

    #[test]
    fn pm_filter_connector_passes_when_rule_absent_in_explicit_and_default() {
        let bundle = pm_filter_graph::build_pm_filter_graph_bundle(
            &pm_filter_build_connector_filters(),
            Some(&pm_filter_test_routing_config()),
        )
        .expect("bundle should build");

        let params =
            HashMap::from([("payment_method_type".to_string(), enum_value("upi_collect"))]);

        assert_eq!(
            pm_filter_graph::filter_eligible_connectors(
                &bundle,
                &params,
                &pm_filter_test_connectors(&["stripe"])
            )
            .len(),
            1
        );
    }

    #[test]
    fn pm_filter_capture_method_exclusion_works() {
        let bundle = pm_filter_graph::build_pm_filter_graph_bundle(
            &pm_filter_build_connector_filters(),
            Some(&pm_filter_test_routing_config()),
        )
        .expect("bundle should build");

        let params_manual = HashMap::from([
            ("payment_method_type".to_string(), enum_value("credit")),
            (
                "billing_country".to_string(),
                enum_value("US"),
            ),
            ("currency".to_string(), enum_value("USD")),
            ("capture_method".to_string(), enum_value("manual")),
        ]);
        let params_auto = HashMap::from([
            ("payment_method_type".to_string(), enum_value("credit")),
            (
                "billing_country".to_string(),
                enum_value("US"),
            ),
            ("currency".to_string(), enum_value("USD")),
            ("capture_method".to_string(), enum_value("automatic")),
        ]);

        assert!(pm_filter_graph::filter_eligible_connectors(
            &bundle,
            &params_manual,
            &pm_filter_test_connectors(&["adyen"])
        )
        .is_empty());
        assert_eq!(
            pm_filter_graph::filter_eligible_connectors(
                &bundle,
                &params_auto,
                &pm_filter_test_connectors(&["adyen"])
            )
            .len(),
            1
        );
    }

    #[test]
    fn pm_filter_missing_country_or_currency_does_not_fail_when_configured() {
        let bundle = pm_filter_graph::build_pm_filter_graph_bundle(
            &pm_filter_build_connector_filters(),
            Some(&pm_filter_test_routing_config()),
        )
        .expect("bundle should build");

        let params = HashMap::from([("payment_method_type".to_string(), enum_value("credit"))]);
        assert_eq!(
            pm_filter_graph::filter_eligible_connectors(
                &bundle,
                &params,
                &pm_filter_test_connectors(&["adyen"])
            )
            .len(),
            1
        );
    }

    #[test]
    fn pm_filter_billing_country_mapping_is_derived_from_routing_keys() {
        let bundle = pm_filter_graph::build_pm_filter_graph_bundle(
            &pm_filter_build_connector_filters(),
            Some(&pm_filter_test_routing_config()),
        )
        .expect("bundle should build");
        assert_eq!(
            bundle.billing_country_to_iso2.get("US"),
            Some(&"US".to_string())
        );
        assert_eq!(
            bundle.billing_country_to_iso2.get("DE"),
            Some(&"DE".to_string())
        );
    }

    #[test]
    fn pm_filter_unknown_billing_country_is_ignored_and_does_not_false_reject() {
        let bundle = pm_filter_graph::build_pm_filter_graph_bundle(
            &pm_filter_build_connector_filters(),
            Some(&pm_filter_test_routing_config()),
        )
        .expect("bundle should build");

        let params = HashMap::from([
            ("payment_method_type".to_string(), enum_value("credit")),
            ("billing_country".to_string(), enum_value("UnknownLand")),
            ("currency".to_string(), enum_value("USD")),
            ("capture_method".to_string(), enum_value("automatic")),
        ]);

        assert_eq!(
            pm_filter_graph::filter_eligible_connectors(
                &bundle,
                &params,
                &pm_filter_test_connectors(&["adyen"])
            )
            .len(),
            1
        );
    }

    #[test]
    fn extract_connectors_for_priority_output_returns_all_unique_connectors() {
        let output = Output::Priority(vec![
            ConnectorInfo {
                gateway_name: "razorpay".to_string(),
                gateway_id: Some("mca_rzp".to_string()),
            },
            ConnectorInfo {
                gateway_name: "stripe".to_string(),
                gateway_id: Some("mca_strp".to_string()),
            },
            ConnectorInfo {
                gateway_name: "razorpay".to_string(),
                gateway_id: Some("mca_rzp".to_string()),
            },
        ]);

        let connectors = extract_connectors_for_eligibility(&output);
        assert_eq!(
            connectors,
            vec![
                ConnectorInfo {
                    gateway_name: "razorpay".to_string(),
                    gateway_id: Some("mca_rzp".to_string()),
                },
                ConnectorInfo {
                    gateway_name: "stripe".to_string(),
                    gateway_id: Some("mca_strp".to_string()),
                },
            ]
        );
    }

    #[test]
    fn extract_connectors_for_volume_split_output_returns_all_split_connectors() {
        let output = Output::VolumeSplit(vec![
            VolumeSplit {
                split: 70,
                output: ConnectorInfo {
                    gateway_name: "razorpay".to_string(),
                    gateway_id: Some("mca_rzp".to_string()),
                },
            },
            VolumeSplit {
                split: 30,
                output: ConnectorInfo {
                    gateway_name: "stripe".to_string(),
                    gateway_id: Some("mca_strp".to_string()),
                },
            },
        ]);

        let connectors = extract_connectors_for_eligibility(&output);
        assert_eq!(
            connectors,
            vec![
                ConnectorInfo {
                    gateway_name: "razorpay".to_string(),
                    gateway_id: Some("mca_rzp".to_string()),
                },
                ConnectorInfo {
                    gateway_name: "stripe".to_string(),
                    gateway_id: Some("mca_strp".to_string()),
                },
            ]
        );
    }

    #[test]
    fn extract_connectors_for_volume_split_priority_flattens_all_unique_connectors() {
        let output = Output::VolumeSplitPriority(vec![
            VolumeSplit {
                split: 50,
                output: vec![
                    ConnectorInfo {
                        gateway_name: "razorpay".to_string(),
                        gateway_id: Some("mca_rzp".to_string()),
                    },
                    ConnectorInfo {
                        gateway_name: "stripe".to_string(),
                        gateway_id: Some("mca_strp".to_string()),
                    },
                ],
            },
            VolumeSplit {
                split: 50,
                output: vec![
                    ConnectorInfo {
                        gateway_name: "stripe".to_string(),
                        gateway_id: Some("mca_strp".to_string()),
                    },
                    ConnectorInfo {
                        gateway_name: "adyen".to_string(),
                        gateway_id: Some("mca_ady".to_string()),
                    },
                ],
            },
        ]);

        let connectors = extract_connectors_for_eligibility(&output);
        assert_eq!(
            connectors,
            vec![
                ConnectorInfo {
                    gateway_name: "razorpay".to_string(),
                    gateway_id: Some("mca_rzp".to_string()),
                },
                ConnectorInfo {
                    gateway_name: "stripe".to_string(),
                    gateway_id: Some("mca_strp".to_string()),
                },
                ConnectorInfo {
                    gateway_name: "adyen".to_string(),
                    gateway_id: Some("mca_ady".to_string()),
                },
            ]
        );
    }

    #[test]
    fn priority_and_fallback_permutation_matrix_with_logs() {
        let bundle = build_priority_matrix_bundle();
        print_priority_test_setup(&bundle);

        #[derive(Clone)]
        struct Case {
            name: &'static str,
            connectors: Vec<&'static str>,
            payment_method_type: Option<&'static str>,
            billing_country: Option<&'static str>,
            currency: Option<&'static str>,
            expected: Vec<&'static str>,
        }

        let cases = vec![
            Case {
                name: "priority_card_no_pmt_type_pm_skipped",
                connectors: vec!["razorpay", "stripe"],
                payment_method_type: None,
                billing_country: None,
                currency: None,
                expected: vec!["razorpay", "stripe"],
            },
            Case {
                name: "priority_card_upi_in_inr_both_pass",
                connectors: vec!["razorpay", "stripe"],
                payment_method_type: Some("upi_collect"),
                billing_country: Some("IN"),
                currency: Some("INR"),
                expected: vec!["razorpay", "stripe"],
            },
            Case {
                name: "priority_card_upi_us_usd_only_stripe_passes",
                connectors: vec!["razorpay", "stripe"],
                payment_method_type: Some("upi_collect"),
                billing_country: Some("US"),
                currency: Some("USD"),
                expected: vec!["stripe"],
            },
            Case {
                name: "priority_card_credit_us_usd_both_pass",
                connectors: vec!["razorpay", "stripe"],
                payment_method_type: Some("credit"),
                billing_country: Some("US"),
                currency: Some("USD"),
                expected: vec!["razorpay", "stripe"],
            },
            Case {
                name: "priority_card_google_pay_us_both_pass",
                connectors: vec!["razorpay", "stripe"],
                payment_method_type: Some("google_pay"),
                billing_country: Some("US"),
                currency: None,
                expected: vec!["razorpay", "stripe"],
            },
            Case {
                name: "priority_card_google_pay_in_both_fail",
                connectors: vec!["razorpay", "stripe"],
                payment_method_type: Some("google_pay"),
                billing_country: Some("IN"),
                currency: None,
                expected: vec![],
            },
            Case {
                name: "default_output_no_pmt_type_pm_skipped",
                connectors: vec!["adyen"],
                payment_method_type: None,
                billing_country: None,
                currency: None,
                expected: vec!["adyen"],
            },
            Case {
                name: "fallback_razorpay_stripe_no_pmt_type_pm_skipped",
                connectors: vec!["razorpay", "stripe"],
                payment_method_type: None,
                billing_country: None,
                currency: None,
                expected: vec!["razorpay", "stripe"],
            },
            Case {
                name: "fallback_razorpay_stripe_upi_in_inr_both_pass",
                connectors: vec!["razorpay", "stripe"],
                payment_method_type: Some("upi_collect"),
                billing_country: Some("IN"),
                currency: Some("INR"),
                expected: vec!["razorpay", "stripe"],
            },
            Case {
                name: "fallback_razorpay_stripe_upi_us_usd_only_stripe_passes",
                connectors: vec!["razorpay", "stripe"],
                payment_method_type: Some("upi_collect"),
                billing_country: Some("US"),
                currency: Some("USD"),
                expected: vec!["stripe"],
            },
            Case {
                name: "fallback_stripe_razorpay_upi_us_usd_only_stripe_passes",
                connectors: vec!["stripe", "razorpay"],
                payment_method_type: Some("upi_collect"),
                billing_country: Some("US"),
                currency: Some("USD"),
                expected: vec!["stripe"],
            },
            Case {
                name: "fallback_razorpay_stripe_credit_us_usd_both_pass",
                connectors: vec!["razorpay", "stripe"],
                payment_method_type: Some("credit"),
                billing_country: Some("US"),
                currency: Some("USD"),
                expected: vec!["razorpay", "stripe"],
            },
        ];

        for (index, case) in cases.iter().enumerate() {
            let input_connectors = connectors(&case.connectors);
            let params = params_for_case(
                case.payment_method_type,
                case.billing_country,
                case.currency,
            );
            let eligible =
                compute_routing_evaluate_eligibility(Some(&bundle), &params, &input_connectors);
            let expected = connectors(&case.expected);

            println!(
                "CASE {index}: {}\n  input={:?}\n  pmt={:?} country={:?} currency={:?}\n  eligible={:?}\n  expected={:?}",
                case.name,
                connector_names(&input_connectors),
                case.payment_method_type,
                case.billing_country,
                case.currency,
                connector_names(&eligible),
                case.expected
            );

            assert_eq!(eligible, expected, "failed case: {}", case.name);
        }
    }

    #[test]
    fn eligibility_runs_on_all_connectors_for_all_output_forms_with_logs() {
        let bundle = build_priority_matrix_bundle();
        print_priority_test_setup(&bundle);
        println!("Output forms tested: single, priority, volume_split, volume_split_priority");
        let params = params_for_case(
            Some("upi_collect"),
            Some("US"),
            Some("USD"),
        );

        let test_outputs = vec![
            (
                "single",
                Output::Single(ConnectorInfo {
                    gateway_name: "razorpay".to_string(),
                    gateway_id: Some("mca_rzp".to_string()),
                }),
                vec![],
            ),
            (
                "priority",
                Output::Priority(vec![
                    ConnectorInfo {
                        gateway_name: "razorpay".to_string(),
                        gateway_id: Some("mca_rzp".to_string()),
                    },
                    ConnectorInfo {
                        gateway_name: "stripe".to_string(),
                        gateway_id: Some("mca_strp".to_string()),
                    },
                ]),
                vec!["stripe"],
            ),
            (
                "volume_split",
                Output::VolumeSplit(vec![
                    VolumeSplit {
                        split: 60,
                        output: ConnectorInfo {
                            gateway_name: "razorpay".to_string(),
                            gateway_id: Some("mca_rzp".to_string()),
                        },
                    },
                    VolumeSplit {
                        split: 40,
                        output: ConnectorInfo {
                            gateway_name: "stripe".to_string(),
                            gateway_id: Some("mca_strp".to_string()),
                        },
                    },
                ]),
                vec!["stripe"],
            ),
            (
                "volume_split_priority",
                Output::VolumeSplitPriority(vec![
                    VolumeSplit {
                        split: 50,
                        output: vec![
                            ConnectorInfo {
                                gateway_name: "razorpay".to_string(),
                                gateway_id: Some("mca_rzp".to_string()),
                            },
                            ConnectorInfo {
                                gateway_name: "stripe".to_string(),
                                gateway_id: Some("mca_strp".to_string()),
                            },
                        ],
                    },
                    VolumeSplit {
                        split: 50,
                        output: vec![ConnectorInfo {
                            gateway_name: "adyen".to_string(),
                            gateway_id: Some("mca_ady".to_string()),
                        }],
                    },
                ]),
                vec!["stripe", "adyen"],
            ),
        ];

        for (name, output, expected_eligible_names) in test_outputs {
            let connectors_for_eligibility = extract_connectors_for_eligibility(&output);
            let eligible = compute_routing_evaluate_eligibility(
                Some(&bundle),
                &params,
                &connectors_for_eligibility,
            );

            println!(
                "FORM {name}\n  extracted={:?}\n  eligible={:?}\n  expected={:?}",
                connector_names(&connectors_for_eligibility),
                connector_names(&eligible),
                expected_eligible_names
            );

            assert_eq!(
                connector_names(&eligible),
                expected_eligible_names,
                "unexpected eligible connectors for output form {name}"
            );
        }
    }
}
