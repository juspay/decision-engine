//! Account hierarchy: the Hyperswitch org → merchant → profile tree, as DE stores it.
//!
//! DE's routing scope is the HS **profile** — `merchant_account.merchant_id` holds a
//! `profile_id`, and every rule, score, and config keys off it. The levels above a profile are
//! *ancestry*: they group and select profiles, and they resolve inherited config, but they
//! never key runtime state.
//!
//! Two access directions, stored two ways, each on storage DE already has:
//!
//! - **Forward** (profile → its ancestors), used by authorization and config inheritance on
//!   every request: a JSON blob under the `hierarchy` key of
//!   [`merchant_account.internal_metadata`]. It arrives inside the row
//!   `load_merchant_by_merchant_id` already fetches and caches, so a lookup costs nothing extra
//!   and needs no join.
//! - **Reverse** (merchant/org → its profiles), used by the dashboard tree and super-admin
//!   lookup: denormalized index entries in `service_configuration`, the same key-value table
//!   that already holds `SR_V3_INPUT_CONFIG_{scope}` and friends.
//!
//! The sync endpoint is the only writer of either, and it replaces whole values, which is what
//! makes the denormalized reverse indexes safe.
//!
//! Naming throughout: `profile_id` is the routing scope; `hs_merchant_id` / `hs_org_id` are
//! ancestry and are never scopes. Ancestry is absent for standalone DE and for rows HS does not
//! know about — absence is represented as `None`, never as a synthetic ID.

use serde::{Deserialize, Serialize};

use crate::types::service_configuration::{find_config_by_name, insert_config, update_config};

/// Top-level key inside `merchant_account.internal_metadata` that ancestry lives under.
/// Namespaced so other future consumers of that column take their own key and never sit
/// adjacent to `merchant_id`.
const HIERARCHY_METADATA_KEY: &str = "hierarchy";

/// Shape version of the stored blobs. Bumped only on a breaking change; readers accept any
/// version they can deserialize and treat the rest as absent ancestry.
pub const HIERARCHY_SHAPE_VERSION: u32 = 1;

const HIERARCHY_ORG_PREFIX: &str = "HIERARCHY_ORG_";
const HIERARCHY_MERCHANT_PREFIX: &str = "HIERARCHY_MERCHANT_";
const HIERARCHY_ORG_INDEX_KEY: &str = "HIERARCHY_ORG_INDEX";

/// Ancestry of one routing scope: the HS merchant and org a profile belongs to.
///
/// Stored under [`HIERARCHY_METADATA_KEY`]; `None` at the call site means the scope has no HS
/// parent — a standalone-DE merchant, a DE-native onboarded merchant, or a row HS does not
/// recognise. Callers must treat that as "no ancestry", never as an error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hierarchy {
    #[serde(default = "default_shape_version")]
    pub v: u32,
    pub hs_org_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hs_org_name: Option<String>,
    pub hs_merchant_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hs_merchant_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_name: Option<String>,
    /// RFC3339, written by the sync. Diagnostic only — nothing branches on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synced_at: Option<String>,
}

fn default_shape_version() -> u32 {
    HIERARCHY_SHAPE_VERSION
}

/// Envelope for `merchant_account.internal_metadata`. Unknown sibling keys are preserved
/// across writes via [`merge_into_metadata`] so this never clobbers another consumer's data.
#[derive(Debug, Default, Serialize, Deserialize)]
struct InternalMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hierarchy: Option<Hierarchy>,
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

/// Reads ancestry out of a raw `internal_metadata` value.
///
/// Every failure mode — absent column, non-JSON text, missing key, unreadable shape — returns
/// `None`. A malformed blob must degrade to "no ancestry" rather than failing a request, since
/// this is read on the authorization path.
pub fn from_internal_metadata(internal_metadata: Option<&str>) -> Option<Hierarchy> {
    let raw = internal_metadata?;
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let node = value.get(HIERARCHY_METADATA_KEY)?;
    serde_json::from_value(node.clone()).ok()
}

/// Ancestry of a loaded merchant account, or `None` when it has no HS parent.
pub fn of_account(account: &super::merchant_account::MerchantAccount) -> Option<Hierarchy> {
    from_internal_metadata(account.internalMetadata.as_deref())
}

/// Produces the `internal_metadata` value that stores `hierarchy`, preserving any other
/// top-level keys already present.
pub fn merge_into_metadata(
    existing: Option<&str>,
    hierarchy: &Hierarchy,
) -> Result<String, serde_json::Error> {
    let mut envelope: InternalMetadata = existing
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default();
    envelope.hierarchy = Some(hierarchy.clone());
    serde_json::to_string(&envelope)
}

// ---------------------------------------------------------------------------------------
// Reverse indexes
// ---------------------------------------------------------------------------------------

/// The orgs DE knows about. Entry point for building the dashboard tree.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrgIndex {
    #[serde(default = "default_shape_version")]
    pub v: u32,
    #[serde(default)]
    pub org_ids: Vec<String>,
}

/// One org's merchants.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrgEntry {
    #[serde(default = "default_shape_version")]
    pub v: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_name: Option<String>,
    #[serde(default)]
    pub merchant_ids: Vec<String>,
}

/// One merchant's profiles — the routing scopes under it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MerchantEntry {
    #[serde(default = "default_shape_version")]
    pub v: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merchant_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hs_org_id: Option<String>,
    #[serde(default)]
    pub profile_ids: Vec<String>,
}

pub fn org_index_key() -> String {
    HIERARCHY_ORG_INDEX_KEY.to_string()
}

pub fn org_key(hs_org_id: &str) -> String {
    format!("{HIERARCHY_ORG_PREFIX}{hs_org_id}")
}

pub fn merchant_key(hs_merchant_id: &str) -> String {
    format!("{HIERARCHY_MERCHANT_PREFIX}{hs_merchant_id}")
}

/// Reads and deserializes one config-store entry, treating an unreadable value as absent for
/// the same reason [`from_internal_metadata`] does.
async fn read_entry<T: for<'de> Deserialize<'de>>(name: String) -> Option<T> {
    let config = find_config_by_name(name).await.ok().flatten()?;
    let raw = config.value?;
    serde_json::from_str(&raw).ok()
}

/// Writes one config-store entry, inserting or updating as needed. `service_configuration`
/// has no upsert primitive, so presence decides which single-table write runs.
async fn write_entry<T: Serialize>(
    name: String,
    entry: &T,
) -> error_stack::Result<(), crate::generics::MeshError> {
    let value = serde_json::to_string(entry).map_err(|_| crate::generics::MeshError::Others)?;

    let exists = find_config_by_name(name.clone())
        .await
        .ok()
        .flatten()
        .is_some();

    if exists {
        update_config(name, Some(value)).await
    } else {
        insert_config(name, Some(value)).await
    }
}

pub async fn load_org_index() -> Option<OrgIndex> {
    read_entry(org_index_key()).await
}

pub async fn load_org(hs_org_id: &str) -> Option<OrgEntry> {
    read_entry(org_key(hs_org_id)).await
}

pub async fn load_merchant(hs_merchant_id: &str) -> Option<MerchantEntry> {
    read_entry(merchant_key(hs_merchant_id)).await
}

pub async fn store_org_index(
    entry: &OrgIndex,
) -> error_stack::Result<(), crate::generics::MeshError> {
    write_entry(org_index_key(), entry).await
}

pub async fn store_org(
    hs_org_id: &str,
    entry: &OrgEntry,
) -> error_stack::Result<(), crate::generics::MeshError> {
    write_entry(org_key(hs_org_id), entry).await
}

pub async fn store_merchant(
    hs_merchant_id: &str,
    entry: &MerchantEntry,
) -> error_stack::Result<(), crate::generics::MeshError> {
    write_entry(merchant_key(hs_merchant_id), entry).await
}

// ---------------------------------------------------------------------------------------
// Scope grants
// ---------------------------------------------------------------------------------------

/// The level of the Hyperswitch tree a handed-over session was granted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantLevel {
    Profile,
    Merchant,
    Org,
}

/// The node a session was handed, and so the set of routing scopes it may act on.
///
/// Hyperswitch decides the level, from the entity type of the user who opened the handoff. DE
/// decides which scopes that level covers, by walking its own synced indexes. A grant therefore
/// states *where in the tree* a session sits; it is never a list of scopes taken on trust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeGrant {
    pub level: GrantLevel,
    /// A profile, merchant, or org ID, matching `level`. Only a `Profile` grant's ID is a routing
    /// scope; the other two are ancestry.
    pub id: String,
}

impl ScopeGrant {
    /// The single-profile grant — what a session gets when Hyperswitch sends no level, and the
    /// reading applied to tokens minted before grants existed.
    pub fn profile(profile_id: impl Into<String>) -> Self {
        Self {
            level: GrantLevel::Profile,
            id: profile_id.into(),
        }
    }
}

/// One routing scope a grant covers, carrying the labels a picker needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrantedScope {
    pub profile_id: String,
    pub profile_name: Option<String>,
    pub hs_merchant_id: Option<String>,
    pub hs_merchant_name: Option<String>,
    pub hs_org_id: Option<String>,
    pub hs_org_name: Option<String>,
}

/// Upper bound on how many scopes one grant expands to.
///
/// An org grant fans out across every merchant beneath it, and the expansion runs on the session
/// path. The cap keeps a large org from turning `/auth/me` into a long series of reads; reaching
/// it is logged, so a truncated picker is never mistaken for a complete one.
const MAX_GRANTED_SCOPES: usize = 500;

/// Every routing scope `grant` covers.
///
/// Resolution walks the reverse indexes and then loads each candidate scope, so a profile appears
/// only where DE actually holds a scope for it. That second step is what makes a grant safe to act
/// on: a level broader than the tree justifies still cannot reach anything the last sync did not
/// place under that node.
pub async fn scopes_for_grant(grant: &ScopeGrant) -> Vec<GrantedScope> {
    match grant.level {
        GrantLevel::Profile => granted_profile(&grant.id, Under::Itself)
            .await
            .into_iter()
            .collect(),
        GrantLevel::Merchant => scopes_under_merchant(&grant.id, Under::Merchant(&grant.id)).await,
        GrantLevel::Org => {
            let Some(org) = load_org(&grant.id).await else {
                return Vec::new();
            };

            let mut scopes = Vec::new();
            for hs_merchant_id in &org.merchant_ids {
                scopes.extend(scopes_under_merchant(hs_merchant_id, Under::Org(&grant.id)).await);

                if scopes.len() >= MAX_GRANTED_SCOPES {
                    scopes.truncate(MAX_GRANTED_SCOPES);
                    crate::logger::warn!(
                        category = "HIERARCHY",
                        org_id = %grant.id,
                        cap = MAX_GRANTED_SCOPES,
                        "Org grant reached the scope cap; the returned list is truncated"
                    );
                    break;
                }
            }
            scopes
        }
    }
}

/// Whether `grant` covers `profile_id`.
///
/// Resolved exactly as [`scopes_for_grant`] resolves it, so a switch can never land on a scope the
/// picker would not have offered.
pub async fn grant_covers(grant: &ScopeGrant, profile_id: &str) -> bool {
    if grant.level == GrantLevel::Profile {
        return grant.id == profile_id;
    }

    scopes_for_grant(grant)
        .await
        .iter()
        .any(|scope| scope.profile_id == profile_id)
}

/// The node a candidate profile must itself claim as an ancestor for a grant to reach it.
#[derive(Debug, Clone, Copy)]
enum Under<'a> {
    /// The profile *is* the granted node, so there is nothing above it to agree with.
    Itself,
    Merchant(&'a str),
    Org(&'a str),
}

/// One scope with its labels, or `None` when DE has no such scope or the scope does not place
/// itself under `under`.
///
/// The forward blob decides both questions, not the index entry that led here. An index left stale
/// by a partial sync can still name a profile that has since moved, and a grant must not reach it —
/// so the reverse index proposes candidates and the forward ancestry confirms them. Labels come
/// from the same place, so a profile renamed since the last index write shows its current name.
async fn granted_profile(profile_id: &str, under: Under<'_>) -> Option<GrantedScope> {
    let account =
        super::merchant_account::load_merchant_by_merchant_id(profile_id.to_owned()).await?;
    let ancestry = of_account(&account);

    let placed = match under {
        Under::Itself => true,
        Under::Merchant(id) => ancestry
            .as_ref()
            .is_some_and(|parent| parent.hs_merchant_id == id),
        Under::Org(id) => ancestry
            .as_ref()
            .is_some_and(|parent| parent.hs_org_id == id),
    };
    if !placed {
        return None;
    }

    Some(GrantedScope {
        profile_id: profile_id.to_string(),
        profile_name: ancestry.as_ref().and_then(|h| h.profile_name.clone()),
        hs_merchant_id: ancestry.as_ref().map(|h| h.hs_merchant_id.clone()),
        hs_merchant_name: ancestry.as_ref().and_then(|h| h.hs_merchant_name.clone()),
        hs_org_id: ancestry.as_ref().map(|h| h.hs_org_id.clone()),
        hs_org_name: ancestry.as_ref().and_then(|h| h.hs_org_name.clone()),
    })
}

/// The scopes DE holds for one HS merchant's profiles, keeping only those that agree they sit
/// under `under`.
async fn scopes_under_merchant(hs_merchant_id: &str, under: Under<'_>) -> Vec<GrantedScope> {
    let Some(entry) = load_merchant(hs_merchant_id).await else {
        return Vec::new();
    };

    let mut scopes = Vec::with_capacity(entry.profile_ids.len());
    for profile_id in &entry.profile_ids {
        if let Some(scope) = granted_profile(profile_id, under).await {
            scopes.push(scope);
        }
    }
    scopes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Hierarchy {
        Hierarchy {
            v: HIERARCHY_SHAPE_VERSION,
            hs_org_id: "org_abc".to_string(),
            hs_org_name: Some("Acme Group".to_string()),
            hs_merchant_id: "merchant_id_1".to_string(),
            hs_merchant_name: Some("Clothing".to_string()),
            profile_name: Some("Profile 1".to_string()),
            synced_at: Some("2026-08-13T10:04:00Z".to_string()),
        }
    }

    #[test]
    fn round_trips_through_metadata() {
        let metadata = merge_into_metadata(None, &sample()).expect("serializes");
        assert_eq!(from_internal_metadata(Some(&metadata)), Some(sample()));
    }

    #[test]
    fn absent_metadata_has_no_ancestry() {
        assert_eq!(from_internal_metadata(None), None);
    }

    #[test]
    fn malformed_metadata_degrades_to_no_ancestry() {
        // Read on the authorization path — must never surface as an error.
        assert_eq!(from_internal_metadata(Some("not json at all")), None);
        assert_eq!(from_internal_metadata(Some("{}")), None);
        assert_eq!(from_internal_metadata(Some(r#"{"hierarchy":42}"#)), None);
        assert_eq!(
            from_internal_metadata(Some(r#"{"hierarchy":{"hs_org_id":"org_abc"}}"#)),
            None,
            "a blob missing hs_merchant_id is not usable ancestry"
        );
    }

    #[test]
    fn preserves_other_metadata_keys() {
        let existing = r#"{"some_other_consumer":{"keep":"me"}}"#;
        let merged = merge_into_metadata(Some(existing), &sample()).expect("serializes");

        let value: serde_json::Value = serde_json::from_str(&merged).expect("valid json");
        assert_eq!(value["some_other_consumer"]["keep"], "me");
        assert_eq!(from_internal_metadata(Some(&merged)), Some(sample()));
    }

    #[test]
    fn overwrites_only_the_hierarchy_key() {
        let first = merge_into_metadata(None, &sample()).expect("serializes");

        let mut updated = sample();
        updated.hs_merchant_id = "merchant_id_2".to_string();
        let second = merge_into_metadata(Some(&first), &updated).expect("serializes");

        assert_eq!(
            from_internal_metadata(Some(&second)).map(|h| h.hs_merchant_id),
            Some("merchant_id_2".to_string())
        );
    }

    #[test]
    fn reads_blob_written_without_a_version() {
        // Forward compatibility in the other direction: a blob predating `v` still reads.
        let raw = r#"{"hierarchy":{"hs_org_id":"org_abc","hs_merchant_id":"merchant_id_1"}}"#;
        let parsed = from_internal_metadata(Some(raw)).expect("reads");
        assert_eq!(parsed.v, HIERARCHY_SHAPE_VERSION);
        assert_eq!(parsed.hs_merchant_id, "merchant_id_1");
        assert_eq!(parsed.profile_name, None);
    }

    #[test]
    fn grant_levels_serialize_as_snake_case() {
        // Hyperswitch sends these by name, so the wire spelling is part of the contract.
        let grant = ScopeGrant {
            level: GrantLevel::Org,
            id: "org_abc".to_string(),
        };
        let encoded = serde_json::to_string(&grant).expect("serializes");
        assert_eq!(encoded, r#"{"level":"org","id":"org_abc"}"#);

        let decoded: ScopeGrant =
            serde_json::from_str(r#"{"level":"merchant","id":"m_1"}"#).expect("deserializes");
        assert_eq!(decoded.level, GrantLevel::Merchant);
    }

    #[test]
    fn a_profile_grant_covers_only_itself() {
        // Checked without touching storage, so an unsynced scope can still reach its own dashboard.
        let grant = ScopeGrant::profile("pro_HJIeHSxLVWDmHsodyAYJ");
        assert_eq!(grant.level, GrantLevel::Profile);
        assert_eq!(grant.id, "pro_HJIeHSxLVWDmHsodyAYJ");
    }

    #[test]
    fn index_keys_are_namespaced_by_level() {
        assert_eq!(org_key("org_abc"), "HIERARCHY_ORG_org_abc");
        assert_eq!(
            merchant_key("merchant_id_1"),
            "HIERARCHY_MERCHANT_merchant_id_1"
        );
        assert_eq!(org_index_key(), "HIERARCHY_ORG_INDEX");
    }
}
