use std::collections::HashMap;

use anyhow::Context;
use csv::StringRecord;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Serialize, Deserialize)]
struct Config {
    amount_field: String,
    processor_field: String,
    payment_method_field: String,
    all_fields: Vec<String>,
    non_filterable_fields: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Stats {
    total: usize,
    total_amount: f64,
    /// sorted in descending order of frequency
    processors: Vec<String>,
    /// sorted in descending order of frequency
    #[serde(rename = "payment_method_types")]
    payment_methods: Vec<String>,
}

#[derive(Serialize)]
struct Output {
    #[serde(with = "serde_bytes")]
    data: Vec<u8>,
    stats: Stats,
}

fn extract_transform_load(data: &[u8], config: Config) -> anyhow::Result<(Vec<u8>, Stats)> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(data);

    let headers = reader
        .headers()
        .context("Failed to read CSV headers")?
        .clone();

    let amount_idx = headers
        .iter()
        .position(|h| h == config.amount_field)
        .context(format!(
            "Amount field '{}' not found in headers",
            config.amount_field
        ))?;

    let processor_idx = headers
        .iter()
        .position(|h| h == config.processor_field)
        .context(format!(
            "Processor field '{}' not found in headers",
            config.processor_field
        ))?;

    let payment_method_idx = headers
        .iter()
        .position(|h| h == config.payment_method_field)
        .context(format!(
            "Payment method field '{}' not found in headers",
            config.payment_method_field
        ))?;

    let mut output_headers = Vec::with_capacity(config.all_fields.len());
    // map from input header index to output header index
    let mut header_map: Vec<(usize, usize)> = Vec::new();
    let mut filter_omitted_header_map: Vec<usize> = Vec::new();

    for (i, header) in headers.iter().enumerate() {
        if config.all_fields.contains(&header.to_string()) {
            output_headers.push(header);
            header_map.push((output_headers.len() - 1, i));
            if config.non_filterable_fields.contains(&header.to_string()) {
                filter_omitted_header_map.push(i);
            }
        }
    }

    header_map.sort();

    let mut output = Vec::with_capacity(data.len());
    let mut writer = csv::WriterBuilder::new()
        .has_headers(true)
        .from_writer(&mut output);

    writer
        .write_record(&output_headers)
        .context("Failed to write headers")?;

    type Agg = (i32, f64, HashMap<String, usize>, HashMap<String, usize>);

    let extrapolate_amount = |mut acc: Agg, row: &StringRecord| {
        let amount = row.get(amount_idx).and_then(|s| s.parse::<f64>().ok())?;
        acc.0 += 1;
        acc.1 += amount;
        Some(acc)
    };

    let extrapolate_processors = |mut acc: Agg, row: &StringRecord| {
        let processor = row.get(processor_idx)?;
        if processor.is_empty() {
            return None;
        }
        *acc.2.entry(processor.to_string()).or_insert(0) += 1;
        Some(acc)
    };

    let extrapolate_payment_methods = |mut acc: Agg, row: &StringRecord| {
        let payment_method = row.get(payment_method_idx)?;

        if payment_method.is_empty() {
            return None;
        }

        *acc.3.entry(payment_method.to_string()).or_insert(0) += 1;
        Some(acc)
    };

    let start: Agg = (
        // total
        0,
        // total_amount
        0.,
        // processors
        HashMap::<String, usize>::new(),
        // payment_methods
        HashMap::<String, usize>::new(),
    );

    let stats = reader
        .into_records()
        .flat_map(|record| record.ok())
        .try_fold(start, |acc, row| {
            let output = extrapolate_amount(acc.clone(), &row)
                .and_then(|acc| extrapolate_processors(acc, &row))
                .and_then(|acc| extrapolate_payment_methods(acc, &row));

            if let Some(o) = output {
                let mut new_record = csv::StringRecord::new();
                for (_, j) in &header_map {
                    let value = row.get(*j);

                    match value {
                        Some("") | None => {
                            if filter_omitted_header_map.contains(j) {
                                new_record.push_field(value.unwrap_or(""))
                            } else {
                                return Ok::<_, anyhow::Error>(acc);
                            }
                        }
                        Some(value) => new_record.push_field(value),
                    }
                }
                writer
                    .write_record(&new_record)
                    .context("failed to write a row")?;
                Ok(o)
            } else {
                Ok(acc)
            }
        })
        .context("failed to create aggregates")?;

    let stats = Stats {
        total: stats.0 as usize,
        total_amount: stats.1,
        processors: {
            let mut v: Vec<_> = stats.2.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            v.into_iter().map(|(k, _)| k).collect()
        },
        payment_methods: {
            let mut v: Vec<_> = stats.3.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            v.into_iter().map(|(k, _)| k).collect()
        },
    };

    writer
        .flush()
        .context("failed to flush data to the buffer")?;
    drop(writer);

    Ok((output, stats))
}

#[wasm_bindgen]
pub fn validate_extract(data: &[u8], config: JsValue) -> Result<JsValue, JsValue> {
    let output: Config =
        serde_wasm_bindgen::from_value(config).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let (data, agg) =
        extract_transform_load(data, output).map_err(|e| JsValue::from_str(&e.to_string()))?;

    serde_wasm_bindgen::to_value(&Output { data, stats: agg })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
