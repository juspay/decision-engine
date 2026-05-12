use once_cell::sync::OnceCell;

use crate::{
    config::ConfigGsmStore,
    lookup::lookup_from_error_info,
    source::{GsmConfig, GsmSourceKind},
    types::{GsmErrorInfo, GsmInfo, GsmOptionRow},
};

static GSM_STORE: OnceCell<ConfigGsmStore> = OnceCell::new();

/// Initialise the GSM store from the source configured in `[gsm]`.
/// Must be called once at startup before any request is served.
/// Panics if the configured source is unreachable or malformed.
pub async fn init(config: &GsmConfig) {
    if GSM_STORE.get().is_some() {
        return;
    }
    let store = match config.source {
        GsmSourceKind::Bundled => load_bundled(),
        GsmSourceKind::File => load_file(config),
        GsmSourceKind::S3 => load_s3(config).await,
    };
    GSM_STORE.set(store).ok();
}

pub fn get_store() -> &'static ConfigGsmStore {
    GSM_STORE.get_or_init(load_bundled)
}

fn load_bundled() -> ConfigGsmStore {
    let bundled = include_str!("../data/gsm.csv");
    let store = ConfigGsmStore::from_csv_str(bundled).expect("bundled GSM CSV must be valid");
    tracing::info!(tag = "GSM", count = store.len(), "Loaded GSM rules from bundled data");
    store
}

fn load_file(config: &GsmConfig) -> ConfigGsmStore {
    let path = config
        .path
        .as_deref()
        .expect("[gsm] source = \"file\" requires a `path` field");
    match ConfigGsmStore::from_csv_file(path) {
        Ok(store) => {
            tracing::info!(tag = "GSM", count = store.len(), path, "Loaded GSM rules from file");
            store
        }
        Err(e) => panic!("Failed to load GSM from file {path}: {e}"),
    }
}

#[cfg(feature = "s3")]
async fn load_s3(config: &GsmConfig) -> ConfigGsmStore {
    let bucket = config
        .bucket
        .as_deref()
        .expect("[gsm] source = \"s3\" requires a `bucket` field");
    let key = config
        .key
        .as_deref()
        .expect("[gsm] source = \"s3\" requires a `key` field");

    let aws_config = {
        let loader = aws_config::defaults(aws_config::BehaviorVersion::latest());
        let loader = if let Some(region) = &config.region {
            loader.region(aws_config::meta::region::RegionProviderChain::first_try(
                aws_sdk_s3::config::Region::new(region.clone()),
            ))
        } else {
            loader
        };
        loader.load().await
    };

    let client = aws_sdk_s3::Client::new(&aws_config);
    let output = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .unwrap_or_else(|e| panic!("Failed to fetch GSM from s3://{bucket}/{key}: {e}"));

    let bytes = output
        .body
        .collect()
        .await
        .unwrap_or_else(|e| panic!("Failed to read S3 response body for GSM: {e}"))
        .into_bytes();

    let content = std::str::from_utf8(&bytes)
        .unwrap_or_else(|e| panic!("GSM CSV from S3 is not valid UTF-8: {e}"));

    match ConfigGsmStore::from_csv_str(content) {
        Ok(store) => {
            tracing::info!(
                tag = "GSM",
                count = store.len(),
                bucket,
                key,
                "Loaded GSM rules from S3"
            );
            store
        }
        Err(e) => panic!("Failed to parse GSM CSV from s3://{bucket}/{key}: {e}"),
    }
}

#[cfg(not(feature = "s3"))]
async fn load_s3(_config: &GsmConfig) -> ConfigGsmStore {
    panic!("[gsm] source = \"s3\" requires enabling the `s3` feature on the gsm crate");
}

pub fn options() -> Vec<GsmOptionRow> {
    get_store()
        .rules()
        .map(|r| GsmOptionRow {
            connector: r.connector.clone(),
            flow: r.flow.clone(),
            sub_flow: r.sub_flow.clone(),
            error_code: r.code.clone(),
            error_message: r.message.clone(),
            error_category: r.error_category.clone(),
            decision: r.decision.to_string(),
        })
        .collect()
}

pub fn lookup(error_info: &GsmErrorInfo) -> Option<GsmInfo> {
    lookup_from_error_info(get_store(), error_info)
}
