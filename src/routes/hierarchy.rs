//! Account-hierarchy provisioning: `POST /admin/hierarchy/reconcile` and
//! `POST /admin/hierarchy/sync`.
//!
//! Both take the HS org → merchant → profile tree and are gated by the shared admin secret,
//! like `/merchant-account/create`. Neither creates users nor sends email — HS-backed
//! deployments provision scopes entirely server-to-server.
//!
//! `reconcile` is read-only and is meant to run first: it classifies every existing DE scope
//! against the submitted tree so that stranded rows (see [`ScopeClassification::Stranded`]) are
//! dispositioned by a person *before* anything is written.
//!
//! `sync` then writes ancestry forward (onto each scope) and rebuilds the reverse indexes. It
//! is idempotent — re-running with an unchanged tree reports zero changes.

use axum::http::HeaderMap;
use axum::Json;
use masking::PeekInterface;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::app::APP_STATE;
use crate::error;
use crate::logger;
use crate::types::merchant::hierarchy::{
    self, Hierarchy, MerchantEntry, OrgEntry, OrgIndex, HIERARCHY_SHAPE_VERSION,
};
use crate::types::merchant::merchant_account as MA;
use crate::utils::date_time;

// ---------------------------------------------------------------------------------------
// Request — mirrors the HS tree, so it uses HS's own field names
// ---------------------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct HierarchyRequest {
    pub orgs: Vec<OrgNode>,

    /// `reconcile` only. By default the report lists just the scopes needing a decision —
    /// stranded ones. A real deployment has hundreds to thousands of unlinked scopes (test
    /// merchants, DE-native accounts) that are correct as they are and would bury the rows
    /// that matter. Set true for the full inventory.
    #[serde(default)]
    pub include_all_scopes: bool,

    /// `sync` only. Detecting stranded scopes requires reading every merchant account, which
    /// is fine for the periodic full-tree sync but not for the per-profile call made from
    /// HS's profile-create path. Set false there; run `reconcile` for the full picture.
    #[serde(default = "default_report_stranded")]
    pub report_stranded: bool,
}

fn default_report_stranded() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrgNode {
    pub org_id: String,
    pub org_name: Option<String>,
    #[serde(default)]
    pub merchants: Vec<MerchantNode>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MerchantNode {
    /// The HS **merchant account** ID — ancestry, never a routing scope.
    pub merchant_id: String,
    pub merchant_name: Option<String>,
    #[serde(default)]
    pub profiles: Vec<ProfileNode>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProfileNode {
    /// The HS profile ID — this *is* the DE routing scope.
    pub profile_id: String,
    pub profile_name: Option<String>,
}

// ---------------------------------------------------------------------------------------
// Responses
// ---------------------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct SyncResponse {
    pub orgs_seen: usize,
    pub merchants_seen: usize,
    pub profiles_created: usize,
    pub profiles_updated: usize,
    pub profiles_unchanged: usize,
    /// Scopes whose ID matches an HS *merchant* rather than a profile. Never written to;
    /// reported on every run so they stay visible until dispositioned.
    pub stranded: Vec<String>,
}

/// How one existing DE scope relates to the submitted HS tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeClassification {
    /// Matches an HS profile. Sync writes ancestry; nothing else about the row changes.
    Linked,
    /// HS does not recognise this ID. Left untouched and fully functional, without ancestry —
    /// expected for standalone and DE-native merchants.
    Unlinked,
    /// The ID is an HS *merchant* ID, not a profile ID. The row holds live rules, SR config,
    /// and accumulated scores that no profile will inherit: once its merchant's profiles are
    /// synced, traffic follows those and this row's configuration is silently orphaned.
    /// Requires a per-row decision — DE cannot know which profile a merchant-level rule meant.
    Stranded,
}

#[derive(Debug, Serialize)]
pub struct ScopeReport {
    pub scope_id: String,
    pub classification: ScopeClassification,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hs_merchant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hs_org_id: Option<String>,
    /// True when ancestry is already stored — i.e. a previous sync linked this scope.
    pub has_ancestry: bool,
}

#[derive(Debug, Serialize)]
pub struct ReconcileResponse {
    pub linked: usize,
    pub unlinked: usize,
    pub stranded: usize,
    /// HS profiles with no DE scope yet. These are what `sync` creates, and what a
    /// `TE_04 Merchant not found` on the SSO handoff means.
    pub missing_in_de: Vec<String>,
    /// Stranded scopes only, unless `include_all_scopes` was set on the request.
    pub scopes: Vec<ScopeReport>,
}

// ---------------------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------------------

fn verify_admin_secret(
    headers: &HeaderMap,
) -> Result<(), error::ContainerError<error::MerchantAccountConfigurationError>> {
    let global_config = APP_STATE
        .get()
        .map(|state| state.global_config.clone())
        .ok_or(error::MerchantAccountConfigurationError::StorageError)?;

    let provided = headers
        .get("x-admin-secret")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");

    if provided != global_config.admin_secret.secret.peek().as_str() {
        return Err(error::MerchantAccountConfigurationError::Unauthorized.into());
    }
    Ok(())
}

/// Read-only classification of every existing scope against the submitted tree. Writes nothing.
#[axum::debug_handler]
pub async fn reconcile_hierarchy(
    headers: HeaderMap,
    Json(payload): Json<HierarchyRequest>,
) -> Result<Json<ReconcileResponse>, error::ContainerError<error::MerchantAccountConfigurationError>>
{
    verify_admin_secret(&headers)?;

    let index = TreeIndex::build(&payload);

    let accounts = MA::load_all_merchant_accounts()
        .await
        .map_err(|_| error::MerchantAccountConfigurationError::StorageError)?;

    let mut scopes = Vec::with_capacity(accounts.len());
    let (mut linked, mut unlinked, mut stranded) = (0usize, 0usize, 0usize);
    let mut seen_profiles = HashSet::new();

    for account in &accounts {
        let scope_id = crate::types::merchant::id::merchant_id_to_text(account.merchantId.clone());
        let ancestry = hierarchy::of_account(account);

        let classification = index.classify(&scope_id);
        match classification {
            ScopeClassification::Linked => {
                linked += 1;
                seen_profiles.insert(scope_id.clone());
            }
            ScopeClassification::Unlinked => unlinked += 1,
            ScopeClassification::Stranded => stranded += 1,
        }

        // Stranded rows always appear — they are the only class where doing nothing loses
        // working configuration, so they must never be scrolled past.
        if payload.include_all_scopes || classification == ScopeClassification::Stranded {
            let parents = index.parents_of(&scope_id);
            scopes.push(ScopeReport {
                scope_id,
                classification,
                hs_merchant_id: parents
                    .map(|(merchant_id, _)| merchant_id.to_string())
                    .or_else(|| ancestry.as_ref().map(|h| h.hs_merchant_id.clone())),
                hs_org_id: parents
                    .map(|(_, org_id)| org_id.to_string())
                    .or_else(|| ancestry.as_ref().map(|h| h.hs_org_id.clone())),
                has_ancestry: ancestry.is_some(),
            });
        }
    }

    let missing_in_de = index
        .profile_parents
        .keys()
        .filter(|profile_id| !seen_profiles.contains(*profile_id))
        .cloned()
        .collect::<Vec<_>>();

    logger::info!(
        category = "HIERARCHY",
        action = "reconcile",
        linked,
        unlinked,
        stranded,
        missing_in_de = missing_in_de.len(),
        "Hierarchy reconcile report generated"
    );

    Ok(Json(ReconcileResponse {
        linked,
        unlinked,
        stranded,
        missing_in_de,
        scopes,
    }))
}

/// Upserts a scope per profile, writes ancestry forward, and rebuilds the reverse indexes.
#[axum::debug_handler]
pub async fn sync_hierarchy(
    headers: HeaderMap,
    Json(payload): Json<HierarchyRequest>,
) -> Result<Json<SyncResponse>, error::ContainerError<error::MerchantAccountConfigurationError>> {
    verify_admin_secret(&headers)?;

    let index = TreeIndex::build(&payload);
    let synced_at = synced_at_stamp();

    let mut created = 0usize;
    let mut updated = 0usize;
    let mut unchanged = 0usize;

    for org in &payload.orgs {
        for merchant in &org.merchants {
            for profile in &merchant.profiles {
                let desired = Hierarchy {
                    v: HIERARCHY_SHAPE_VERSION,
                    hs_org_id: org.org_id.clone(),
                    hs_org_name: org.org_name.clone(),
                    hs_merchant_id: merchant.merchant_id.clone(),
                    hs_merchant_name: merchant.merchant_name.clone(),
                    profile_name: profile.profile_name.clone(),
                    synced_at: Some(synced_at.clone()),
                };

                let existing = MA::load_merchant_by_merchant_id(profile.profile_id.clone()).await;

                match existing {
                    None => {
                        // The scope does not exist yet — this is the row whose absence makes the
                        // SSO handoff return TE_04.
                        MA::insert_merchant_account(MA::MerchantAccountCreateRequest {
                            merchant_id: profile.profile_id.clone(),
                            gateway_success_rate_based_decider_input: None,
                        })
                        .await
                        .map_err(|_| {
                            error::MerchantAccountConfigurationError::MerchantInsertionFailed
                        })?;

                        MA::update_merchant_hierarchy(profile.profile_id.clone(), &desired)
                            .await
                            .map_err(|_| error::MerchantAccountConfigurationError::StorageError)?;
                        created += 1;
                    }
                    Some(account) => {
                        // Compare ignoring `synced_at`, which changes on every run and would
                        // otherwise make every scope look modified.
                        let current = hierarchy::of_account(&account);
                        let is_unchanged = current
                            .as_ref()
                            .map(|stored| {
                                let mut stored = stored.clone();
                                stored.synced_at = desired.synced_at.clone();
                                stored == desired
                            })
                            .unwrap_or(false);

                        if is_unchanged {
                            unchanged += 1;
                        } else {
                            MA::update_merchant_hierarchy(profile.profile_id.clone(), &desired)
                                .await
                                .map_err(|_| {
                                    error::MerchantAccountConfigurationError::StorageError
                                })?;
                            updated += 1;
                        }
                    }
                }
            }
        }
    }

    write_reverse_indexes(&payload)
        .await
        .map_err(|_| error::MerchantAccountConfigurationError::StorageError)?;

    // Stranded scopes stay visible after the initial reconcile, since they are the one class
    // where doing nothing loses working configuration — unless the caller opted out because
    // this is a per-profile sync that cannot afford the full table read.
    let stranded = if payload.report_stranded {
        collect_stranded(&index).await
    } else {
        Vec::new()
    };

    logger::info!(
        category = "HIERARCHY",
        action = "sync",
        orgs = payload.orgs.len(),
        profiles_created = created,
        profiles_updated = updated,
        profiles_unchanged = unchanged,
        stranded = stranded.len(),
        "Hierarchy sync completed"
    );

    Ok(Json(SyncResponse {
        orgs_seen: payload.orgs.len(),
        merchants_seen: payload.orgs.iter().map(|o| o.merchants.len()).sum(),
        profiles_created: created,
        profiles_updated: updated,
        profiles_unchanged: unchanged,
        stranded,
    }))
}

// ---------------------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------------------

/// ISO-8601 UTC stamp for `Hierarchy::synced_at`. Diagnostic only — nothing branches on it, so
/// an unformattable clock degrades to an absent stamp rather than failing the sync.
fn synced_at_stamp() -> String {
    date_time::now()
        .assume_utc()
        .format(&time::format_description::well_known::Iso8601::DEFAULT)
        .unwrap_or_default()
}

/// Lookup tables over the submitted tree, so classification is a hash lookup per scope rather
/// than a walk of the whole tree.
struct TreeIndex {
    /// profile_id → (hs_merchant_id, hs_org_id)
    profile_parents: HashMap<String, (String, String)>,
    /// Every hs_merchant_id in the tree — used to spot scopes registered with a merchant ID.
    merchant_ids: HashSet<String>,
}

impl TreeIndex {
    fn build(request: &HierarchyRequest) -> Self {
        let mut profile_parents = HashMap::new();
        let mut merchant_ids = HashSet::new();

        for org in &request.orgs {
            for merchant in &org.merchants {
                merchant_ids.insert(merchant.merchant_id.clone());
                for profile in &merchant.profiles {
                    profile_parents.insert(
                        profile.profile_id.clone(),
                        (merchant.merchant_id.clone(), org.org_id.clone()),
                    );
                }
            }
        }

        Self {
            profile_parents,
            merchant_ids,
        }
    }

    fn classify(&self, scope_id: &str) -> ScopeClassification {
        if self.profile_parents.contains_key(scope_id) {
            ScopeClassification::Linked
        } else if self.merchant_ids.contains(scope_id) {
            ScopeClassification::Stranded
        } else {
            ScopeClassification::Unlinked
        }
    }

    fn parents_of(&self, scope_id: &str) -> Option<(&str, &str)> {
        self.profile_parents
            .get(scope_id)
            .map(|(merchant_id, org_id)| (merchant_id.as_str(), org_id.as_str()))
    }
}

/// Existing scopes whose ID is an HS merchant ID rather than a profile ID.
async fn collect_stranded(index: &TreeIndex) -> Vec<String> {
    let accounts = match MA::load_all_merchant_accounts().await {
        Ok(accounts) => accounts,
        Err(_) => return Vec::new(),
    };

    accounts
        .into_iter()
        .map(|account| crate::types::merchant::id::merchant_id_to_text(account.merchantId))
        .filter(|scope_id| index.classify(scope_id) == ScopeClassification::Stranded)
        .collect()
}

/// Rebuilds the reverse indexes from the submitted tree.
///
/// Written after the forward blobs, so an interrupted sync can leave a linked scope missing
/// from its merchant's list. Authorization reads only the forward direction and is unaffected;
/// the dashboard tree is what degrades, and re-running sync repairs it.
///
/// Every level merges rather than replaces, because a sync may carry any subtree — the
/// per-profile call from HS's profile-create path carries exactly one org, one merchant, and one
/// profile. Replacing at that granularity would drop an org's other merchants and a merchant's
/// other profiles from the index, narrowing what an org- or merchant-level grant can reach. Entries
/// that outlive the tree are harmless in the other direction: grant resolution confirms every
/// candidate against the scope's own ancestry before offering it.
async fn write_reverse_indexes(
    request: &HierarchyRequest,
) -> error_stack::Result<(), crate::generics::MeshError> {
    for org in &request.orgs {
        for merchant in &org.merchants {
            let mut profile_ids = hierarchy::load_merchant(&merchant.merchant_id)
                .await
                .map(|entry| entry.profile_ids)
                .unwrap_or_default();
            merge_ids(
                &mut profile_ids,
                merchant.profiles.iter().map(|profile| &profile.profile_id),
            );

            let entry = MerchantEntry {
                v: HIERARCHY_SHAPE_VERSION,
                merchant_name: merchant.merchant_name.clone(),
                hs_org_id: Some(org.org_id.clone()),
                profile_ids,
            };
            hierarchy::store_merchant(&merchant.merchant_id, &entry).await?;
        }

        let mut merchant_ids = hierarchy::load_org(&org.org_id)
            .await
            .map(|entry| entry.merchant_ids)
            .unwrap_or_default();
        merge_ids(
            &mut merchant_ids,
            org.merchants.iter().map(|merchant| &merchant.merchant_id),
        );

        let entry = OrgEntry {
            v: HIERARCHY_SHAPE_VERSION,
            org_name: org.org_name.clone(),
            merchant_ids,
        };
        hierarchy::store_org(&org.org_id, &entry).await?;
    }

    let mut org_ids = hierarchy::load_org_index()
        .await
        .map(|index| index.org_ids)
        .unwrap_or_default();
    merge_ids(&mut org_ids, request.orgs.iter().map(|org| &org.org_id));

    hierarchy::store_org_index(&OrgIndex {
        v: HIERARCHY_SHAPE_VERSION,
        org_ids,
    })
    .await
}

/// Appends the IDs `existing` does not already hold, preserving order so a re-sync of an unchanged
/// tree writes back exactly what it read.
///
/// Membership is tracked in a set rather than scanned per candidate. The org index is the list that
/// motivates it: it holds every org in the deployment, and a full-tree sync merges all of them into
/// it at once, so a linear scan per ID would grow quadratically in the size of the estate.
fn merge_ids<'a>(existing: &mut Vec<String>, incoming: impl Iterator<Item = &'a String>) {
    let mut seen: HashSet<String> = existing.iter().cloned().collect();
    for id in incoming {
        if seen.insert(id.clone()) {
            existing.push(id.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> HierarchyRequest {
        HierarchyRequest {
            include_all_scopes: false,
            report_stranded: true,
            orgs: vec![OrgNode {
                org_id: "org_abc".to_string(),
                org_name: Some("Acme Group".to_string()),
                merchants: vec![MerchantNode {
                    merchant_id: "merchant_id_1".to_string(),
                    merchant_name: Some("Clothing".to_string()),
                    profiles: vec![
                        ProfileNode {
                            profile_id: "pro_HJIeHSxLVWDmHsodyAYJ".to_string(),
                            profile_name: Some("Profile 1".to_string()),
                        },
                        ProfileNode {
                            profile_id: "pro_92kdlsWQ".to_string(),
                            profile_name: Some("Profile 1b".to_string()),
                        },
                    ],
                }],
            }],
        }
    }

    #[test]
    fn merging_keeps_ids_a_partial_sync_did_not_carry() {
        // The per-profile sync from HS's profile-create path carries one merchant. Replacing here
        // would drop the org's other merchants and narrow every org-level grant over it.
        let mut existing = vec!["merchant_1786625341".to_string()];
        let incoming = ["merchant_1786625390".to_string()];
        merge_ids(&mut existing, incoming.iter());

        assert_eq!(existing, ["merchant_1786625341", "merchant_1786625390"]);
    }

    #[test]
    fn merging_an_unchanged_tree_writes_back_what_it_read() {
        let mut existing = vec!["merchant_a".to_string(), "merchant_b".to_string()];
        let incoming = existing.clone();
        merge_ids(&mut existing, incoming.iter());

        assert_eq!(existing, ["merchant_a", "merchant_b"]);
    }

    #[test]
    fn request_defaults_keep_reports_small_and_safe() {
        // Absent flags: the report stays actionable, and sync still surfaces stranded rows.
        let parsed: HierarchyRequest = serde_json::from_str(r#"{"orgs":[]}"#).expect("parses");
        assert!(!parsed.include_all_scopes);
        assert!(parsed.report_stranded);
    }

    #[test]
    fn classifies_a_known_profile_as_linked() {
        let index = TreeIndex::build(&request());
        assert_eq!(
            index.classify("pro_HJIeHSxLVWDmHsodyAYJ"),
            ScopeClassification::Linked
        );
    }

    #[test]
    fn classifies_a_merchant_id_scope_as_stranded() {
        // The dangerous case: a DE scope registered with an HS merchant ID. It must not be
        // folded into Unlinked, because it holds config no profile will inherit.
        let index = TreeIndex::build(&request());
        assert_eq!(
            index.classify("merchant_id_1"),
            ScopeClassification::Stranded
        );
    }

    #[test]
    fn classifies_de_native_merchants_as_unlinked() {
        let index = TreeIndex::build(&request());
        assert_eq!(
            index.classify("merchant_a1b2c3d4e5f6"),
            ScopeClassification::Unlinked
        );
        assert_eq!(
            index.classify("standalone_test"),
            ScopeClassification::Unlinked
        );
    }

    #[test]
    fn does_not_infer_ancestry_from_the_pro_prefix() {
        // `pro_` is a convention, not a guarantee — only the submitted tree links a scope.
        let index = TreeIndex::build(&request());
        assert_eq!(
            index.classify("pro_neverSeenByHs"),
            ScopeClassification::Unlinked
        );
    }

    #[test]
    fn resolves_parents_for_linked_profiles_only() {
        let index = TreeIndex::build(&request());
        assert_eq!(
            index.parents_of("pro_92kdlsWQ"),
            Some(("merchant_id_1", "org_abc"))
        );
        assert_eq!(index.parents_of("merchant_a1b2c3d4e5f6"), None);
    }

    #[test]
    fn indexes_every_profile_in_the_tree() {
        let index = TreeIndex::build(&request());
        assert_eq!(index.profile_parents.len(), 2);
        assert_eq!(index.merchant_ids.len(), 1);
    }
}
