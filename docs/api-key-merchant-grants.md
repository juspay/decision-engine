# API key merchant grants: storage foundation

`2026-11-03-000001_api_key_merchant_grants` adds `authz_version` to credentials and an
`api_key_merchants` table with one row per key/merchant, a positive authorization version,
`active`/`revoked` status, nullable permissions, and creation time. The credential's existing
`merchant_id` remains its owner and the sole runtime authorization scope in this release.
There is no shared-key issuance API or change to authentication, request scoping, or caches.

The migration preserves key IDs, hashes, prefixes, creation times, and revoked state. It
backfills exactly the credential's existing merchant; it never infers access from a user's
memberships, account names, or the Hyperswitch hierarchy. A revoked key or a key whose merchant
account is missing receives a revoked grant. Its credential is left unchanged. Replaying the
migration does not replace existing grants or reactivate revoked ones.

New single-merchant keys and their matching grants are inserted in one transaction. Existing
issuance behavior for a missing merchant is preserved, but its grant is revoked. Revocation
updates the credential, all its grants, and their versions atomically. `NULL` permissions
represent the existing unrestricted single-merchant credential only; future shared-key issuance
must require explicit, nonempty permissions and must check both key and grant status.

The key foreign key cascades when a credential is deleted. There is deliberately no merchant
foreign key: `merchant_account.merchant_id` is nullable and is not uniquely constrained in
the existing schema. Merchant deletion, disconnection, transfer, distributed invalidation,
permission enforcement, shared-key management, and tenant-bound authentication remain future
work. Do not manually add shared grants and expect them to authorize requests in this release.

Run the matching migration directory for each tenant database/schema that stores credentials.
Membership is local to that schema; no grant authorizes access to another tenant. This change
does not alter the existing helpers that resolve the public tenant.

Deploy migrations before the new binary, because Diesel reads the new credential column.
Rollback the binary before running `down.sql`; rollback removes grant metadata and versions,
leaving the original credentials intact. Drain old writers during migration/deployment, or
rerun the idempotent backfill after all writers use the new insertion path, before a later
release starts enforcing grants. An old binary can otherwise mint a key without a grant.

Run `python3 scripts/test_api_key_grants_migration.py` with Docker available to verify both
migration tracks. The stdlib-only harness creates and removes its own PostgreSQL 16 and
MySQL 8 containers, exposes no host ports, and uses synthetic fixtures. It checks backfill,
credential preservation, replay, constraints, reverse lookup indexes, cascade, and down/up,
including MySQL when the database's default collation changed. It does not exercise the Rust
mint/revoke transaction paths or runtime authentication.
