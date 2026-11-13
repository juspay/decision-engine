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

use std::cell::RefCell;
use std::io::Read;

use crate::cost_ingestion::mapping::ColumnMapping;
use crate::cost_ingestion::types::{IngestError, SettledFeeRow};

/// Index handed back for a required column that is absent. `require` does **not** fail on the spot
/// (see [`Headers`]), so a connector's resolved column state is built with this placeholder for any
/// missing column. Nothing ever reads through it: [`Headers::finish`] aborts before the row loop.
/// It is `usize::MAX` rather than 0 so that even a leaked one degrades to `""` via [`Row::get`]
/// instead of silently reading a real (wrong) column.
const MISSING: usize = usize::MAX;

/// Header-row view: resolve column indices by label, once, before the row loop. Order can drift
/// between report versions, so connectors index by name, never positionally.
///
/// [`require`](Self::require) **accumulates** missing columns instead of returning `Err` at the
/// first one. Connectors resolve their columns in a single `Ok(Cols { … })` literal chained with
/// `?`, so a short-circuiting `require` reported exactly one missing column per attempt — and since
/// a manual upload only parses in a background task after the whole (multi-GB) file has landed, the
/// merchant paid one full upload per missing column to discover them one at a time. Accumulating
/// turns that into a single report of everything wrong with the file.
///
/// The accumulated state is also what lets a connector's schema be *enumerated*: resolve against an
/// empty header row and every `require` misses, so `missing` comes back as the complete required
/// list. That keeps the preflight endpoint free of any duplicated copy of the column names.
pub struct Headers<'a> {
    record: &'a csv::ByteRecord,
    /// A merchant's `expected label -> their label` overrides, applied at lookup time.
    mapping: &'a ColumnMapping,
    /// Required labels that were absent, in resolution order.
    missing: RefCell<Vec<String>>,
    /// Every label asked for via `require`, present or not.
    required: RefCell<Vec<String>>,
    /// Every label asked for via `index`.
    optional: RefCell<Vec<String>>,
}

impl<'a> Headers<'a> {
    /// Wrap a header record. `parse` does this for connectors that use it; a connector driving its
    /// own row loop constructs one directly and is then responsible for calling
    /// [`finish`](Self::finish).
    pub fn new(record: &'a csv::ByteRecord, mapping: &'a ColumnMapping) -> Self {
        Self {
            record,
            mapping,
            missing: RefCell::new(Vec::new()),
            required: RefCell::new(Vec::new()),
            optional: RefCell::new(Vec::new()),
        }
    }

    /// Index of an optional column by exact label, or `None` if the report omits it.
    pub fn index(&self, name: &str) -> Option<usize> {
        self.optional.borrow_mut().push(name.to_string());
        self.position(name)
    }

    /// Index of a required column. Returns `Ok` even when the column is absent — the miss is
    /// recorded and surfaces from [`finish`](Self::finish) as a single aggregated
    /// [`IngestError::MissingColumns`] once *all* columns have been resolved. The `Result` is kept
    /// so connectors keep their `h.require("…")?` form and so a future hard failure stays possible.
    pub fn require(&self, name: &str) -> Result<usize, IngestError> {
        self.required.borrow_mut().push(name.to_string());
        match self.position(name) {
            Some(i) => Ok(i),
            None => {
                self.missing.borrow_mut().push(name.to_string());
                Ok(MISSING)
            }
        }
    }

    /// Locate a column, honouring the merchant's mapping. The *only* place a mapping takes effect:
    /// a connector always asks for its own expected label and the substitution happens here, so a
    /// mapping can never drift out of step with what the parser actually reads. `missing` /
    /// `required` still record the **expected** label — that is the vocabulary the connector, the
    /// error message, and the mapping UI all share.
    fn position(&self, name: &str) -> Option<usize> {
        let name = self.mapping.resolve(name).as_bytes();
        self.record.iter().position(|h| h == name)
    }

    /// The header labels the file actually carried.
    fn found(&self) -> Vec<String> {
        self.record
            .iter()
            .map(|h| String::from_utf8_lossy(h).into_owned())
            .collect()
    }

    /// Convert anything accumulated during resolution into one error. **Must** be called after a
    /// connector's `prepare` and before any data row is read — it is the single point that keeps a
    /// [`MISSING`]-poisoned column state from reaching the row loop. [`parse`] does this for every
    /// connector that goes through it; a connector resolving headers by hand must call it itself.
    pub fn finish(&self) -> Result<(), IngestError> {
        let missing = self.missing.borrow();
        if missing.is_empty() {
            return Ok(());
        }
        Err(IngestError::MissingColumns {
            missing: missing.clone(),
            required: self.required.borrow().clone(),
            optional: self.optional.borrow().clone(),
            found: self.found(),
        })
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
    mapping: &ColumnMapping,
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
    // Resolve columns, then convert any accumulated misses into one error *before* the row loop —
    // `prepare` returns `Ok` even with columns missing (see `Headers::require`), so this check is
    // what makes that safe.
    let headers = Headers::new(&header_rec, mapping);
    let cols = prepare(&headers)?;
    headers.finish()?;

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
