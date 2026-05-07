use once_cell::sync::Lazy;

pub use gsm::{GsmErrorInfo, GsmInfo, GsmOptionRow};

static GSM_STORE: Lazy<gsm::ConfigGsmStore> = Lazy::new(|| {
    let csv_path = "config/gsm.csv";
    if std::path::Path::new(csv_path).exists() {
        match gsm::ConfigGsmStore::from_csv_file(csv_path) {
            Ok(store) => {
                crate::logger::info!(
                    tag = "GSM",
                    "Loaded {} GSM rules from {}",
                    store.len(),
                    csv_path
                );
                return store;
            }
            Err(e) => {
                crate::logger::warn!(
                    tag = "GSM",
                    "Failed to load GSM from {}: {}. Falling back to bundled data.",
                    csv_path,
                    e
                );
            }
        }
    }

    let bundled = include_str!("../crates/gsm/data/gsm.csv");
    let store =
        gsm::ConfigGsmStore::from_csv_str(bundled).expect("bundled GSM CSV must be valid");
    crate::logger::info!(
        tag = "GSM",
        "Loaded {} GSM rules from bundled data",
        store.len()
    );
    store
});

/// Returns all rules currently loaded in the GSM store as a flat list.
pub fn options() -> Vec<GsmOptionRow> {
    GSM_STORE
        .rules()
        .map(|r| GsmOptionRow {
            connector: r.connector.clone(),
            flow: r.flow.clone(),
            sub_flow: r.sub_flow.clone(),
            error_code: r.code.clone(),
            error_message: r.message.clone(),
        })
        .collect()
}

/// Look up GSM info for a previous payment failure.
/// Returns `None` when no matching rule exists in the store.
pub fn lookup(error_info: &GsmErrorInfo) -> Option<GsmInfo> {
    gsm::lookup_from_error_info(&*GSM_STORE, error_info)
}
