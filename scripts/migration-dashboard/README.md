# Migration front

A local console over Hyperswitch's routing-rule migration endpoints: where every profile stands
in the move to the decision engine, and a way to move the ones that have not crossed yet.

Rule translation happens in Hyperswitch, not here — it holds the rules, the credentials and the
connector context (see [`docs/hs-hierarchy-migration.md`](../../docs/hs-hierarchy-migration.md)).
This tool only drives the endpoints:

| Endpoint | Used for |
|---|---|
| `GET /routing/migration/status` (HS) | Every profile holding rules, with its HS and DE rule ids set-diffed |
| `POST /routing/rule/migrate` (HS) | Writing a batch of profiles' rules into the DE, and provisioning the scope they land in |
| `POST /admin/hierarchy/reconcile` (DE) | Which scanned profiles the DE has no scope for. Read-only |
| `PUT /context` (Superposition) | Cutting a profile's routing over, and back |

The Hyperswitch endpoints are admin-authenticated and idempotent: rules already in the decision
engine are skipped, so a partial run repairs itself on the next one.

## Running it

```bash
scripts/migration-dashboard/server.py
```

Then open <http://127.0.0.1:8090>. Python 3.9+, standard library only.

## Environments

The console holds as many environments as you configure and works one at a time; the header
picker switches between them. Sandbox and Production EU are seeded, without credentials — the
keys are pasted in when needed. **Environment** in the header sets, per environment:

| Field | What it is |
|---|---|
| Environment, Kind | Its name, and what a write here reaches — *sandbox*, *production*, *local*, *other*. Kind is guessed from the URL and yours to correct |
| Hyperswitch URL + Admin API key | Sent as `api-key`. Base URL, so a path on it is kept |
| Extra headers | Sent on every HS call; hosted environments route by `x-feature` |
| Superposition URL + secret + headers | Where cutovers are written (`x-superposition-secret`, `x-tenant`, `x-org-id`). Leave empty and the dashboard stays read-only about routing |
| Override key | The config a cutover writes, `routing.routing_result_source` by default |
| Decision engine URL + admin secret + headers | Where scopes are read (`x-admin-secret`). Leave empty and the scope column reads as unchecked |

Credentials stay in the local server process — the page only ever sees their last four
characters. Nothing touches disk unless **Remember on this machine** is ticked, which writes
`~/.config/hs-migration-dashboard/config.json` at mode 0600 (credentials only if *including the
credentials* is ticked too). **Forget saved** removes it.

Changing an environment's URL clears its key and the scan; switching environments clears the
scan but keeps each one's credentials. Profile ids are not comparable across environments.

`--env`, `--hs-url`, `--admin-api-key`, `--de-url`, `--de-admin-secret`, `--port`, `--host` (or
`HS_ENV`, `HS_URL`, `HS_ADMIN_API_KEY`, `DE_URL`, `DE_ADMIN_SECRET`, `PORT`, `HOST`) open on a
named environment or preset its fields:

```bash
scripts/migration-dashboard/server.py --env production-eu
```

It listens on loopback only. This server authenticates nobody and holds the admin credentials,
so whoever reaches the port is the operator — a non-loopback `--host` is refused unless
`--allow-remote` is given with it. Forward the port over SSH rather than binding wide.

**Production.** An environment of kind *production* is marked wherever it is named, and the
first migration, cutover or scope run of a session asks for its name to be typed back — once per
session, checked by the server too, so a page left open cannot write on a click alone. Nothing
else differs: same endpoints, same idempotency.

## Reading the table

**Scan estate** walks the status endpoint to the end of the feed, 500 profiles at a time, and
resolves each merchant through `GET /accounts/{merchant_id}`. Every profile costs Hyperswitch a
call to the decision engine, so it runs as a background job with progress. **Rescan** reads it
again.

The crossing bar on each row — and once across the estate — splits that scope's rules into
landed in the DE, still only in HS, and out of scope. States come from Hyperswitch:

| State | Meaning |
|---|---|
| `pending` | Rules in Hyperswitch, none in the decision engine |
| `partial` | Some crossed, some did not |
| `diverged` | Same number of rules on both sides, but not the same ones |
| `migrated` | Every rule is across; Hyperswitch is still routing |
| `enabled` | Cut over — the decision engine decides this profile's routing |
| `enabled_without_rules` | Cut over with rules missing, so traffic falls through to the default connector list |
| `unknown` | The decision engine could not be read |

Click a state chip to filter; chips combine with the org, merchant and routing-source selectors
and the search box, which covers rule ids as well as scope ids.

The **Note** column says why a profile is held back:

- `merchant account missing` — the rules outlived the merchant account. Held back by default;
  see *Allow profiles with no merchant account* below. Fixing it means dispositioning the
  account in Hyperswitch.
- `only out-of-scope rules` — the profile holds nothing but `dynamic` or `three_ds_decision_rule`
  rules, which reach the decision engine another way. Already finished despite reading `pending`.
- `no scope in DE` / `no ancestry in DE` — see [Scopes](#scopes).

## Migrating

Click a row to open its panel — *Migrate this profile*, *Add to batch*, and the rule ids it is
missing. For many profiles, tick the checkboxes (the header one takes every movable profile on
the page; a selection survives paging) and choose **Migrate selected**. Runs go 25 profiles at a
time; a failed batch does not stop the ones after it, and re-running is safe. A disabled
checkbox says why on hover. The scan refreshes when the run finishes.

**Allow profiles with no merchant account** sends the profiles Hyperswitch cannot read an
account for; they come back `not attempted` otherwise. Profiles holding only out-of-scope rules
stay unavailable either way.

`/routing/rule/migrate` reads at most 1000 rules per profile per call, so a larger profile is
walked with successive offsets rather than reported as done on its first page.

**Tracing a failure.** Every report line ends with the `x-request-id` of the Hyperswitch request
that produced it, and Hyperswitch logs under that id — click it to select it whole. That is what
makes `HE_00 · Something went wrong` actionable. Upstream errors name the service and endpoint
too, so a `401` is attributable to one of the three credentials.

Migration writes rules; it does not switch traffic. A profile reads `migrated` until it is cut
over.

## Scopes

Whether the decision engine holds a **scope** for a profile — a `merchant_account` row keyed by
the profile id — is a separate question from rules, and Hyperswitch cannot answer it: its
per-profile read lists rules only, so a profile with no scope looks exactly like a scope with no
rules yet. It reads `pending`, a migration **succeeds** on it and turns it `migrated`, and the
failure only shows on live traffic after cutover — `decide-gateway` loads the scope first and
answers `MERCHANT_NOT_FOUND` without one.

So the dashboard asks the decision engine directly: each scan posts the scanned profiles as an
org → merchant → profile tree to the read-only `POST /admin/hierarchy/reconcile`, which answers
with `missing_in_de` and `has_ancestry`. Missing scopes carry a `no scope in DE` note, count
under **Missing in DE**, and are **held back from cutover** (enforced by the server against the
last scan). Missing ancestry is milder — routing is unaffected, but the dashboard cannot name
the account above the profile and org or merchant grants cannot expand to it — so it is
reported, not held back. Migration is never held back; nor is rolling back.

**Provision scopes** — from a row panel, a selection, or the notice — creates them by
**re-running the migration**, because `/routing/rule/migrate` provisions the scope and its
ancestry before it reads a rule. Every rule is skipped, so a pure provisioning run reports
skipping all of them. It works this way rather than calling the DE's `/admin/hierarchy/sync`
because a scope is only as good as its ancestry, and the status feed carries neither profile
name nor org name, and its `merchant_id` is the *provider* for platform-written rules. Only
Hyperswitch has it right. Re-running is safe: `sync` writes only what differs.

**Stranded** scopes — a DE scope whose id is an HS *merchant* id rather than a profile id — are
reported in the notice and never written to. They hold rules and scores no profile inherits.
Only a person can say which profile was meant.

Leave the decision engine unset and none of this runs; the notice says the check did not happen,
because an unchecked scope and a present one are not the same claim.

## Cutting over

A `migrated` profile is fully across and still routed by Hyperswitch. What decides that is
`routing.routing_result_source`, read per profile from Superposition, so cutting over means
writing an override for it:

```bash
curl --request PUT 'https://superposition.example.net/context' \
  --header 'x-superposition-secret: <secret>' \
  --header 'x-tenant: hyperswitch' --header 'x-org-id: hyperswitch' \
  --data '{"context": {"profile_id": "pro_…"},
           "override": {"routing.routing_result_source": "decision_engine"},
           "description": "Cut over profile pro_… to Decision Engine",
           "change_reason": "DE cutover"}'
```

which is what **Cut over to decision engine** does, from a row panel or from the selection bar.
The direction is offered by state, and the server enforces it against the last scan so a stale
tab cannot cut over a profile that has since fallen behind:

| Profile state | Offered |
|---|---|
| `migrated`, with a scope | *Cut over to decision engine* |
| `migrated`, no scope | Neither — the decision engine would refuse the profile outright |
| `enabled`, `enabled_without_rules` | *Roll back to Hyperswitch* — routing only, no rule is touched |
| anything else | Neither — live traffic would route against a DE missing rules |

Failures raise a banner as well as appearing in the report, and a failed profile can just be
sent again. The estate is not re-read afterwards; **Rescan** when you want it.

**The override key.** Hyperswitch renamed this config into a folder — `routing_result_source`
became `routing.routing_result_source` — so which name works depends on when the workspace was
seeded. A workspace refuses an override whose key it has no schema for, so a wrong name fails
loudly (`failed to get schema for config key …`); the run retries once under the other spelling,
adopts it for the rest of the run, and leaves the Override key field holding it. When both are
refused, nothing changes.

The one failure nothing here can detect: if a workspace defines *both* names, an override under
the one the router does not read is accepted and does nothing. A cutover that reports every
profile written while a rescan still shows them `migrated` is that.

Cutovers move live traffic the moment Superposition accepts the override. Rules are untouched in
both directions, so it is reversible by rolling back.
