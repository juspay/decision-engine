//! Merchant-supplied column mappings: "the column your parser calls `Payable (SC)` is called
//! `Net Settlement Amount` in my file".
//!
//! A mapping is `expected label -> merchant's label`, scoped to one `(merchant, connector,
//! account)`. It exists because report labels drift — between connector report versions, regional
//! variants, and the merchant's own export pipeline — and re-cutting the file is often not something
//! the merchant can do. Rather than rewrite the header, a mapping changes *what
//! [`Headers`](super::connectors::csv_reader::Headers) searches for*, so a connector's `require`
//! calls are untouched and the mapping cannot desynchronise from the parser.
//!
//! **Mappings are load-bearing on correctness, not just convenience.** The fee columns are a
//! decomposition (`interchange` + `scheme` + `markup` + `commission` sum into `total_fee`, which the
//! fit regresses against `gross` to produce the `pct_bps`/`fixed` the router serves), so a
//! plausible-but-wrong mapping — pointing an all-in fee column at `Commission (SC)`, say — does not
//! fail loudly. It converges, grades `GOOD`, and serves a wrong cost model. A missing column is a
//! safe failure; a bad mapping is not. Two things guard against that, and both matter:
//!
//! - Nothing can be mapped that the connector did not ask for ([`ColumnMapping::validate`]), and
//!   nothing can be mapped to a column the file does not have.
//! - The dashboard must show the merchant a [`preview`](super::preflight::preview) of rows their
//!   mapping actually produces — the derived `gross` and `total_fee`, not just the column pairing —
//!   before it is saved. A mapping that type-checks can still be semantically wrong, and the
//!   derived values are where that becomes visible.
//!
//! Persisted in the generic `service_configuration` KV store, like connector credentials — no new
//! table. Mappings hold no secrets, so unlike credentials they are stored as plain JSON.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::types::service_configuration;

use super::types::IngestError;

/// `expected label -> the label this merchant's file actually uses`.
///
/// Empty is the common case and costs nothing: [`resolve`](Self::resolve) returns the expected label
/// unchanged, so an unmapped ingestion behaves exactly as it did before mappings existed.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ColumnMapping {
    #[serde(default)]
    columns: HashMap<String, String>,
}

/// Shared empty mapping, so the overwhelmingly common "no mapping" path allocates nothing and
/// callers can pass `ColumnMapping::none()` without owning one.
static EMPTY: std::sync::OnceLock<ColumnMapping> = std::sync::OnceLock::new();

impl ColumnMapping {
    /// The empty mapping: every column resolves to its own expected label.
    pub fn none() -> &'static Self {
        EMPTY.get_or_init(Self::default)
    }

    pub fn from_pairs(columns: HashMap<String, String>) -> Self {
        Self { columns }
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    pub fn columns(&self) -> &HashMap<String, String> {
        &self.columns
    }

    /// The header label to search for when a connector asks for `expected`. Unmapped columns
    /// resolve to themselves, so this is the only lookup the CSV layer needs.
    pub fn resolve<'a>(&'a self, expected: &'a str) -> &'a str {
        self.columns
            .get(expected)
            .map(String::as_str)
            .unwrap_or(expected)
    }

    /// Reject a mapping that cannot be meaningful before it is ever stored:
    ///
    /// - a key the connector never asks for (typo, or a stale mapping kept across a parser change)
    ///   would sit inert and silently stop taking effect;
    /// - a target column absent from `found` would resolve to nothing, reintroducing the missing
    ///   column the mapping was written to fix;
    /// - mapping two expected columns onto one source column is nearly always a mistake, and an
    ///   expensive one here: it would feed the same values into two different fee components and
    ///   double-count them into `total_fee`.
    ///
    /// `known` is the connector's own required + optional labels, and `found` the header labels of
    /// the merchant's file — both discovered from the connector itself, never hardcoded.
    pub fn validate(&self, known: &[String], found: &[String]) -> Result<(), IngestError> {
        let mut problems = Vec::new();
        let mut targets: HashMap<&str, &str> = HashMap::new();

        // Sort for a deterministic message — HashMap iteration order is arbitrary.
        let mut pairs: Vec<(&String, &String)> = self.columns.iter().collect();
        pairs.sort();

        for (expected, theirs) in pairs {
            if !known.iter().any(|k| k == expected) {
                problems.push(format!("'{expected}' is not a column this connector reads"));
            }
            if !found.iter().any(|f| f == theirs) {
                problems.push(format!(
                    "'{expected}' is mapped to '{theirs}', which is not a column in this file"
                ));
            }
            if let Some(other) = targets.insert(theirs.as_str(), expected.as_str()) {
                problems.push(format!(
                    "'{other}' and '{expected}' are both mapped to '{theirs}'"
                ));
            }
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(IngestError::Parse(format!(
                "invalid column mapping: {}",
                problems.join("; ")
            )))
        }
    }
}

/// KV key for a settlement source's column mapping. Scoped by merchant as well as
/// `(connector, account)` — unlike credentials, where the account is globally unique because the
/// webhook must resolve a merchant *from* it, a mapping is always looked up with the merchant
/// already known, and scoping it prevents one merchant's mapping applying to another's account.
fn config_name(merchant_id: &str, connector: &str, account: &str) -> String {
    format!("cost_ingest_colmap::{merchant_id}::{connector}::{account}")
}

/// Load a source's stored mapping, or the empty mapping when none is saved.
///
/// A stored value that no longer parses is treated as absent rather than failing the ingestion: a
/// mapping is an optimisation over a file that may well parse on its own, and a corrupt one must not
/// be able to block ingestion outright.
pub async fn load(
    merchant_id: &str,
    connector: &str,
    account: &str,
) -> Result<ColumnMapping, IngestError> {
    let stored =
        service_configuration::find_config_by_name(config_name(merchant_id, connector, account))
            .await
            .map_err(|e| IngestError::Storage(e.to_string()))?;
    Ok(stored
        .and_then(|c| c.value)
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default())
}

/// Persist a source's mapping. An empty mapping deletes the row rather than storing `{}`, so
/// "cleared" and "never set" are the same state.
pub async fn save(
    merchant_id: &str,
    connector: &str,
    account: &str,
    mapping: &ColumnMapping,
) -> Result<(), IngestError> {
    let name = config_name(merchant_id, connector, account);
    if mapping.is_empty() {
        return delete(merchant_id, connector, account).await;
    }
    let value = serde_json::to_string(mapping).map_err(|e| IngestError::Storage(e.to_string()))?;
    let exists = service_configuration::find_config_by_name(name.clone())
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?
        .is_some();
    if exists {
        service_configuration::update_config(name, Some(value)).await
    } else {
        service_configuration::insert_config(name, Some(value)).await
    }
    .map_err(|e| IngestError::Storage(e.to_string()))
}

/// Remove a source's mapping. Idempotent — clearing a mapping that was never set is not an error,
/// since "nothing stored" is the requested end state either way. Real storage failures are *not*
/// absorbed; see the match arms.
pub async fn delete(merchant_id: &str, connector: &str, account: &str) -> Result<(), IngestError> {
    match service_configuration::delete_config(config_name(merchant_id, connector, account)).await {
        Ok(()) => Ok(()),
        // The one absorbed case: nothing was stored, which is the state the caller asked for.
        Err(crate::generics::MeshError::NoRowstoDelete) => Ok(()),
        // Anything else — a dropped connection, a failed query — really did fail to clear the
        // mapping. Reporting success there would tell a merchant their mapping was removed while
        // the next ingestion silently kept applying it: precisely the quiet wrong answer this
        // module exists to prevent.
        Err(e) => Err(IngestError::Storage(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(pairs: &[(&str, &str)]) -> ColumnMapping {
        ColumnMapping::from_pairs(
            pairs
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
        )
    }

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_mapping_resolves_to_the_expected_label() {
        assert_eq!(
            ColumnMapping::none().resolve("Payable (SC)"),
            "Payable (SC)"
        );
        assert!(ColumnMapping::none().is_empty());
    }

    #[test]
    fn mapped_column_resolves_to_the_merchants_label() {
        let m = mapping(&[("Payable (SC)", "Net Settlement Amount")]);
        assert_eq!(m.resolve("Payable (SC)"), "Net Settlement Amount");
        assert_eq!(
            m.resolve("Record Type"),
            "Record Type",
            "unmapped passes through"
        );
    }

    #[test]
    fn accepts_a_well_formed_mapping() {
        let m = mapping(&[("Payable (SC)", "Net Amount")]);
        let known = strings(&["Payable (SC)", "Record Type"]);
        let found = strings(&["Net Amount", "Record Type"]);
        assert!(m.validate(&known, &found).is_ok());
    }

    #[test]
    fn rejects_a_column_the_connector_does_not_read() {
        let m = mapping(&[("Payble (SC)", "Net Amount")]); // typo'd key
        let err = m
            .validate(&strings(&["Payable (SC)"]), &strings(&["Net Amount"]))
            .unwrap_err();
        assert!(
            format!("{err}").contains("not a column this connector reads"),
            "got {err}"
        );
    }

    #[test]
    fn rejects_a_target_missing_from_the_file() {
        let m = mapping(&[("Payable (SC)", "Nonexistent")]);
        let err = m
            .validate(&strings(&["Payable (SC)"]), &strings(&["Net Amount"]))
            .unwrap_err();
        assert!(
            format!("{err}").contains("not a column in this file"),
            "got {err}"
        );
    }

    /// The expensive mistake: two fee components fed from one source column double-count into
    /// `total_fee` and silently bias the fitted price.
    #[test]
    fn rejects_two_expected_columns_sharing_one_source_column() {
        let m = mapping(&[("Commission (SC)", "Fee"), ("Markup (SC)", "Fee")]);
        let err = m
            .validate(
                &strings(&["Commission (SC)", "Markup (SC)"]),
                &strings(&["Fee"]),
            )
            .unwrap_err();
        assert!(
            format!("{err}").contains("both mapped to 'Fee'"),
            "got {err}"
        );
    }
}
