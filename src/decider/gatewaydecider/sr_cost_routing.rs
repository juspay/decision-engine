use super::types;
use crate::logger;
use std::cmp::Ordering;
use std::collections::HashMap;

// Structure to hold gap analysis results
struct GapAnalysisResult {
    // Whether a significant gap was found
    has_significant_gap: bool,
    // The index where the gap occurs
    gap_index: usize,
    // The gap value
    // Weight for success rate
    sr_weight: f64,
    // Weight for cost
    cost_weight: f64,
}

// Function to prepare entries with original scores
fn prepare_entries(entries: &[(String, f64, f64)]) -> Vec<(String, f64, f64)> {
    // Create a vector with original scores
    entries
        .iter()
        .map(|(key, score, cost)| {
            // Use the original score
            (key.clone(), *score, *cost)
        })
        .collect()
}

// Function to analyze success rates and determine weights based on a fixed threshold
fn analyze_gaps_and_weights(entries: &[(String, f64, f64)]) -> GapAnalysisResult {
    // Default result with no significant gap and equal weights
    let mut result = GapAnalysisResult {
        has_significant_gap: false,
        gap_index: 0,
        sr_weight: 1.0,
        cost_weight: 1.0,
    };

    // If we have at least 1 entry, analyze success rates
    if !entries.is_empty() {
        // Fixed threshold for success rate (85%)
        let sr_threshold = 0.85;

        // Find entries above the threshold
        let above_threshold: Vec<&(String, f64, f64)> = entries
            .iter()
            .filter(|(_, score, _)| *score >= sr_threshold)
            .collect();

        // If we have entries above the threshold, use them for weight determination
        if !above_threshold.is_empty() {
            // Mark that we have a significant gap (entries above threshold)
            result.has_significant_gap = true;
            result.gap_index = above_threshold.len() - 1;

            // Calculate spread within entries above threshold
            let sr_spread = if above_threshold.len() > 1 {
                above_threshold[0].1 - above_threshold[above_threshold.len() - 1].1
            } else {
                0.0
            };

            // Fixed spread value for weight determination (5%)
            let spread_threshold = 0.02;

            // Calculate weights based on spread
            // If spread is small, cost dominates; if spread is large, SR dominates
            if sr_spread < spread_threshold {
                // Small spread, cost dominates
                result.sr_weight = 1.0;
                result.cost_weight = 3.0;
            } else {
                // Large spread, SR dominates
                result.sr_weight = 3.0;
                result.cost_weight = 1.0;
            }

            logger::info!(
                tag = "Weight_Determination",
                action = "Weight_Determination",
                "Entries above threshold ({}): {}. SR spread: {}. Using weights - SR: {}, Cost: {}",
                sr_threshold,
                above_threshold.len(),
                sr_spread,
                result.sr_weight,
                result.cost_weight
            );
        } else {
            // No entries above threshold, use equal weights
            logger::info!(
                tag = "Weight_Determination",
                action = "Weight_Determination",
                "No entries above threshold ({}). Using equal weights - SR: {}, Cost: {}",
                sr_threshold,
                result.sr_weight,
                result.cost_weight
            );
        }
    }

    result
}

// Function to calculate weighted Euclidean distance and sort network-gateway combinations
// using cluster-based gap analysis - original version
pub fn sort_by_euclidean_distance_original(
    combined_map: &mut Vec<types::SUPERROUTERPRIORITYMAP>,
) -> Vec<types::SUPERROUTERPRIORITYMAP> {
    // Create a vector of (key, score, cost) tuples
    let mut entries: Vec<(String, f64, f64)> = Vec::new();

    // Extract data from combined_map
    for entry in combined_map.iter() {
        if let (Some(score), Some(cost)) = (entry.success_rate, entry.saving_normalized) {
            let key = format!("{}_{}", entry.payment_method, entry.gateway);
            entries.push((key, score, cost));
        }
    }

    // Prepare entries with original scores
    let mut sorted_entries = prepare_entries(&entries);

    // Sort by score in descending order
    sorted_entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

    // Log the sorted entries
    logger::info!(
        tag = "Cluster_Analysis",
        action = "Cluster_Analysis",
        "Sorted entries by score (descending): {:?}",
        sorted_entries
    );

    // Analyze gaps and determine weights using original scores
    let gap_analysis = analyze_gaps_and_weights(&sorted_entries);

    // Create a map to store calculated distances
    let mut distance_map: HashMap<String, f64> = HashMap::new();

    // Calculate weighted Euclidean distance for each entry using original scores
    for (i, (key, score, cost)) in sorted_entries.iter().enumerate() {
        // Determine if this entry is in the top cluster
        let is_in_top_cluster = !gap_analysis.has_significant_gap || i <= gap_analysis.gap_index;

        // Apply weights based on cluster
        let (entry_sr_weight, entry_cost_weight) = if is_in_top_cluster {
            // Use calculated weights for top cluster
            (gap_analysis.sr_weight, gap_analysis.cost_weight)
        } else {
            // Use standard weights for bottom cluster
            (1.0, 1.0)
        };

        // Calculate weighted Euclidean distance using original score
        let distance = (entry_sr_weight * (1.0 - score).powi(2)
            + entry_cost_weight * (1.0 - cost).powi(2))
        .sqrt();

        // Store in map
        distance_map.insert(key.clone(), distance);

        // Log the entry details
        logger::info!(
            tag = "Entry_Processing",
            action = "Entry_Processing",
            "Key: {}, Score: {}, Cost: {}, In Top Cluster: {}, Weights (SR: {}, Cost: {}), Distance: {}",
            key,
            score,
            cost,
            is_in_top_cluster,
            entry_sr_weight,
            entry_cost_weight,
            distance
        );
    }

    // Create a map of scores for each entry
    let mut score_map: HashMap<String, f64> = HashMap::new();
    for (key, score, _) in &sorted_entries {
        score_map.insert(key.clone(), *score);
    }

    // Update the combined_score in the original combined_map
    for entry in combined_map.iter_mut() {
        let key = format!("{}_{}", entry.payment_method, entry.gateway);
        if let Some(&distance) = distance_map.get(&key) {
            entry.combined_score = Some(distance);
        }
    }

    // Create a clone of the combined_map for sorting
    let mut sorted_combined_map = combined_map.clone();

    // Sort by distance (ascending), then by normalized score (descending), then by cost (descending)
    sorted_combined_map.sort_by(|a, b| {
        let a_distance = a.combined_score.unwrap_or(f64::MAX);
        let b_distance = b.combined_score.unwrap_or(f64::MAX);

        // Get scores from the map
        let a_score = a.success_rate.unwrap_or(0.0);
        let b_score = b.success_rate.unwrap_or(0.0);

        let a_cost = a.saving_normalized.unwrap_or(0.0);
        let b_cost = b.saving_normalized.unwrap_or(0.0);

        // Compare distances
        match a_distance
            .partial_cmp(&b_distance)
            .unwrap_or(Ordering::Equal)
        {
            Ordering::Equal => {
                // If distances are equal, compare normalized scores (higher score first)
                match b_score.partial_cmp(&a_score).unwrap_or(Ordering::Equal) {
                    Ordering::Equal => {
                        // If scores are equal, compare costs (higher cost first)
                        b_cost.partial_cmp(&a_cost).unwrap_or(Ordering::Equal)
                    }
                    other => other,
                }
            }
            other => other,
        }
    });

    // Log the final sorted results
    logger::info!(
        tag = "Cluster_Based_Sorting",
        action = "Cluster_Based_Sorting",
        "Final sorted SUPERROUTERPRIORITYMAP results: {:?}",
        sorted_combined_map
    );

    sorted_combined_map
}
