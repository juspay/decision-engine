//! Shared CSV driver for connector report parsers.
//!
//! Settlement reports are large and mostly *not* fee-bearing — an Adyen report, for instance, is
//! ~90% transaction-lifecycle rows (`Received`, `Authorised`, …) that every connector skips. This
//! driver makes those skips cheap: it reads each row into a single **reused [`csv::ByteRecord`]**
//! (no per-row allocation) and decodes a field to `&str` **only when the connector reads it**. So a
//! skipped row costs one byte-slice comparison on its type column instead of allocating and
//! UTF-8-validating all ~30 fields, as `StringRecord`-based iteration did.
//!
//! A connector supplies two closures: `prepare` resolves column indices from the header row once,
//! and `map_row` turns each data row into an optional [`SettledFeeRow`] (`None` = skip). The reader
//! creation, header handling, and the reuse loop live here once, so every connector benefits and no
//! connector re-implements them.

use std::io::Read;

use crate::cost_ingestion::types::{IngestError, SettledFeeRow};

/// Header-row view: resolve column indices by label, once, before the row loop. Order can drift
/// between report versions, so connectors index by name, never positionally.
pub struct Headers<'a> {
    record: &'a csv::ByteRecord,
}

impl Headers<'_> {
    /// Index of an optional column by exact label, or `None` if the report omits it.
    pub fn index(&self, name: &str) -> Option<usize> {
        let name = name.as_bytes();
        self.record.iter().position(|h| h == name)
    }

    /// Index of a required column, or a `Parse` error naming the missing column.
    pub fn require(&self, name: &str) -> Result<usize, IngestError> {
        self.index(name)
            .ok_or_else(|| IngestError::Parse(format!("missing column: {name}")))
    }
}

/// Data-row view backed by the reused [`csv::ByteRecord`]. Fields are decoded to `&str` lazily on
/// access; invalid UTF-8 in a field degrades to `""` rather than failing the whole report.
pub struct Row<'a> {
    record: &'a csv::ByteRecord,
}

impl Row<'_> {
    /// Field `i` as `&str` (`""` if out of range or not valid UTF-8).
    pub fn get(&self, i: usize) -> &str {
        self.record
            .get(i)
            .map(|b| std::str::from_utf8(b).unwrap_or(""))
            .unwrap_or("")
    }

    /// Field at an optional index (`""` when the column was absent). Convenience for optional columns.
    pub fn get_opt(&self, i: Option<usize>) -> &str {
        i.map(|i| self.get(i)).unwrap_or("")
    }
}

/// Drive a CSV report with a single reused `ByteRecord`. `prepare` maps the header row to the
/// connector's resolved column state; `map_row` maps each data row to an optional [`SettledFeeRow`]
/// (`None` skips it, cheaply). Each produced row is handed to `on_row`. Streaming: at most one row
/// is resident, so a multi-GB report stays flat in memory.
pub fn parse<C>(
    reader: Box<dyn Read + Send>,
    prepare: impl FnOnce(&Headers<'_>) -> Result<C, IngestError>,
    mut map_row: impl FnMut(&C, &Row<'_>) -> Result<Option<SettledFeeRow>, IngestError>,
    on_row: &mut dyn FnMut(SettledFeeRow) -> Result<(), IngestError>,
) -> Result<(), IngestError> {
    // `csv::Reader` wraps `reader` in its own buffered reader and pulls records lazily.
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_reader(reader);

    // Headers are read once; clone so the borrow doesn't pin the reader for the row loop.
    let header_rec = rdr
        .byte_headers()
        .map_err(|e| IngestError::Parse(e.to_string()))?
        .clone();
    let cols = prepare(&Headers {
        record: &header_rec,
    })?;

    // One record, reused for every row — the whole point of the ByteRecord path.
    let mut rec = csv::ByteRecord::new();
    while rdr
        .read_byte_record(&mut rec)
        .map_err(|e| IngestError::Parse(e.to_string()))?
    {
        if let Some(row) = map_row(&cols, &Row { record: &rec })? {
            on_row(row)?;
        }
    }
    Ok(())
}
