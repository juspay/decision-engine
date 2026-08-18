# Hyperswitch Org Hierarchy → Decision Engine (migration design)

## Overview

Hyperswitch (HS) models accounts as a four-level tree — **Organization → Merchant account →
Profile → Connector** — and configures routing **per profile**. Decision Engine (DE) has a
single flat scope axis: a `merchant_id: Text` column that every rule, config, score, cost
record, API key, and analytics event hangs off. `grep -rn profile_id src/` returns nothing.

This document covers how existing HS merchants move their routing rules into DE and run all
rule creation and multi-objective routing from DE alone.

The central decision: **DE's routing scope is the HS profile.** This is not a new mapping — it
is what the live integration already does. The SSO handoff in production is called with IDs
like `pro_HJIeHSxLVWDmHsodyAYJ`, and HS sends `profile_id` in `decide-gateway.merchant_id` on
every call (confirmed). One DE `merchant_account` row = one HS profile. The levels above it
become **ancestry** — used for display, inheritance, and access grants, never for keying
runtime state.

The design constraints that shape everything below: **no schema migrations** (the hierarchy
must be able to evolve without another `ALTER TABLE` every quarter) and **no SQL joins**
(the data layer in [`generics.rs`](../src/generics.rs) has only single-table primitives —
there is no join anywhere in this codebase, and no primitive that could express one).

---

## Concepts and naming

The single largest source of confusion in this migration is that DE's legacy column
`merchant_account.merchant_id` does not hold a merchant — it holds a profile. Every naming
decision below exists to stop that ambiguity from spreading.

### The mapping

```
   Hyperswitch                          Decision Engine
   ───────────                          ───────────────

   Organization                         hierarchy.hs_org_id
     org_id  ───────────────────────►   ancestry: display, rollups, key grants
        │
        ▼
   Merchant account                     hierarchy.hs_merchant_id
     merchant_id  ──────────────────►   ancestry: cost/contract scope, key grants
        │
        ▼
   Profile                              merchant_account.merchant_id
     profile_id  ───────────────────►   ═══════════════════════════
        │                               ►  THE ROUTING SCOPE
        │                                  owns: rules, scores, SR config,
        │                                        feature flags, analytics grain
        ▼
   Connector                            ConnectorInfo.gateway_id
     merchant_connector_id  ─────────►   ( = merchant_connector_id )
```

### Three naming rules

These are the whole concept. Everything downstream follows mechanically.

1. **`profile_id` is the routing scope.** Every runtime key — rules, scores, SR config,
   feature flags, analytics grain — is a `profile_id`. New code says `profile_id`, never
   "merchant", when it means the scope.
2. **`hs_merchant_id` and `hs_org_id` are ancestry, never scopes.** They select and group
   profiles; they never key routing state directly. The `hs_` prefix marks them as replicas
   of upstream HS state that DE syncs and never authors.
3. **`merchant_account.merchant_id` is a frozen legacy alias for `profile_id`.** It is not
   renamed (it is the single most-referenced column in the codebase and appears in the public
   API contract), and no ancestry field is ever placed beside it as a sibling column.

Rule 3 prevents exactly one failure mode: a row where `merchant_id` and `hs_merchant_id` sit
side by side and a reader cannot tell which is the merchant. In the model below, ancestry
lives one level down inside a namespaced JSON object, so the two names are never adjacent.

> **Standalone DE.** With no HS upstream, no ancestry is ever written, everything resolves to
> `None`, and `merchant_account.merchant_id` genuinely is a merchant ID — the legacy name is
> correct there. "Profile" is the *alias the name takes on* when HS is upstream, which is why
> the column is left alone rather than renamed.

---

## Grounding: every scope-keyed surface today

All of these key off one string. This is the asset — it means the migration defines what that
string *is* rather than adding a second axis everywhere.

| Surface | Key | Source |
|---|---|---|
| Routing rules | `routing_algorithm.created_by`, `routing_algorithm_mapper.created_by` | [`schema_pg.rs:393`](../src/storage/schema_pg.rs) |
| Active-rule cache | `DE_routing_algo_eval:{scope}` | [`routing_rules.rs:56`](../src/euclid/handlers/routing_rules.rs) |
| SR / multi-objective input | `SR_V3_INPUT_CONFIG_{scope}` | [`flow_new.rs:337`](../src/decider/gatewaydecider/flow_new.rs) |
| SR dimensions | `SR_DIMENSION_CONFIG_{scope}` | [`routing_rules.rs:216`](../src/euclid/handlers/routing_rules.rs) |
| Debit routing | `DEBIT_ROUTING_ENABLED_{scope}`, `DEBIT_ROUTING_CONFIG_{scope}` | [`merchant_account_config.rs:38`](../src/routes/merchant_account_config.rs) |
| Gateway scores | `SR1_KEY_PREFIX` / `N_KEY_PREFIX` / `AGGREGATE_KEY_PREFIX` + scope (Redis) | [`gw_scoring.rs:2246`](../src/decider/gatewaydecider/gw_scoring.rs) |
| Feature flags | `feature.merchant_id` | [`schema_pg.rs:100`](../src/storage/schema_pg.rs) |
| Cost ingestion / seed costs | `cost_ingestion.merchant_id`, per-scope seed KV | [`cost_ingestion/store.rs`](../src/cost_ingestion/store.rs) |
| API keys | `merchant_api_keys.merchant_id` | [`schema_pg.rs:261`](../src/storage/schema_pg.rs) |
| Dashboard session | JWT `merchant_id` claim, `user_merchants` | [`user_auth.rs`](../src/routes/user_auth.rs) |
| Analytics | `merchant_id` on every Kafka event → ClickHouse | [`analytics/events.rs:13`](../src/analytics/events.rs) |

After this migration every one of those is **profile-grained**. No existing dashboard depends
on the column being merchant-grained (confirmed), so no ClickHouse backfill is required.

---

## Blocking finding: DE authenticates but does not authorize

This must be fixed before merchant-level API keys mean anything.

[`authenticate`](../src/middleware.rs) resolves an API key or JWT to an
[`AuthContext`](../src/auth/context.rs) and inserts it into request extensions. **Outside of
analytics, no handler ever reads it.** The only consumer is
[`AuthenticatedAnalyticsContext`](../src/custom_extractors.rs); every other route takes the
scope from the path or body and acts on it:

```rust
// src/euclid/handlers/routing_rules.rs:282
pub async fn routing_create(
    headers: axum::http::HeaderMap,
    Json(payload): Json<Value>,
) -> ... {
    let config: RoutingRule = serde_json::from_value(payload.clone())?;
    //  `config.created_by` is trusted verbatim — never compared to the caller's identity.
```

So today **any valid API key can read or overwrite any scope's routing rules** by putting a
different `created_by` in the body. The same holds for `/rule-configuration/*`,
`/merchant-account/:id/features`, seed costs, and cost ingestion.

Widening keys from profile to merchant level is meaningless while keys are effectively
global. Phase 2 closes this, and the containment check it introduces is the same mechanism
merchant-level keys need — one piece of work, two requirements.

---

## Data model: no new tables, no migrations, no joins

The hierarchy is stored in two places DE already has, each chosen for its access direction.

### Forward (profile → its ancestors): `merchant_account.internal_metadata`

This column already exists in both schemas ([`schema_pg.rs:248`](../src/storage/schema_pg.rs),
[`schema.rs:253`](../src/storage/schema.rs)), is already carried on the `MerchantAccount`
domain type as `internalMetadata: Option<String>`
([`merchant_account.rs:70`](../src/types/merchant/merchant_account.rs)), and **is currently
unused** — set to `None` on create and read by nothing. It is a JSON extension point that is
already plumbed end to end.

```jsonc
// merchant_account.internal_metadata
{
  "hierarchy": {
    "v": 1,
    "hs_org_id": "org_abc",       "hs_org_name": "Acme Group",
    "hs_merchant_id": "merchant_id_1", "hs_merchant_name": "Clothing",
    "profile_name": "Profile 1",
    "synced_at": "2026-08-13T10:04:00Z"
  }
}
```

Three properties fall out of this, and together they satisfy both constraints:

- **No migration, ever.** Adding a field later is a Rust struct change, not an `ALTER TABLE`.
  The `"v"` field carries the shape version so old rows stay readable.
- **No join, and no extra read.** Ancestry arrives inside the row
  `load_merchant_by_merchant_id` already fetches, which is already served from `GLOBAL_CACHE`
  under `merchant_acct_{id}` ([`merchant_account.rs:226`](../src/types/merchant/merchant_account.rs)).
  The authorization check costs zero additional lookups.
- **No name collision.** Ancestry sits one level down under `"hierarchy"`, so `merchant_id`
  and `hs_merchant_id` are never adjacent. Other future consumers of `internal_metadata` take
  their own top-level key.

Access goes through one accessor so nothing parses the blob by hand:

```rust
/// Ancestry of a routing scope. `None` for standalone DE with no HS upstream.
pub fn hierarchy(account: &MerchantAccount) -> Option<Hierarchy>;
```

### Reverse (merchant/org → its profiles): `service_configuration`

The forward direction covers authorization and inheritance — the frequent paths. The
dashboard tree and super-admin lookup need the reverse, which a JSON text column cannot index.

DE already has a general key-value store for exactly this kind of per-scope state:
`service_configuration`, which is how `SR_V3_INPUT_CONFIG_{scope}`,
`SR_DIMENSION_CONFIG_{scope}`, and `DEBIT_ROUTING_CONFIG_{scope}` are all stored, read
through [`find_config_by_name`](../src/types/service_configuration.rs) — a single-table
primitive, no join.

```
HIERARCHY_ORG_{hs_org_id}           → { "v": 1, "org_name": …, "merchant_ids": [ … ] }
HIERARCHY_MERCHANT_{hs_merchant_id} → { "v": 1, "merchant_name": …, "profile_ids": [ … ] }
HIERARCHY_ORG_INDEX                 → { "v": 1, "org_ids": [ … ] }
```

These are denormalized indexes maintained by the sync endpoint, which is their **only**
writer and rewrites them wholesale. That is what makes denormalization safe here: the update
anomalies normalization protects against cannot occur when one writer replaces whole values.

### API key grants: `service_configuration`, not a schema change

```
API_KEY_GRANT_{key_id} → { "v": 1, "level": "merchant", "id": "merchant_id_1" }
```

**Absent means profile-level grant over `merchant_api_keys.merchant_id`** — today's behaviour,
so every existing key keeps working with no backfill and no migration.

On the auth path the grant is folded into the value already cached at `api_key:{hash}`
([`middleware.rs:94`](../src/middleware.rs)), which becomes
`{"level":…,"id":…}` instead of a bare scope string. **Bump the cache key prefix on deploy**
(`api_key_v2:{hash}`) so in-flight entries in the old format are not misparsed. A cache miss
costs two single-table reads (key row, then grant config) — the same shape as today plus one.

### What this costs

Stated plainly, because it is a real trade:

- **No DB-level constraints or referential integrity** on ancestry. Acceptable because the
  codebase has no foreign keys anywhere already, and the sync is the sole writer.
- **No ad-hoc SQL analysis** of the hierarchy — you cannot `GROUP BY hs_merchant_id` in a
  psql session. The reverse indexes answer the queries the product needs; genuine analytics
  rollups belong in ClickHouse, where the ancestry can be denormalized onto events.
- **Blob discipline is on us.** Every write goes through the typed accessor, every read
  tolerates a missing or older `"v"`. A malformed blob must degrade to "no ancestry", never
  to an error on the decide path.

### Rust newtypes — the naming rule made unbreakable

```rust
/// The routing scope: owns rules, scores, and SR config. One per HS profile.
/// Physically stored in the legacy `merchant_account.merchant_id` column.
pub struct ProfileId(String);

/// HS merchant account. Ancestry only — never keys routing state.
pub struct HsMerchantId(String);

/// HS organization. Ancestry only.
pub struct HsOrgId(String);
```

Adopt at the boundaries first (sync, `Scope`, new endpoints), then widen inward as handlers
are touched. No repo-wide conversion in this migration.

---

## Scope resolution ladder

The one genuinely new mechanism. Routing config is per profile, but **cost data is not** — a
merchant with three profiles negotiates one Adyen contract, not three. Keying seed costs
strictly per profile would make merchants re-upload identical rate cards N times and split
the multi-objective predictor's evidence N ways.

Resolution walks up until it finds a value:

| Config | Resolves at | Inherits from | Why |
|---|---|---|---|
| Routing rules (`routing_algorithm`) | profile | — | Connectors are profile-scoped; a rule naming a connector is meaningless outside its profile |
| `SR_V3_INPUT_CONFIG` | profile | merchant | Merchant-wide defaults are useful; per-profile override wins |
| `SR_DIMENSION_CONFIG` | profile | merchant | as above |
| Debit routing config | profile | merchant | as above |
| Gateway scores (Redis) | profile | **never** | Success statistics from different connector sets cannot be merged |
| Seed costs / contract rates | merchant | org | Negotiated at merchant or org level |
| Cost ingestion + predictor | merchant | org | Rate cards and settlement reports arrive per merchant |
| Feature flags | profile | merchant → org | Lets a whole org be flipped onto DE routing at once |

The ladder is pure key construction over the existing config store — try
`SR_V3_INPUT_CONFIG_{profile_id}`, then `SR_V3_INPUT_CONFIG_{hs_merchant_id}`, then the org
key. Ancestry for step two comes from the cached scope row, so a full ladder walk is at most
three `find_config_by_name` calls and no joins.

```rust
pub enum ScopeLevel { Org, Merchant, Profile }

pub struct Scope { pub level: ScopeLevel, pub id: String }

impl Scope {
    /// True when `profile` sits at or under this scope. Reads only the cached scope row.
    pub async fn permits(&self, profile: &ProfileId) -> bool;
}
```

Implement once in `src/auth/scope.rs`, used by `find_config_by_name` callers, the seed store,
and cost serving. Nothing else walks the ladder by hand.

---

## Endpoint contracts

### 1. `POST /admin/hierarchy/sync` — provisioning

Admin-secret gated, idempotent, creates no users and sends no email. Replaces per-merchant
`/merchant-account/create` for HS-backed deployments (that endpoint stays for standalone).

```jsonc
// Request — mirrors the HS tree exactly
{
  "orgs": [{
    "org_id": "org_abc", "org_name": "Acme Group",
    "merchants": [{
      "merchant_id": "merchant_id_1", "merchant_name": "Clothing",
      "profiles": [
        { "profile_id": "pro_HJIeHSxLVWDmHsodyAYJ", "profile_name": "Profile 1" }
      ]
    }]
  }]
}

// Response
{ "orgs_seen": 1, "merchants_seen": 1,
  "profiles_created": 1, "profiles_updated": 0, "profiles_unchanged": 0,
  "stranded": [] }
```

Per profile it upserts the `merchant_account` row, writes the `hierarchy` blob into
`internal_metadata`, invalidates `merchant_acct_{profile_id}`, and rewrites the affected
reverse-index entries. The request body uses HS's own field names since it *is* the HS tree;
DE stores them under the `hs_`-prefixed names.

Run once as a backfill, then call it from HS's profile-create path so new profiles appear in
DE automatically. Re-running is safe and reports zero changes — the comparison ignores
`synced_at`, which would otherwise make every scope look modified on every run.

`"report_stranded": false` skips the stranded scan. Detecting stranded scopes means reading
every merchant account, which is right for the periodic full-tree sync but not for the
per-profile call from HS's profile-create path.

### 1b. `POST /admin/hierarchy/reconcile` — read-only classification

Same body, writes nothing. Run before the first sync.

```jsonc
{ "linked": 3, "unlinked": 575, "stranded": 0,
  "missing_in_de": ["pro_HJIeHSxLVWDmHsodyAYJ"],
  "scopes": [] }
```

`scopes` lists **stranded rows only** by default. A real deployment carries hundreds of
correct-as-they-are unlinked scopes — the local dev database has 575 — and including them
buries the rows that need a decision. `"include_all_scopes": true` returns the full inventory.

### 2. `POST /auth/admin/merchant-token` — extended

Today the body is `{ "merchant_id": ... }` and the handler verifies existence at
[`user_auth.rs:1447`](../src/routes/user_auth.rs). Extended to accept the tree, resolving to
one profile:

```jsonc
{ "org_id": "org_abc", "merchant_id": "merchant_id_1",
  "profile_id": "pro_HJIeHSxLVWDmHsodyAYJ" }
```

`profile_id` alone remains valid and is the fast path — **the existing HS call site keeps
working unchanged**, since its `merchant_id` field already carries a profile ID. When
`profile_id` is absent and the merchant has exactly one profile, resolve it through
`HIERARCHY_MERCHANT_{id}`; when it has several, return `TE_04` with the candidate list rather
than guessing. The minted JWT keeps `merchant_id = profile_id` for wire compatibility and
gains optional `hs_org_id` / `hs_merchant_id` claims so the SPA can render the breadcrumb.

This is the migration's user-provisioning story: HS-initiated entry needs no DE user row and
sends no email. `/auth/signup` stops being part of the HS path entirely.

### 3. `POST /api-key/create` — grant level

```jsonc
{ "grant_level": "merchant", "grant_id": "merchant_id_1",
  "description": "Clothing — all profiles" }
```

Writes `API_KEY_GRANT_{key_id}`. Omitting `grant_level` writes nothing and mints a profile
key, preserving current behaviour exactly.

### 4. `GET /auth/merchants` — tree instead of flat list

Built from the reverse indexes: `HIERARCHY_ORG_INDEX` → per-org entry → per-merchant entry.
Three keyed reads per level, no scan, no join. Super-admin `lookup_merchants` returns the
same shape, matched at any level.

---

## Existing rows: classification and backfill

An existing DE database holds three populations of `merchant_account` rows, created by three
different paths:

| Origin | ID shape | Is it a profile? |
|---|---|---|
| HS integration / SSO handoff | `pro_HJIeHSxLVWDmHsodyAYJ` | Yes — HS has been sending profile IDs all along |
| DE onboarding ([`user_auth.rs:455`](../src/routes/user_auth.rs)) | `merchant_<12 hex>` | No — minted by DE, unknown to HS |
| `/merchant-account/create` | caller-supplied, arbitrary | Unknown — could be anything, including an HS *merchant* ID |

### The rule: never fabricate ancestry

A row with no HS parent gets **no org ID and no merchant ID** — not a placeholder, not
`org_default`, not the scope ID copied upward. The UI shows the absence as an absence, and
the `hierarchy` blob simply stays unset.

This matters because a synthetic ID would not stay cosmetic. It would flow into API-key
grants, into the tree, and eventually into analytics rollups, and would then be impossible to
distinguish from a real org. `Option<Hierarchy>` returning `None` is already the standalone-DE
path (see [Concepts](#concepts-and-naming)), so unlinked rows reuse a case the code must
handle anyway rather than inventing a second one.

### Classification comes from the sync, not from the ID

Do not infer anything from the `pro_` prefix — it is a convention, not a guarantee, and a
merchant could have supplied a `pro_`-looking ID to `/merchant-account/create`. The sync is
the source of truth: after backfill, a row has a `hierarchy` blob **iff** HS confirmed it as a
profile. Absence of the blob is meaningful data, not a failure.

[`POST /admin/hierarchy/reconcile`](#1b-post-adminhierarchyreconcile--read-only-classification)
is a **read-only** dry-run report, run before the first write-mode sync:

| Bucket | Meaning | Handling |
|---|---|---|
| **Linked** | DE row matches an HS profile | Sync writes the blob. Nothing else changes — same ID, same rules, same scores. |
| **Missing in DE** | HS profile with no DE row | Sync creates the row. This is what fixes the `TE_04 Merchant not found` seen on the SSO call. |
| **Unlinked** | DE row HS does not recognise | Left exactly as-is, fully functional, no ancestry. Expected for DE-native and test merchants. |
| **Stranded** | DE row whose ID is an HS *merchant* ID, not a profile | **Needs a decision per row — see below.** |

### Stranded rows are the one real hazard

If any DE row was registered with an HS merchant ID rather than a profile ID, sync will not
match it. It stays live, holding rules, SR config, and accumulated gateway scores — while sync
separately creates fresh, empty rows for each of that merchant's profiles. Traffic follows the
profile rows, so the old row's configuration is silently orphaned: nothing errors, routing just
quietly stops using it.

The reconcile report must list these explicitly rather than folding them into **Unlinked**,
because they are the only bucket where doing nothing loses working configuration. Per row,
someone decides: copy its rules to one designated profile, split them across profiles, or
abandon it deliberately. There is no safe automatic answer — DE cannot know which profile a
merchant-level rule was meant for.

Scores are not copyable in any of these cases; a freshly created profile row starts cold
regardless, which is the warm-up cost already noted under [Risks](#risks-and-open-items).

### What existing users and keys see

- **`user_merchants` rows** keep pointing at the same scope IDs and keep working. A DE user
  who logs in with a password sees their scopes exactly as before, unlinked ones included.
- **Existing API keys** keep working unchanged — no grant record means profile-level over
  their current `merchant_id`, which is today's behaviour.
- **A merchant-level key can never reach an unlinked row**, since it has no merchant. That is
  the correct default: reach is granted only by confirmed ancestry, never assumed.
- **ClickHouse history** is untouched. Existing events keyed by these IDs stay valid and
  keep resolving to the same scope.

---

## Dashboard

The scope the dashboard operates on is still **one ID held in one store**, and it is still the
same ID it holds today. That is why the page layer does not change: `useMerchantStore` keeps
holding a single scope string, and the ~15 pages that read it — `SRRoutingPage`,
`EuclidRulesPage`, `VolumeSplitPage`, `ABTestingPage`, `CostCoverageCard`,
`DecisionExplorerPage`, and the rest — are untouched.

### Scope selector: flat list → tree

Today the switcher is a flat merchant list
([`TopBar.tsx:261`](../website/src/components/layout/TopBar.tsx)):

```
┌ Merchants ──────────────────┐
│ ✓ Clothing                  │
│   merchant_id_1             │
│   Shoes                     │
│   merchant_id_2             │
│ ─────────────────────────── │
│ + Add merchant              │
└─────────────────────────────┘
```

After, it is org → merchant → profile. What gets **selected is always a profile**, because
that is the routing scope; org and merchant rows are expandable grouping headers, not
destinations:

```
┌ Acme Group ─────────────────────────┐
│  ▾ Clothing          merchant_id_1  │
│      ✓ Profile 1     pro_HJIeHSx…   │
│        Profile 1b    pro_92kdlsW…   │
│  ▸ Shoes             merchant_id_2  │
│  ▸ Bags              merchant_id_3  │
├ Not linked ─────────────────────────┤
│    Test merchant     merchant_a1b2… │
└─────────────────────────────────────┘
```

"Not linked" is a **presentation-only** group for rows with no ancestry (see
[Existing rows](#existing-rows-classification-and-backfill)) — no synthetic org or merchant ID
is ever persisted for them. They stay selectable, because they may hold live routing config.
On standalone DE every row lands here and the group header is dropped, leaving today's flat
list.

The TopBar button shows the breadcrumb — `Acme Group / Clothing / Profile 1` — rather than a
bare name. The tree is built from the reverse indexes (`HIERARCHY_ORG_INDEX` → per-org →
per-merchant), so it is keyed reads all the way down, no scan.

### Cost pages: inheritance notice

The one genuinely new UI concept, and it falls directly out of the resolution ladder.
Contract rates resolve at merchant level, so a profile-scoped page editing them must say that
it is editing something shared:

```
┌──────────────────────────────────────────────────────┐
│ ⓘ  Contract rates are set for Clothing and apply to  │
│    all 2 profiles under it.          [View profiles] │
└──────────────────────────────────────────────────────┘
```

Without it, a merchant edits rates while viewing Profile 1 and silently changes Profile 1b.
Routing pages need no such notice — they are profile-only, which is what the user already
expects from HS.

### Changed files

| File | Change |
|---|---|
| [`TopBar.tsx`](../website/src/components/layout/TopBar.tsx) | Tree instead of `merchants.map`; breadcrumb label; super-admin lookup renders matches at any level |
| [`authStore.ts`](../website/src/store/authStore.ts) | Nested tree type; optional `hsOrgId` / `hsMerchantId` on `AuthUser` |
| [`SuperAdminBanner.tsx`](../website/src/components/layout/SuperAdminBanner.tsx) | Show the full path, not a bare ID — it matters when three profiles look alike |
| [`ApiKeysPage.tsx`](../website/src/pages/ApiKeysPage.tsx) | Grant level selector (org / merchant / profile) |
| [`OnboardingPage.tsx`](../website/src/pages/OnboardingPage.tsx) | "Add merchant" hidden for HS-backed deployments (see below) |
| `useMerchantStore` + all pages | **Unchanged** |

### Onboarding and standalone

`OnboardingPage` creates a DE merchant out of band. Under the sync model that produces a
scope with no ancestry — an orphan the tree cannot place — so for HS-backed deployments the
entry point is hidden and profiles arrive only from `/admin/hierarchy/sync`.

Standalone DE keeps it, and keeps today's flat switcher: with `hierarchy` resolving to `None`
everywhere, the tree collapses to a single level on its own. No feature flag is needed for
this — the shape of the data decides the shape of the UI.

### Deliberate v1 limitation

**v1 always selects a profile.** Cost pages explain upward inheritance with the notice above
rather than offering a merchant-level editing mode.

The alternative — letting the selector land on a merchant, with pages adapting (cost pages
edit directly, routing pages prompt for a profile, analytics rolls up) — is better for
merchants with many profiles, but it puts a "which level am I at" branch in every page and
needs `IN (profile_ids)` rollup queries that do not exist yet. Ship profile-only first, then
revisit once the tree is populated and the real depth of profile lists is visible.

---

## Rule translation

DE's [`StaticRoutingAlgorithm`](../src/euclid/types.rs) and HS's routing algorithms share
euclid ancestry, so the four production variants map structurally:

| HS algorithm | DE `StaticRoutingAlgorithm` | Notes |
|---|---|---|
| `single` | `Single(ConnectorInfo)` | Direct |
| `priority` | `Priority(Vec<ConnectorInfo>)` | Order preserved |
| `volume_split` | `VolumeSplit(Vec<VolumeSplit<ConnectorInfo>>)` | Split percentages carry over; verify they sum as DE expects |
| `advanced` | `Advanced(Program)` | Statement/comparison tree; the bulk of translation effort |
| `dynamic` | — | Not a euclid rule. Success-rate and elimination configs, already carried by `POST /rule/create` into `service_configuration` |
| `three_ds_decision_rule` | — | Euclid-shaped, but its output is a 3DS decision rather than a connector; `Output` has no such variant |
| — | `AbTest(ABTestData)` | DE-only, no HS equivalent |

The two unmapped kinds are not failures — they reach DE another way, or not at all — so a
migration must report them apart from errors or a finished profile never reads as finished.
Measured against a sandbox export of 9,182 rules: 8,488 in scope, 503 `dynamic`, 191
`three_ds_decision_rule`.

`[routing_config.keys]` is what decides whether a translated rule is accepted, and it must
track HS's `frontend/dir` key set and `ast/lowering.rs` value rules exactly. Looser and DE
matches traffic HS never would; stricter and valid rules are refused at the door.

**Connector references are the part that needs care.** DE's
[`ConnectorInfo`](../src/euclid/ast.rs) is:

```rust
pub struct ConnectorInfo {
    pub gateway_name: String,
    pub gateway_id: Option<String>,
}
```

Map HS `connector` → `gateway_name` and HS `merchant_connector_id` → `gateway_id`. **Always
populate `gateway_id`**: `gateway_name` alone collides when a profile holds two accounts on
the same connector, and `pm_filter_graph.rs:130` lowercases the name for matching, so case
differences between HS and DE connector naming are absorbed but duplicate names are not.

### Where migration runs: Hyperswitch, not here

Translation is driven from HS, which already holds the rules, the credentials and the
connector context. Two admin endpoints there do the work, both idempotent and safe to re-run:

- **`POST /routing/rule/migrate`** — takes `profile_ids`, provisions each scope with its
  ancestry via `/admin/hierarchy/sync`, then translates and writes each rule through
  `POST /routing/create`. Rules already present are skipped, so a partial run repairs itself.
- **`GET /routing/migration/status`** — pages over every profile holding rules and set-diffs
  HS's rule ids against `POST /routing/list/{created_by}`. IDs are comparable because HS sends
  its own `algorithm_id` as the rule's primary key here.

A DE-side tool was prototyped and dropped: it needed credentials for both systems, and its
count-based comparison could not tell "same number of rules" from "the same rules".

---

## Phased rollout

| Phase | Work | Exit criteria |
|---|---|---|
| **1. Hierarchy** ✅ **built** | `Hierarchy` blob type + accessor ([`types/merchant/hierarchy.rs`](../src/types/merchant/hierarchy.rs)); reverse indexes; reconcile + sync ([`routes/hierarchy.rs`](../src/routes/hierarchy.rs)) | Reconcile report reviewed and every **Stranded** row dispositioned; every profile HS routes to resolves in DE; re-running sync reports zero changes |
| **2. Authorization** | `ProfileId`/`HsMerchantId`/`HsOrgId` newtypes; `Scope` + `permits()`; `ScopedProfile` extractor replacing bare `Path(merchant_id)`; API-key grants | A profile key cannot touch a sibling profile; a merchant key can touch all its own and none outside |
| **3. Rule translation** | HS's `/routing/rule/migrate` and `/routing/migration/status` across all profiles | Every profile reports `migrated`, with its remaining gaps named and dispositioned |
| **4. Cutover** | `routing_source: de\|hs` feature flag, flipped per profile | Profiles serving from DE with HS rules retained for rollback; bake period passed |
| **5. Multi-objective** | Per-profile SR config; cost evidence via the inheritance ladder | Cost-aware routing live per profile with rate cards held once per merchant |
| **6. Dashboard** | Tree selector; breadcrumb; tree lookup; cost inheritance notice (see [Dashboard](#dashboard)) | Super-admin can enter any profile from the org tree without a DE user row; no page outside `TopBar` needed changing |

Phase 2 gates everything after it. Phase 1 and phase 3's read-only half — `/routing/migration/status`, which writes nothing — can run in parallel. **No
phase requires a database migration.**

---

## Risks and open items

- **Authorization is a behaviour change.** Any internal tooling relying on today's
  act-on-any-scope behaviour breaks at phase 2. Ship the check in log-only mode for one
  release to find them before enforcing.
- **Reverse indexes are eventually consistent.** They are written after the forward blobs, so
  a sync that dies midway can leave a profile whose ancestry is correct but which is missing
  from its merchant's list. Authorization reads only the forward direction and is therefore
  unaffected; the dashboard tree is what degrades, and re-running sync repairs it.
- **`tolerancePp` units.** `multi_objective_info.tolerancePp` is percentage points while
  analytics `tolerance_pp` is still a 0–1 fraction. Do not double-scale when surfacing
  per-profile tuning in the UI.
- **Score warm-up.** Profiles cut over to DE start with empty gateway-score state and cannot
  inherit a parent's — the ladder deliberately excludes scores. Expect a learning period per
  profile, and stage the cutover rather than flipping an org at once.
- **`gateway_id` completeness.** Older HS rules name a connector as a bare string with no
  `merchant_connector_id`, and these translate cleanly — `gateway_id` simply arrives null. They
  are still ambiguous where a profile holds two accounts on the same connector, and nothing in
  the migration path flags that, so it has to be found by inspection rather than waited for.
- **Profile reparenting.** If HS moves a profile between merchant accounts, the blob and the
  reverse indexes go stale until the next sync — and a merchant-level API key silently keeps
  or loses reach in the interim. Prefer calling sync from HS's profile mutation paths over a
  nightly job.
