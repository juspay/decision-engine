use once_cell::sync::OnceCell;

pub use gsm::{GsmErrorInfo, GsmInfo, GsmOptionRow};

use crate::config::{GsmConfig, GsmSourceType};

static GSM_STORE: OnceCell<gsm::ConfigGsmStore> = OnceCell::new();

/// Initialise the GSM store from the source configured in `[gsm]`.
/// Must be called once at startup before any request is served.
/// Panics if the configured source is unreachable or malformed.
pub async fn init(config: &GsmConfig) {
    if GSM_STORE.get().is_some() {
        return;
    }

    let store = match &config.source {
        GsmSourceType::Bundled => load_bundled(),
        GsmSourceType::File => load_file(config),
        GsmSourceType::S3 => load_s3(config).await,
    };

    GSM_STORE.set(store).ok();
}

fn load_bundled() -> gsm::ConfigGsmStore {
    let bundled = include_str!("../crates/gsm/data/gsm.csv");
    let store = gsm::ConfigGsmStore::from_csv_str(bundled).expect("bundled GSM CSV must be valid");
    crate::logger::info!(
        tag = "GSM",
        "Loaded {} GSM rules from bundled data",
        store.len()
    );
    store
}

fn load_file(config: &GsmConfig) -> gsm::ConfigGsmStore {
    let path = config
        .path
        .as_deref()
        .expect("[gsm] source = \"file\" requires a `path` field");

    match gsm::ConfigGsmStore::from_csv_file(path) {
        Ok(store) => {
            crate::logger::info!(
                tag = "GSM",
                "Loaded {} GSM rules from {}",
                store.len(),
                path
            );
            store
        }
        Err(e) => panic!("Failed to load GSM from file {path}: {e}"),
    }
}

async fn load_s3(config: &GsmConfig) -> gsm::ConfigGsmStore {
    let bucket = config
        .bucket
        .as_deref()
        .expect("[gsm] source = \"s3\" requires a `bucket` field");
    let key = config
        .key
        .as_deref()
        .expect("[gsm] source = \"s3\" requires a `key` field");

    let aws_config = {
        let loader = aws_config::from_env();
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

    match gsm::ConfigGsmStore::from_csv_str(content) {
        Ok(store) => {
            crate::logger::info!(
                tag = "GSM",
                "Loaded {} GSM rules from s3://{}/{}",
                store.len(),
                bucket,
                key
            );
            store
        }
        Err(e) => panic!("Failed to parse GSM CSV from s3://{bucket}/{key}: {e}"),
    }
}

fn get_store() -> &'static gsm::ConfigGsmStore {
    GSM_STORE.get().unwrap_or_else(|| {
        // Fallback for tests or binaries that skip GlobalAppState::new()
        GSM_STORE.get_or_init(load_bundled)
    })
}

/// Returns all rules currently loaded in the GSM store as a flat list.
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

/// Look up GSM info for a previous payment failure.
/// Returns `None` when no matching rule exists in the store.
pub fn lookup(error_info: &GsmErrorInfo) -> Option<GsmInfo> {
    gsm::lookup_from_error_info(get_store(), error_info)
}
