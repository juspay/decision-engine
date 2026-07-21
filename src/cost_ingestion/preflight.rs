//! Header preflight for manual report upload: tell a merchant whether a file will parse *before*
//! they upload it, and if not, exactly what is wrong with it.
//!
//! Without this, the manual path fails as badly as it can: `upload_report` streams the whole body
//! to a temp file, returns `202`, and only then parses in a background task — so a bad header costs
//! a full (potentially multi-GB) upload, a poll cycle, and a `last_error` on the history row. This
//! runs the *same* connector code against just the file's first few KB, synchronously, in
//! milliseconds.
//!
//! **No column list is duplicated here.** Both halves of the check are the connector's own
//! `parse_rows`, driven over a tiny in-memory reader:
//!
//! - *What does this connector need?* — resolve it against an **empty** header row. Every
//!   `Headers::require` misses, so [`IngestError::MissingColumns`] comes back carrying the complete
//!   required and optional label lists.
//! - *Does this file satisfy it?* — resolve it against the file's real header bytes. The same error
//!   comes back carrying only what is actually absent.
//!
//! A connector's schema therefore cannot drift from what preflight reports: adding a `require` call
//! updates both automatically.
//!
//! Because the check is cheap, it also runs against *every* registered connector, not just the
//! selected one. A file whose headers fit a different connector far better is the signature of the
//! most common upload mistake — picking the wrong connector in the dropdown — which no amount of
//! column mapping would fix, and which mapping would actively paper over.

use std::io::Cursor;

use serde::Serialize;

use super::mapping::ColumnMapping;
use super::source::ConnectorRegistry;
use super::types::{IngestError, SettledFeeRow};

/// How much of the uploaded file the caller need send. The header row is the first line; a few KB
/// covers it comfortably (Chase also spends a handful of lines on its `BEGIN`/`EntityId=` envelope,
/// which its reader strips) while keeping the request small enough to feel instant.
pub const HEADER_SAMPLE_BYTES: usize = 64 * 1024;

/// A connector's verdict on a header row.
#[derive(Debug, Serialize)]
pub struct PreflightReport {
    /// The connector this verdict is for.
    pub connector: String,
    /// Whether the file's header satisfies every required column.
    pub ok: bool,
    /// Required labels absent from the file — empty when `ok`.
    pub missing: Vec<String>,
    /// Required labels the file *does* carry.
    pub matched: Vec<String>,
    /// Every label this connector requires.
    pub required: Vec<String>,
    /// Every label this connector uses when present but tolerates the absence of.
    pub optional: Vec<String>,
    /// Optional labels this file does *not* carry (after any mapping). These never fail an
    /// ingestion, which is exactly why they are reported: their absence is silent but not free.
    /// Adyen's `Unique Terminal ID` is what distinguishes in-person from online acceptance, and
    /// `Booking Date` is what dates the report's period — a renamed one degrades the fitted model
    /// with nothing anywhere saying so. Worth offering to map, not worth blocking on.
    pub optional_missing: Vec<String>,
    /// The header labels the file actually carried.
    pub found: Vec<String>,
    /// Better-fitting connectors, best first — populated only when the selected one rejected the
    /// file. A non-empty list here usually means the wrong connector was selected, not that the
    /// columns need mapping.
    pub suggested_connectors: Vec<Suggestion>,
}

/// Another connector that fully accepts a header row the selected connector rejected.
#[derive(Debug, Serialize)]
pub struct Suggestion {
    pub connector: String,
    /// How many required columns it matched. Since only complete matches are suggested (see
    /// [`suggest`]), this is also its full required-column count — it is reported so the dashboard
    /// can say *"matches all 12 of adyen's columns"* rather than just naming the connector.
    pub matched_required: usize,
}

/// Run `connector`'s own column resolution against `header_sample` (the leading bytes of the
/// merchant's file) and report the outcome. Errors only if `connector` is not registered; a file
/// that cannot satisfy the connector is a successful call returning `ok: false`.
pub fn check(
    registry: &ConnectorRegistry,
    connector: &str,
    header_sample: &[u8],
    mapping: &ColumnMapping,
) -> Result<PreflightReport, IngestError> {
    let source = registry.get(connector)?;
    let schema = resolve(&*source, header_sample, mapping);

    // The connector's full schema, from an empty-header probe. Always unmapped: this asks "what does
    // this connector read?", which a mapping does not change — and the answer is the vocabulary the
    // mapping UI offers as its right-hand column, so it must not shift as the merchant maps.
    let (required, optional) = match resolve(&*source, b"", ColumnMapping::none()) {
        Resolution::Missing {
            missing, optional, ..
        } => (missing, optional),
        _ => match &schema {
            Resolution::Missing {
                required, optional, ..
            } => (required.clone(), optional.clone()),
            _ => (Vec::new(), Vec::new()),
        },
    };

    let (ok, missing, found) = match schema {
        Resolution::Ok { found } => (true, Vec::new(), found),
        Resolution::Missing { missing, found, .. } => (false, missing, found),
        // A malformed sample (unreadable CSV) is not a missing-column problem; report it as such
        // rather than claiming every column is absent.
        Resolution::Unreadable(e) => return Err(e),
    };

    let matched: Vec<String> = required
        .iter()
        .filter(|c| !missing.contains(c))
        .cloned()
        .collect();

    // Which optional columns the file lacks. Derived here rather than plumbed out of `Headers`
    // because it is the same question `position` asks — does the file contain the label this
    // expected column resolves to — and answering it here keeps the successful-parse path (which
    // raises no error to carry the information) and the failing one consistent.
    let optional_missing: Vec<String> = optional
        .iter()
        .filter(|o| !found.iter().any(|f| f == mapping.resolve(o)))
        .cloned()
        .collect();

    // Only look for a better-fitting connector once this one has already rejected the file. Probed
    // unmapped: a mapping is written for one connector, so applying it while testing the others
    // would distort the comparison it exists to inform.
    let suggested_connectors = if ok {
        Vec::new()
    } else {
        suggest(registry, connector, header_sample)
    };

    Ok(PreflightReport {
        connector: connector.to_string(),
        ok,
        missing,
        matched,
        required,
        optional,
        optional_missing,
        found,
        suggested_connectors,
    })
}

/// Connectors *other than* `selected` that fit `header_sample` better than it did, best first. A
/// connector is only suggested if it matches every required column — a partial match is more likely
/// to be coincidental label overlap (`Card Brand`, `Settlement Currency`) than the right answer, and
/// a wrong suggestion here is worse than none.
fn suggest(registry: &ConnectorRegistry, selected: &str, header_sample: &[u8]) -> Vec<Suggestion> {
    let mut out: Vec<Suggestion> = registry
        .connectors()
        .into_iter()
        .filter(|c| *c != selected)
        .filter_map(|c| {
            let source = registry.get(c).ok()?;
            match resolve(&*source, header_sample, ColumnMapping::none()) {
                // An `Ok` resolution means nothing was missing, so the matched count is simply this
                // connector's required-column count — recovered from an empty-header probe.
                Resolution::Ok { .. } => Some(Suggestion {
                    connector: c.to_string(),
                    matched_required: match resolve(&*source, b"", ColumnMapping::none()) {
                        Resolution::Missing { missing, .. } => missing.len(),
                        _ => 0,
                    },
                }),
                _ => None,
            }
        })
        .collect();
    // Most columns matched first; `connectors()` is already sorted, so ties stay deterministic.
    out.sort_by(|a, b| b.matched_required.cmp(&a.matched_required));
    out
}

/// One parsed row, as a candidate mapping would produce it.
#[derive(Debug, Serialize)]
pub struct PreviewRow {
    pub card_network: String,
    pub variant: String,
    pub funding: String,
    pub currency: String,
    pub issuer_country: String,
    /// Transaction value the fee applied to — derived, not a raw column.
    pub gross: f64,
    /// `interchange + scheme_fee + markup + commission` — derived, not a raw column.
    pub total_fee: f64,
    /// `total_fee` as a percentage of `gross`, which is the quantity the fit actually models. The
    /// single most diagnostic number here: a mapping that pairs columns plausibly but wrongly shows
    /// up as an implausible effective rate long before it shows up anywhere else.
    pub effective_pct: f64,
    pub interchange: f64,
    pub scheme_fee: f64,
    pub markup: f64,
    pub commission: f64,
}

/// What a candidate mapping actually produces, for the merchant to eyeball before saving it.
#[derive(Debug, Serialize)]
pub struct PreviewReport {
    /// Rows the mapping produced from the sample. Empty means the mapping parsed but matched no
    /// fee-bearing rows — itself a strong signal that something is mapped wrong.
    pub rows: Vec<PreviewRow>,
    /// Median `effective_pct` across `rows`, or `None` when there are none. Compare against the
    /// connector's plausible range: card processing lands in single-digit percent, so a median of
    /// 0.01% or 60% means the mapping is wrong however sensible the column pairing looked.
    pub median_effective_pct: Option<f64>,
    /// Set when the numbers do not look like payment processing at all. Advisory, not blocking — the
    /// merchant may have an unusual but legitimate report, and this cannot tell the difference.
    pub warning: Option<String>,
}

/// Rows to parse for a preview. Enough to see a pattern rather than one unrepresentative row, few
/// enough to render as a table.
const PREVIEW_ROWS: usize = 10;

/// Effective-rate band outside which a mapping is almost certainly wrong. Deliberately very wide:
/// this is meant to catch a mapping that is off by orders of magnitude (an all-in fee mapped onto a
/// single fee component, a minor-units column mapped onto a major-units one), not to second-guess
/// unusual-but-real pricing.
const PLAUSIBLE_PCT: std::ops::RangeInclusive<f64> = 0.05..=25.0;

/// Parse the first rows of `sample` under a candidate `mapping` and return what they became.
///
/// This exists because [`ColumnMapping::validate`](super::mapping::ColumnMapping::validate) can only
/// check that a mapping is *well-formed* — every column known, every target present, no two columns
/// sharing a source. It cannot tell whether `Commission (SC)` was pointed at the merchant's
/// commission column or at their all-in fee column. Both validate; only one is right; the wrong one
/// yields a cost model that grades `GOOD` and silently misprices routing. Showing the merchant the
/// derived `gross`, `total_fee`, and effective rate is what makes that difference visible, so the
/// dashboard must render this before offering to save.
/// `truncated` says whether `sample` is the head of a larger file rather than a whole small one.
/// The caller must state it rather than have it inferred from `sample.len()`: the dashboard reads
/// its slice as text before sending, and decoding can shift the byte count off the cap by enough to
/// make a length comparison answer wrong — silently disabling the incomplete-group handling below on
/// exactly the connectors that need it.
pub fn preview(
    registry: &ConnectorRegistry,
    connector: &str,
    sample: &[u8],
    mapping: &ColumnMapping,
    truncated: bool,
) -> Result<PreviewReport, IngestError> {
    let source = registry.get(connector)?;

    // For a connector that assembles one row from several (`groups_rows`), the group straddling a
    // cut is incomplete — its capture line is present but some fee lines are not — so it surfaces as
    // real gross with missing or zero fee. Left alone, that lone artefact drags the median to ~0%
    // and fires the "your mapping is wrong" warning at a mapping that is perfectly correct, which is
    // worse than no guardrail: it teaches merchants to click past the one warning that matters.
    let drop_incomplete = truncated && source.groups_rows();

    let mut rows: Vec<SettledFeeRow> = Vec::new();
    let reader = Box::new(Cursor::new(sample.to_vec()));
    // Stop once we have enough: a sample is capped at a few KB, but there is no reason to parse
    // past the rows we will show. The sentinel error is swallowed below.
    let outcome = source.parse_rows(reader, mapping, &mut |row| {
        rows.push(row);
        if rows.len() >= PREVIEW_ROWS {
            return Err(IngestError::Parse(STOP.to_string()));
        }
        Ok(())
    });
    match outcome {
        Ok(()) => {}
        Err(IngestError::Parse(ref m)) if m == STOP => {}
        // A genuine failure (missing columns under this mapping) is the caller's answer.
        Err(e) => return Err(e),
    }

    // Drop groups the cut left half-assembled. A captured payment always carries a fee in a real
    // report, so a zero-fee row from a truncated grouping connector is the artefact, not data.
    if drop_incomplete {
        rows.retain(|r| r.total_fee.abs() > f64::EPSILON);
    }

    let preview_rows: Vec<PreviewRow> = rows
        .iter()
        .map(|r| PreviewRow {
            card_network: r.card_network.clone(),
            variant: r.variant.clone(),
            funding: r.funding.clone(),
            currency: r.currency.clone(),
            issuer_country: r.issuer_country.clone(),
            gross: r.gross,
            total_fee: r.total_fee,
            effective_pct: if r.gross.abs() > f64::EPSILON {
                r.total_fee / r.gross * 100.0
            } else {
                0.0
            },
            interchange: r.interchange,
            scheme_fee: r.scheme_fee,
            markup: r.markup,
            commission: r.commission,
        })
        .collect();

    let median = median_pct(&preview_rows);
    let warning = match median {
        // Having dropped every row as incomplete is a statement about the sample size, not the
        // mapping — say so plainly instead of blaming a mapping that may well be right.
        None if preview_rows.is_empty() && drop_incomplete => Some(
            "This sample was cut off before any complete transaction, so there is nothing to \
             preview yet. The columns still resolve correctly — upload the report to check the \
             fitted result."
                .to_string(),
        ),
        None if preview_rows.is_empty() => Some(
            "This mapping parsed the file but produced no fee-bearing rows. Check that the row-type \
             and amount columns are mapped to the right source columns."
                .to_string(),
        ),
        Some(p) if !PLAUSIBLE_PCT.contains(&p) => Some(format!(
            "These rows imply an effective fee of {p:.3}% of transaction value, which is outside the \
             range real card processing falls in. That usually means a fee column is mapped to the \
             wrong source column, or that amounts are in minor units (cents) while the report's \
             gross is in major units. Check the derived columns below before saving."
        )),
        _ => None,
    };

    Ok(PreviewReport {
        rows: preview_rows,
        median_effective_pct: median,
        warning,
    })
}

/// Sentinel used to stop parsing once `PREVIEW_ROWS` rows are collected. Not a real failure — a
/// connector's `on_row` callback has no other way to signal "enough".
const STOP: &str = "__preview_row_limit__";

/// Median effective rate across preview rows, ignoring rows with no gross (which carry no rate).
fn median_pct(rows: &[PreviewRow]) -> Option<f64> {
    let mut pcts: Vec<f64> = rows
        .iter()
        .filter(|r| r.gross.abs() > f64::EPSILON)
        .map(|r| r.effective_pct)
        .collect();
    if pcts.is_empty() {
        return None;
    }
    pcts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(pcts[pcts.len() / 2])
}

/// Outcome of running a connector's column resolution over a header sample.
enum Resolution {
    Ok {
        found: Vec<String>,
    },
    Missing {
        missing: Vec<String>,
        required: Vec<String>,
        optional: Vec<String>,
        found: Vec<String>,
    },
    Unreadable(IngestError),
}

/// Drive `source`'s real `parse_rows` over `header_sample` and classify what came back.
///
/// This works — rather than needing a separate "just resolve the headers" entry point on every
/// connector — because `parse_rows` resolves all of its columns *before* reading any data row, and
/// `Headers::finish` raises the aggregated [`IngestError::MissingColumns`] at exactly that boundary.
/// A truncated sample therefore never reaches row parsing: either the columns resolve (and the few
/// sample rows parse harmlessly into a discarded callback, a possibly-truncated last row included)
/// or resolution fails first with the report we want.
fn resolve(
    source: &dyn super::source::SettlementReportSource,
    header_sample: &[u8],
    mapping: &ColumnMapping,
) -> Resolution {
    let reader = Box::new(Cursor::new(header_sample.to_vec()));
    // Rows are irrelevant here; only the header-resolution outcome is.
    match source.parse_rows(reader, mapping, &mut |_row| Ok(())) {
        Ok(()) => Resolution::Ok {
            found: header_labels(source, header_sample),
        },
        Err(IngestError::MissingColumns {
            missing,
            required,
            optional,
            found,
        }) => Resolution::Missing {
            missing,
            required,
            optional,
            found,
        },
        Err(e) => Resolution::Unreadable(e),
    }
}

/// The header labels of a sample, for the `Ok` case (where no error carries them back). Goes
/// through the connector's `unwrap_envelope` so a framed report (Chase) reports its real header row
/// rather than its `BEGIN,…` frame line.
fn header_labels(
    source: &dyn super::source::SettlementReportSource,
    header_sample: &[u8],
) -> Vec<String> {
    let reader = source.unwrap_envelope(Box::new(Cursor::new(header_sample.to_vec())));
    csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(reader)
        .byte_headers()
        .map(|r| {
            r.iter()
                .map(|h| String::from_utf8_lossy(h).into_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> ConnectorRegistry {
        ConnectorRegistry::with_builtins()
    }

    /// A valid Adyen header row (required columns only, deliberately not in resolution order).
    const ADYEN_HEADER: &[u8] =
        b"Psp Reference,Record Type,Payment Method Variant,Global Card Brand,\
Issuer Country,Settlement Currency,Payable (SC),Commission (SC),Markup (SC),Scheme Fees (SC),\
Interchange (SC),ICSF details\n";

    #[test]
    fn accepts_a_valid_header() {
        let r = check(&registry(), "adyen", ADYEN_HEADER, ColumnMapping::none()).unwrap();
        assert!(r.ok, "valid header should pass: missing={:?}", r.missing);
        assert!(r.missing.is_empty());
        assert_eq!(r.matched.len(), r.required.len(), "everything matched");
        assert!(r.found.contains(&"ICSF details".to_string()));
        assert!(
            r.suggested_connectors.is_empty(),
            "no suggestions when the selected connector fits"
        );
    }

    /// The whole point: every missing column in one response, not just the first one resolved.
    #[test]
    fn reports_all_missing_columns_at_once() {
        // Drop three required columns, spread across the resolution order.
        let header = b"Psp Reference,Record Type,Global Card Brand,Issuer Country,\
Settlement Currency,Commission (SC),Markup (SC),Scheme Fees (SC),ICSF details\n";
        let r = check(&registry(), "adyen", header, ColumnMapping::none()).unwrap();

        assert!(!r.ok);
        let mut missing = r.missing.clone();
        missing.sort();
        assert_eq!(
            missing,
            vec!["Interchange (SC)", "Payable (SC)", "Payment Method Variant"],
            "all three misses reported together"
        );
        // And the merchant is told what *did* land, so the UI can show matched vs. unmatched.
        assert!(r.matched.contains(&"Commission (SC)".to_string()));
        assert!(!r.matched.contains(&"Payable (SC)".to_string()));
        assert_eq!(r.matched.len() + r.missing.len(), r.required.len());
    }

    /// Schema enumeration comes from the connector itself (empty-header probe), so it cannot drift
    /// from what `parse_rows` actually requires.
    #[test]
    fn enumerates_connector_schema_without_duplicating_it() {
        let r = check(
            &registry(),
            "adyen",
            b"Nothing,Useful\n",
            ColumnMapping::none(),
        )
        .unwrap();
        assert!(!r.ok);
        assert_eq!(
            r.required.len(),
            12,
            "adyen's 12 required columns, discovered by probing: {:?}",
            r.required
        );
        assert_eq!(
            r.missing.len(),
            12,
            "a header with none of them misses all of them"
        );
        assert!(r.matched.is_empty());
        // Optional columns are surfaced too, and are *not* counted as missing.
        assert!(r.optional.contains(&"Booking Date".to_string()));
        assert!(!r.missing.contains(&"Booking Date".to_string()));
        // The merchant's own headers come back for the left-hand column of the mapping UI.
        assert_eq!(r.found, vec!["Nothing".to_string(), "Useful".to_string()]);
    }

    /// The most common upload mistake is the wrong connector in the dropdown, which column mapping
    /// would paper over rather than fix. An Adyen file uploaded as Braintree should say so.
    #[test]
    fn suggests_the_connector_the_file_actually_matches() {
        let r = check(
            &registry(),
            "braintree",
            ADYEN_HEADER,
            ColumnMapping::none(),
        )
        .unwrap();
        assert!(!r.ok, "adyen's header is not a braintree report");
        assert_eq!(
            r.suggested_connectors
                .iter()
                .map(|s| s.connector.as_str())
                .collect::<Vec<_>>(),
            vec!["adyen"],
            "the file's real connector is suggested"
        );
    }

    /// Only a *complete* match is suggested — partial label overlap (`Settlement Currency`, card
    /// fields) is more likely coincidence than the right answer.
    #[test]
    fn does_not_suggest_on_partial_overlap() {
        let header = b"Settlement Currency,Card Brand,Issuer Country\n";
        let r = check(&registry(), "adyen", header, ColumnMapping::none()).unwrap();
        assert!(!r.ok);
        assert!(
            r.suggested_connectors.is_empty(),
            "partial overlap must not produce a guess, got {:?}",
            r.suggested_connectors
        );
    }

    /// A framed (Chase) report must be read through its envelope stripper, or the preflight would
    /// mistake the `BEGIN,…` frame line for the header row and reject a perfectly good file.
    #[test]
    fn sees_through_a_framed_report_envelope() {
        let framed: &[u8] = b"\xef\xbb\xbfBEGIN,EntityId=418553,Frequency=adhoc\n\
EntityId=418553,ReportTypeName=SubmissionDetails,Frequency=adhoc\n\
Submission Date,Settlement Currency Code,Payment Method Code,Merchant Order Number,\
Transaction Amount in Presentment Currency,Action Type Code Text,Country of Issuance,\
Interchange Qualification Code,Total Interchange Amount,Total Assessment Amount,Card Usage Type\n";
        let r = check(&registry(), "chase", framed, ColumnMapping::none()).unwrap();
        assert!(
            r.ok,
            "envelope must be stripped before the header is read: missing={:?}",
            r.missing
        );
        assert!(
            r.found.contains(&"Merchant Order Number".to_string()),
            "found reflects the real header row, not the BEGIN frame: {:?}",
            r.found
        );
    }

    /// A truncated sample (the dashboard sends only the first slice of the file) must still be
    /// judged on its header alone — a half-written final row is not a validation failure.
    #[test]
    fn tolerates_a_row_truncated_mid_field() {
        let mut sample = ADYEN_HEADER.to_vec();
        sample.extend_from_slice(b"REF1,Settled,visacredit,visa,IN,EUR,10.00,0.1");
        let r = check(&registry(), "adyen", &sample, ColumnMapping::none()).unwrap();
        assert!(r.ok, "truncation is not a header problem: {:?}", r.missing);
    }

    /// A renamed column is exactly what mapping exists for: the file is right, the label is not.
    #[test]
    fn a_mapping_rescues_a_renamed_column() {
        // Same Adyen report, but `Payable (SC)` exported as `Net Settlement Amount`.
        let header = b"Record Type,Psp Reference,Payment Method Variant,Global Card Brand,\
Issuer Country,Settlement Currency,Net Settlement Amount,Commission (SC),Markup (SC),\
Scheme Fees (SC),Interchange (SC),ICSF details\n";

        let unmapped = check(&registry(), "adyen", header, ColumnMapping::none()).unwrap();
        assert_eq!(unmapped.missing, vec!["Payable (SC)"]);

        let m = ColumnMapping::from_pairs(
            [(
                "Payable (SC)".to_string(),
                "Net Settlement Amount".to_string(),
            )]
            .into(),
        );
        let mapped = check(&registry(), "adyen", header, &m).unwrap();
        assert!(
            mapped.ok,
            "mapping resolves the rename: {:?}",
            mapped.missing
        );
        // The connector's own vocabulary is unchanged by the mapping — it is what the UI offers as
        // the right-hand column, so it must not shift as the merchant maps.
        assert_eq!(mapped.required, unmapped.required);
    }

    /// Full Adyen sample with one renamed fee column, used to exercise the preview guardrail.
    /// `Payable (SC)` is net-of-fees, so gross = payable + fees: 96.0 + 4.0 → an effective 4%.
    const RENAMED_SAMPLE: &[u8] = b"Record Type,Psp Reference,Payment Method Variant,\
Global Card Brand,Issuer Country,Settlement Currency,Net Settlement Amount,Commission (SC),\
Markup (SC),Scheme Fees (SC),Interchange (SC),ICSF details\n\
Settled,REF1,visacredit,visa,IN,EUR,96.00,1.00,0.50,0.50,2.00,\n";

    #[test]
    fn preview_shows_the_derived_values_a_mapping_produces() {
        let m = ColumnMapping::from_pairs(
            [(
                "Payable (SC)".to_string(),
                "Net Settlement Amount".to_string(),
            )]
            .into(),
        );
        let p = preview(&registry(), "adyen", RENAMED_SAMPLE, &m, false).unwrap();

        assert_eq!(p.rows.len(), 1);
        let r = &p.rows[0];
        assert!(
            (r.total_fee - 4.0).abs() < 1e-9,
            "fees summed: {}",
            r.total_fee
        );
        assert!(
            (r.gross - 100.0).abs() < 1e-9,
            "payable + fees: {}",
            r.gross
        );
        assert!((r.effective_pct - 4.0).abs() < 1e-9);
        assert_eq!(r.card_network, "visa");
        assert!(p.warning.is_none(), "4% is plausible: {:?}", p.warning);
    }

    /// The failure mode that motivates the preview: a mapping that is well-formed, parses, fits, and
    /// is wrong. Here the merchant points the *gross* column at a fee column, so the implied rate
    /// explodes — visible in the preview, invisible in the column pairing.
    #[test]
    fn preview_warns_when_a_mapping_implies_an_impossible_fee_rate() {
        let m = ColumnMapping::from_pairs(
            [("Payable (SC)".to_string(), "Commission (SC)".to_string())].into(),
        );
        let p = preview(&registry(), "adyen", RENAMED_SAMPLE, &m, false).unwrap();

        assert_eq!(p.rows.len(), 1);
        let pct = p.median_effective_pct.expect("a rate was derived");
        assert!(pct > 25.0, "gross collapsed to a fee column: {pct}%");
        assert!(
            p.warning
                .as_deref()
                .unwrap_or("")
                .contains("outside the range"),
            "the merchant must be warned: {:?}",
            p.warning
        );
    }

    /// A mapping that parses but yields nothing is its own kind of wrong, and silent otherwise.
    #[test]
    fn preview_warns_when_a_mapping_produces_no_rows() {
        // Keep the rename that makes the file parse, and additionally point `Record Type` at a
        // column whose values are never a fee-bearing record type, so every row is skipped.
        let m = ColumnMapping::from_pairs(
            [
                (
                    "Payable (SC)".to_string(),
                    "Net Settlement Amount".to_string(),
                ),
                ("Record Type".to_string(), "Global Card Brand".to_string()),
            ]
            .into(),
        );
        let p = preview(&registry(), "adyen", RENAMED_SAMPLE, &m, false).unwrap();
        assert!(p.rows.is_empty());
        assert!(
            p.warning
                .as_deref()
                .unwrap_or("")
                .contains("no fee-bearing rows"),
            "got {:?}",
            p.warning
        );
    }

    /// A mapping cannot paper over a file that is missing the column outright.
    #[test]
    fn preview_still_fails_when_a_required_column_is_absent() {
        let m = ColumnMapping::from_pairs(
            [("Payable (SC)".to_string(), "Not In This File".to_string())].into(),
        );
        assert!(matches!(
            preview(&registry(), "adyen", RENAMED_SAMPLE, &m, false),
            Err(IngestError::MissingColumns { .. })
        ));
    }

    #[test]
    fn unknown_connector_is_an_error_not_a_verdict() {
        assert!(matches!(
            check(
                &registry(),
                "not_a_psp",
                ADYEN_HEADER,
                ColumnMapping::none()
            ),
            Err(IngestError::UnknownConnector(_))
        ));
    }

    /// Stripe drives its own row loop; it must still aggregate misses like the shared driver does.
    #[test]
    fn hand_rolled_connector_also_aggregates() {
        let r = check(
            &registry(),
            "stripe",
            b"Card Brand,Funding Source\n",
            ColumnMapping::none(),
        )
        .unwrap();
        assert!(!r.ok);
        assert!(
            r.missing.len() > 1,
            "stripe must report every miss, not stop at the first: {:?}",
            r.missing
        );
        assert!(r.matched.contains(&"Card Brand".to_string()));
    }
}

#[cfg(test)]
mod renamed_report_tests {
    use super::*;
    use std::collections::HashMap;

    /// A realistic Adyen report as a merchant's BI export would emit it: five required columns
    /// renamed, the rest untouched, plus the lifecycle rows the parser skips. It must fail preflight
    /// with exactly those five, and the intended mapping must then parse it into plausible rows.
    ///
    /// Held inline rather than read from disk. An `include_bytes!` of a generated file is a trap
    /// here: the natural place to drop one (`scratch/`) is gitignored, so the tests would compile
    /// for whoever generated the file and fail to build for everyone else.
    const SAMPLE: &[u8] = b"\
Record Type,Psp Reference,Payment Method Variant,Global Card Brand,Issuer Country,Settlement Currency,Net Settlement Amount,Processing Commission,Markup (SC),Card Scheme Fee,Interchange Fee,IC Details JSON\n\
Settled,REF1,visacredit,visa,IN,EUR,97.70,0.80,0.15,0.15,1.20,\"[{\"t\":\"ic\",\"n\":\"Visa Consumer Credit Standard\"}]\"\n\
Settled,REF2,visadebit,visa,GB,GBP,244.52,1.90,0.40,0.38,2.80,\"[{\"t\":\"ic\",\"n\":\"Visa Consumer Debit Standard\"}]\"\n\
Settled,REF3,mccredit,mc,DE,EUR,73.74,0.62,0.11,0.11,0.92,\"[{\"t\":\"ic\",\"n\":\"Mastercard Consumer Credit Core\"}]\"\n\
Authorised,REF90,visacredit,visa,GB,GBP,,,,,,\n\
Settled,REF4,mcdebit,mc,FR,EUR,410.50,3.30,0.62,0.63,4.95,\"[{\"t\":\"ic\",\"n\":\"Mastercard Consumer Debit Core\"}]\"\n\
Settled,REF5,visacredit,visa,US,USD,38.95,0.33,0.06,0.06,0.50,\"[{\"t\":\"ic\",\"n\":\"Visa Consumer Credit Standard\"}]\"\n\
Settled,REF6,amexcredit,amex,NL,EUR,176.12,1.44,0.27,0.27,2.15,\"[{\"t\":\"ic\",\"n\":\"Amex Standard\"}]\"\n\
Settled,REF7,visabusinesscredit,visa,ES,EUR,93.80,0.77,0.14,0.14,1.15,\"[{\"t\":\"ic\",\"n\":\"Visa Consumer Credit Standard\"}]\"\n\
Received,REF91,mccredit,mc,DE,EUR,,,,,,\n\
Settled,REF8,mccredit,mc,GB,GBP,303.33,2.46,0.46,0.47,3.68,\"[{\"t\":\"ic\",\"n\":\"Mastercard Consumer Credit Core\"}]\"\n\
Settled,REF9,visadebit,visa,IN,EUR,56.85,0.47,0.09,0.09,0.70,\"[{\"t\":\"ic\",\"n\":\"Visa Consumer Debit Standard\"}]\"\n\
Settled,REF10,mcdebit,mc,US,USD,141.44,1.15,0.22,0.22,1.72,\"[{\"t\":\"ic\",\"n\":\"Mastercard Consumer Debit Core\"}]\"\n\
Settled,REF11,visacredit,visa,DE,EUR,207.74,1.69,0.32,0.32,2.53,\"[{\"t\":\"ic\",\"n\":\"Visa Consumer Credit Standard\"}]\"\n\
Settled,REF12,amexcredit,amex,GB,GBP,86.09,0.70,0.13,0.13,1.05,\"[{\"t\":\"ic\",\"n\":\"Amex Standard\"}]\"\n";

    fn intended_mapping() -> ColumnMapping {
        ColumnMapping::from_pairs(HashMap::from([
            (
                "Payable (SC)".to_string(),
                "Net Settlement Amount".to_string(),
            ),
            (
                "Commission (SC)".to_string(),
                "Processing Commission".to_string(),
            ),
            (
                "Scheme Fees (SC)".to_string(),
                "Card Scheme Fee".to_string(),
            ),
            (
                "Interchange (SC)".to_string(),
                "Interchange Fee".to_string(),
            ),
            ("ICSF details".to_string(), "IC Details JSON".to_string()),
        ]))
    }

    #[test]
    fn sample_fails_preflight_with_the_five_renamed_columns() {
        let r = check(
            &ConnectorRegistry::with_builtins(),
            "adyen",
            SAMPLE,
            ColumnMapping::none(),
        )
        .unwrap();
        assert!(!r.ok);
        let mut missing = r.missing.clone();
        missing.sort();
        assert_eq!(
            missing,
            vec![
                "Commission (SC)",
                "ICSF details",
                "Interchange (SC)",
                "Payable (SC)",
                "Scheme Fees (SC)",
            ]
        );
        assert_eq!(
            r.matched.len(),
            7,
            "the other seven line up: {:?}",
            r.matched
        );
        assert!(
            r.suggested_connectors.is_empty(),
            "must not look like another connector, or the UI steers away from mapping"
        );
    }

    #[test]
    fn intended_mapping_makes_the_sample_parse() {
        let r = check(
            &ConnectorRegistry::with_builtins(),
            "adyen",
            SAMPLE,
            &intended_mapping(),
        )
        .unwrap();
        assert!(r.ok, "still missing {:?}", r.missing);
    }

    #[test]
    fn intended_mapping_previews_plausible_rows() {
        let p = preview(
            &ConnectorRegistry::with_builtins(),
            "adyen",
            SAMPLE,
            &intended_mapping(),
            false,
        )
        .unwrap();
        assert_eq!(p.rows.len(), 10, "the preview fills");
        let median = p.median_effective_pct.expect("a rate was derived");
        assert!(
            (2.0..3.0).contains(&median),
            "should look like real card processing, got {median}%"
        );
        assert!(p.warning.is_none(), "no warning expected: {:?}", p.warning);
        // The interchange category is recovered from the mapped JSON column, not left blank.
        assert!(
            p.rows.iter().any(|r| !r.card_network.is_empty()),
            "networks parsed"
        );
    }

    /// The mistake the preview exists to catch, on the file the user will actually have in hand.
    #[test]
    fn mapping_gross_to_a_fee_column_is_caught() {
        let mut wrong = intended_mapping().columns().clone();
        wrong.insert("Payable (SC)".to_string(), "Interchange Fee".to_string());
        let p = preview(
            &ConnectorRegistry::with_builtins(),
            "adyen",
            SAMPLE,
            &ColumnMapping::from_pairs(wrong),
            false,
        )
        .unwrap();
        assert!(
            p.warning.is_some(),
            "an implausible rate must warn, median was {:?}",
            p.median_effective_pct
        );
    }
}

/// Does the mapping + preview machinery actually hold for *every* connector, not just the one it
/// was designed against? These parse a valid report per connector, rename one required column, and
/// assert the whole loop: preflight names the rename, a mapping fixes it, and the preview produces
/// rows. Anything connector-shaped that breaks the flow shows up here rather than in a merchant's
/// cost model.
#[cfg(test)]
mod all_connector_tests {
    use super::*;
    use std::collections::HashMap;

    /// `(connector, a valid report, a required column to rename, its stand-in label)`.
    fn cases() -> Vec<(&'static str, String, &'static str, &'static str)> {
        let adyen = "Record Type,Psp Reference,Payment Method Variant,Global Card Brand,\
Issuer Country,Settlement Currency,Payable (SC),Commission (SC),Markup (SC),Scheme Fees (SC),\
Interchange (SC),ICSF details\n\
Settled,REF1,visacredit,visa,IN,EUR,96.00,1.00,0.50,0.50,2.00,\n"
            .to_string();

        let braintree = "Transaction ID,Transaction Type,Settlement Currency,Settlement Amount,\
Card Brand,Card Type,Payment Instrument,Interchange Total Amount,Braintree Total Amount,\
Total Scheme Fees\n\
t1,sale,USD,100.00,Visa,Credit,credit_card,1.50,0.80,0.20\n"
            .to_string();

        let chase = "Merchant Order Number,Action Type Code Text,Payment Method Code,\
Card Usage Type,Country of Issuance,Settlement Currency Code,Interchange Qualification Code,\
Transaction Amount in Presentment Currency,Total Interchange Amount,Total Assessment Amount\n\
o1,SALE,VI,3,US,USD,VINT,100.00,-1.50,-0.20\n"
            .to_string();

        let checkout = "Payment ID,Action Type,Breakdown Type,Payment Method,Card Type,\
Issuer Country,Holding Currency,Holding Currency Amount,Processed On\n\
pay_1,Capture,Capture,VISA,Credit,GB,GBP,100.00,2026-07-09T18:07:06.844\n\
pay_1,Capture,Premium Variable Fee,VISA,Credit,GB,GBP,-1.50,2026-07-09T18:07:06.844\n\
pay_1,Capture,Scheme Fixed Fee,VISA,Credit,GB,GBP,-0.60,2026-07-09T18:07:06.844\n"
            .to_string();

        let stripe = "Card Brand,Shopper Interaction,Funding Source,Payment Method Variant,\
Gross Qty,Cost Qty,Refund Flag,Gross Ccy,Cost Ccy,Month,Fee Name,Variable Fee,Fixed Fee\n\
mc,Ecommerce,DEBIT,maestro,10000.00,250.00,Settle,USD,USD,2025/01,Scheme Fee,0.0165,0.15\n"
            .to_string();

        vec![
            ("adyen", adyen, "Payable (SC)", "Net Settlement Amount"),
            ("braintree", braintree, "Settlement Amount", "Settled Amt"),
            ("chase", chase, "Total Interchange Amount", "IC Amount"),
            ("checkout", checkout, "Holding Currency Amount", "Amount"),
            ("stripe", stripe, "Gross Qty", "Turnover"),
        ]
    }

    /// Every connector must (a) parse its own valid report and (b) route column lookups through the
    /// mapping layer — the rename must break it, and the mapping must repair it.
    #[test]
    fn mapping_applies_to_every_connector() {
        let reg = ConnectorRegistry::with_builtins();
        for (connector, report, column, renamed) in cases() {
            // Baseline: the untouched report parses.
            let base = check(&reg, connector, report.as_bytes(), ColumnMapping::none()).unwrap();
            assert!(
                base.ok,
                "{connector}: valid report rejected: {:?}",
                base.missing
            );

            // Rename one required column: preflight must name exactly that one.
            let renamed_report = report.replacen(column, renamed, 1);
            let broken = check(
                &reg,
                connector,
                renamed_report.as_bytes(),
                ColumnMapping::none(),
            )
            .unwrap();
            assert!(!broken.ok, "{connector}: rename went unnoticed");
            assert_eq!(broken.missing, vec![column], "{connector}");
            assert!(
                broken.found.iter().any(|f| f == renamed),
                "{connector}: the merchant's label must be offered as a mapping target"
            );

            // A mapping repairs it.
            let m = ColumnMapping::from_pairs(HashMap::from([(
                column.to_string(),
                renamed.to_string(),
            )]));
            let fixed = check(&reg, connector, renamed_report.as_bytes(), &m).unwrap();
            assert!(
                fixed.ok,
                "{connector}: mapping did not apply: {:?}",
                fixed.missing
            );
        }
    }

    /// The preview is the guardrail against a well-formed-but-wrong mapping, so it has to actually
    /// produce rows for every connector — including the ones that accumulate and flush at EOF
    /// (Checkout) or emit aggregate fee lines rather than transactions (Stripe).
    #[test]
    fn preview_produces_rows_for_every_connector() {
        let reg = ConnectorRegistry::with_builtins();
        for (connector, report, column, renamed) in cases() {
            let renamed_report = report.replacen(column, renamed, 1);
            let m = ColumnMapping::from_pairs(HashMap::from([(
                column.to_string(),
                renamed.to_string(),
            )]));
            let p = preview(&reg, connector, renamed_report.as_bytes(), &m, false).unwrap();
            assert!(
                !p.rows.is_empty(),
                "{connector}: preview produced no rows, so a wrong mapping would be invisible"
            );
            assert!(
                p.median_effective_pct.is_some(),
                "{connector}: no effective rate derived, so the plausibility check cannot fire"
            );
        }
    }
}

/// Partial-read behaviour. The preflight parses only the first `HEADER_SAMPLE_BYTES` of a file, and
/// a connector that assembles one row from several report rows ends that slice mid-group. These pin
/// that the artefact never reaches the merchant as a mapping warning.
#[cfg(test)]
mod truncation_tests {
    use super::*;

    const CHECKOUT_HEADER: &str = "Payment ID,Action Type,Breakdown Type,Payment Method,Card Type,\
Issuer Country,Holding Currency,Holding Currency Amount,Processed On\n";

    /// A Checkout report of `n` complete payments (capture + two fee lines each, ~2.1% all-in).
    fn checkout_report(n: usize) -> String {
        let mut s = String::from(CHECKOUT_HEADER);
        for i in 0..n {
            for (bd, amt) in [
                ("Capture", "100.00"),
                ("Premium Variable Fee", "-1.50"),
                ("Scheme Fixed Fee", "-0.60"),
            ] {
                s.push_str(&format!(
                    "pay_{i},Capture,{bd},VISA,Credit,GB,GBP,{amt},2026-07-09T18:07:06.844\n"
                ));
            }
        }
        s
    }

    /// The regression: a correct mapping on a truncated Checkout sample must not be reported as
    /// wrong. Before the `groups_rows` fix the trailing half-assembled payment pulled the median to
    /// 0.0% and fired the "outside the range real card processing falls in" warning.
    #[test]
    fn truncated_grouping_connector_does_not_warn_on_a_correct_mapping() {
        // Enough payments to exceed the cap, then cut exactly as the browser's slice would — which
        // lands mid-payment, leaving a capture whose fee lines never arrive.
        let full = checkout_report(4000);
        assert!(
            full.len() > HEADER_SAMPLE_BYTES,
            "fixture must exceed the cap"
        );
        let cut = &full.as_bytes()[..HEADER_SAMPLE_BYTES];

        let p = preview(
            &ConnectorRegistry::with_builtins(),
            "checkout",
            cut,
            ColumnMapping::none(),
            // The sample is a genuine mid-file cut, which is the whole point of these tests.
            true,
        )
        .unwrap();

        let median = p.median_effective_pct.expect("a rate was derived");
        assert!(
            (2.0..2.2).contains(&median),
            "the incomplete trailing payment must not skew the rate, got {median}%"
        );
        assert!(
            p.warning.is_none(),
            "a correct mapping must not be flagged: {:?}",
            p.warning
        );
        assert!(
            p.rows.iter().all(|r| r.total_fee.abs() > f64::EPSILON),
            "no half-assembled payment may reach the merchant"
        );
    }

    /// The fix must not blunt the guardrail it protects: a genuinely wrong mapping on the same
    /// truncated sample still has to warn.
    #[test]
    fn truncation_handling_still_catches_a_wrong_mapping() {
        let full = checkout_report(4000);
        let cut = &full.as_bytes()[..HEADER_SAMPLE_BYTES];
        // Point the amount column at a text column, collapsing every derived amount.
        let wrong = ColumnMapping::from_pairs(
            [(
                "Holding Currency Amount".to_string(),
                "Payment Method".to_string(),
            )]
            .into(),
        );
        let p = preview(
            &ConnectorRegistry::with_builtins(),
            "checkout",
            cut,
            &wrong,
            true,
        )
        .unwrap();
        assert!(
            p.warning.is_some(),
            "a wrong mapping must still be caught, median {:?}",
            p.median_effective_pct
        );
    }

    /// Non-grouping connectors are unaffected: one report row is one emitted row, so a cut can only
    /// drop a whole row, never leave a half-built one.
    #[test]
    fn non_grouping_connectors_are_untouched_by_truncation() {
        let mut s = String::from(
            "Record Type,Psp Reference,Payment Method Variant,Global Card Brand,Issuer Country,\
Settlement Currency,Payable (SC),Commission (SC),Markup (SC),Scheme Fees (SC),Interchange (SC),\
ICSF details\n",
        );
        for i in 0..3000 {
            s.push_str(&format!(
                "Settled,REF{i},visacredit,visa,IN,EUR,96.00,1.00,0.50,0.50,2.00,\n"
            ));
        }
        let cut = &s.as_bytes()[..HEADER_SAMPLE_BYTES];
        let p = preview(
            &ConnectorRegistry::with_builtins(),
            "adyen",
            cut,
            ColumnMapping::none(),
            // The sample is a genuine mid-file cut, which is the whole point of these tests.
            true,
        )
        .unwrap();
        assert!((p.median_effective_pct.unwrap() - 4.0).abs() < 1e-9);
        assert!(p.warning.is_none(), "{:?}", p.warning);
    }
}

/// The wire contract with the dashboard. These assert the exact JSON field names the frontend's
/// `PreflightReport` / `PreviewReport` interfaces destructure, because nothing else does: the
/// structs are serialised by `serde` and consumed by hand-written TypeScript, so a renamed or
/// dropped field compiles cleanly on both sides and fails silently in the browser — a mapping panel
/// rendering blank columns with no error anywhere.
#[cfg(test)]
mod wire_contract_tests {
    use super::*;

    fn keys(v: &serde_json::Value) -> Vec<String> {
        let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    }

    #[test]
    fn preflight_report_matches_the_frontend_interface() {
        let r = check(
            &ConnectorRegistry::with_builtins(),
            "adyen",
            b"Nothing,Useful\n",
            ColumnMapping::none(),
        )
        .unwrap();
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(
            keys(&json),
            vec![
                "connector",
                "found",
                "matched",
                "missing",
                "ok",
                "optional",
                "optional_missing",
                "required",
                "suggested_connectors",
            ],
            "field set drifted from the dashboard's PreflightReport"
        );
        // The suggestion shape the panel reads for its wrong-connector hint.
        let s = check(
            &ConnectorRegistry::with_builtins(),
            "braintree",
            b"Record Type,Psp Reference,Payment Method Variant,Global Card Brand,Issuer Country,\
Settlement Currency,Payable (SC),Commission (SC),Markup (SC),Scheme Fees (SC),Interchange (SC),\
ICSF details\n",
            ColumnMapping::none(),
        )
        .unwrap();
        let sj = serde_json::to_value(&s).unwrap();
        let first = &sj["suggested_connectors"][0];
        assert_eq!(keys(first), vec!["connector", "matched_required"]);
    }

    #[test]
    fn preview_report_matches_the_frontend_interface() {
        let sample = "Record Type,Psp Reference,Payment Method Variant,Global Card Brand,\
Issuer Country,Settlement Currency,Payable (SC),Commission (SC),Markup (SC),Scheme Fees (SC),\
Interchange (SC),ICSF details\n\
Settled,REF1,visacredit,visa,IN,EUR,96.00,1.00,0.50,0.50,2.00,\n";
        let p = preview(
            &ConnectorRegistry::with_builtins(),
            "adyen",
            sample.as_bytes(),
            ColumnMapping::none(),
            false,
        )
        .unwrap();
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(
            keys(&json),
            vec!["median_effective_pct", "rows", "warning"],
            "field set drifted from the dashboard's PreviewReport"
        );
        assert_eq!(
            keys(&json["rows"][0]),
            vec![
                "card_network",
                "commission",
                "currency",
                "effective_pct",
                "funding",
                "gross",
                "interchange",
                "issuer_country",
                "markup",
                "scheme_fee",
                "total_fee",
                "variant",
            ],
            "field set drifted from the dashboard's PreviewRow"
        );
        // `null` rather than an omitted key — the dashboard tests `median_effective_pct != null`.
        let empty = preview(
            &ConnectorRegistry::with_builtins(),
            "adyen",
            "Record Type,Psp Reference,Payment Method Variant,Global Card Brand,Issuer Country,\
Settlement Currency,Payable (SC),Commission (SC),Markup (SC),Scheme Fees (SC),Interchange (SC),\
ICSF details\n"
                .as_bytes(),
            ColumnMapping::none(),
            false,
        )
        .unwrap();
        let ej = serde_json::to_value(&empty).unwrap();
        assert!(ej["median_effective_pct"].is_null());
        assert!(
            ej["warning"].is_string(),
            "an empty preview must explain itself"
        );
    }

    /// The mapping is stored and returned as `{ "columns": { expected: theirs } }`, which is the
    /// shape `useColumnMapping` unwraps.
    #[test]
    fn stored_mapping_round_trips_in_the_shape_the_dashboard_expects() {
        let m = super::super::mapping::ColumnMapping::from_pairs(
            [("Payable (SC)".to_string(), "Net Amount".to_string())].into(),
        );
        let json = serde_json::to_value(&m).unwrap();
        assert_eq!(keys(&json), vec!["columns"]);
        assert_eq!(json["columns"]["Payable (SC)"], "Net Amount");
        // And it must deserialise back from exactly what the dashboard PUTs.
        let back: super::super::mapping::ColumnMapping =
            serde_json::from_value(json).expect("round trip");
        assert_eq!(back, m);
    }
}
