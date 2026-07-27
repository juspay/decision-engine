# Hyperswitch → Decision Engine Merchant SSO (revised design)

## Overview

When a merchant clicks **Routing** on the Hyperswitch (HS) dashboard, HS performs a
server-to-server call to DE to obtain a **short-lived, single-use handoff code** for that
merchant, then redirects the merchant's browser to DE with the *code* (not a session token)
in the URL. DE's SPA immediately exchanges the code — server-side, over a POST — for a
short-lived session JWT, and lands the user directly on the routing page. No separate DE
login is required.

The session is **synthetic**: it carries the merchant identity in the JWT and grants access
to that merchant's routing view, **without creating a DE user row**. (Provisioning a real
user was considered — so the person could later set a password and log in directly — but DE
has **no forgot-password / password-reset flow** today, so a provisioned user could never
establish a password. Until a reset flow exists, we keep the session synthetic.)

The design uses things DE already has:

- the `x-admin-secret` shared secret that already gates `/merchant-account/create`,
- the JWT dashboard-session mechanism, and
- the fact that DE's [`authenticate`](../src/middleware.rs) middleware tries a `Bearer` JWT
  **before** falling back to `x-api-key` — so a dashboard JWT already unlocks the protected
  `/routing/*` routes.

> **Design in three points:**
> 1. **Handoff** — a one-time code in the URL, exchanged for the JWT over POST (the JWT never
>    touches a URL, log, or `Referer`).
> 2. **Scope** — the redirect session is *deny-by-default*: routing + analytics only; it cannot
>    manage identity (invite members, create merchants, change password, switch merchant).
> 3. **Hygiene** — short token life; the session token never appears in a URL, log, or `Referer`.

---

## DE's admin / api-key auth model (grounding)

DE has three auth mechanisms. The SSO flow uses the first and third.

| Mechanism | Header | What it is | Used by |
|---|---|---|---|
| **Admin secret** | `x-admin-secret` | A **single static shared secret** (`admin_secret.secret`), compared inline in the handler. Not a per-key system — its blast radius is the whole tenant. | `/merchant-account/create`; the SSO **mint** endpoint below |
| **Per-merchant API key** | `x-api-key` | Random key, SHA-256 hashed in `merchant_api_keys`; auto-minted on merchant creation | Protected router via `authenticate` middleware |
| **Dashboard JWT** | `Authorization: Bearer` | HS256 JWT with `user_id`, `merchant_id`, `role`, `token_type`, `jti`, `exp` | Dashboard / SSO sessions |

---

## End-to-end flow

```
1. Merchant clicks "Routing" on HS dashboard
        │
        ▼
2. HS backend calls DE (server-to-server):
   POST /auth/admin/merchant-token
   Header: x-admin-secret: <secret>        (P1: + IP allowlist / mTLS)
   Body:   { "merchant_id": "merchant_abc123" }
        │
        ▼
3. DE verifies admin secret + merchant exists in DB.
   Generates an opaque one-time code, stores in Redis:
     hs_sso_code:<code>  ->  { merchant_id }     TTL 60s, single-use
   Returns: { "code": "<opaque>" }              ◄── no JWT in this response
        │
        ▼
4. HS backend redirects merchant's browser to:
   https://de.example.com/routing?code=<code>
        │
        ▼
5. DE frontend (App.tsx) reads ?code= from the URL and POSTs it:
   POST /auth/admin/merchant-token/exchange   Body: { "code": "<code>" }
   DE atomically consumes the code (GETDEL), mints a SHORT-LIVED
   hs_redirect JWT (synthetic user_id = hs_<merchant_id>), returns it in the body.
        │
        ▼
6. App.tsx calls setAuth() with the JWT, sets merchant_id, strips ?code= from the URL
        │
        ▼
7. AuthGuard calls GET /auth/me
   Backend sees token_type=hs_redirect → returns synthetic response from JWT claims
        │
        ▼
8. Merchant lands on /routing — fully authenticated, scoped to their merchant
```

**Why the code, not the token, is in the URL:** a session JWT in a URL query string leaks —
*before any JS runs* — into the DE access log, any proxy/CDN log, browser history/bfcache,
and the `Referer` header of the first sub-resource the page loads. `history.replaceState`
runs only *after* those channels have already seen it. The one-time code is useless the
instant the SPA redeems it (or after 60 s), so a leaked code is inert. The JWT is only ever
returned in a POST body — exactly how `/auth/login` already returns tokens.

---

## Synthetic identity

The redirect session does **not** create or reference a `users` row.

- `user_id` = `hs_<merchant_id>` (a synthetic marker; no DB row exists, by design)
- `email` = `""`, `role` = `admin`, `merchant_id` = the real merchant, `token_type` = `hs_redirect`

This is safe **only** because the session is scoped to routing + analytics, which key off the
merchant id and never join on `user_id` (see capabilities and guards below). The mint endpoint
still verifies the merchant exists before issuing a code.

> **Migration note:** the mint endpoint `404`s if the `merchant_id` has no `merchant_account`
> row in DE. Provision merchant accounts as part of the HS→DE migration — not at
> token-issuance time.

---

## API endpoints

### `POST /auth/admin/merchant-token` — mint a handoff code

Server-to-server only. Never call from a browser.

**Auth:** `x-admin-secret` header. **P1:** a dedicated secret (distinct from
merchant-create), constant-time compare, IP allowlist / mTLS, and rate limiting.

**Request**
```json
{ "merchant_id": "merchant_abc123" }
```

**Response**
```json
{ "code": "DE_9f3c…", "expires_in": 60 }
```

| Status | Reason |
|--------|--------|
| `401 Unauthorized` | Missing or incorrect `x-admin-secret` |
| `404 Not Found` | `merchant_id` does not exist in DE's `merchant_account` table (see migration note) |
| `429 Too Many Requests` | Rate limit exceeded (P1) |
| `500` | Redis or config failure |

### `POST /auth/admin/merchant-token/exchange` — redeem the code

Called by the DE SPA. Public router (no `authenticate` middleware; it authenticates via the
code itself).

**Request**
```json
{ "code": "DE_9f3c…" }
```

**Response** — same `AuthResponse` shape as `/auth/login`:
```json
{
  "token": "<jwt>",
  "user_id": "hs_merchant_abc123",
  "email": "",
  "merchant_id": "merchant_abc123",
  "role": "admin",
  "merchants": []
}
```

| Status | Reason |
|--------|--------|
| `401 Unauthorized` | Code missing, unknown, expired, or already redeemed |
| `500` | Token generation or Redis failure |

**Single-use is mandatory.** Redemption must be atomic — use Redis `GETDEL` (or a Lua
get-and-delete, or a `set_key_if_not_exists` claim-lock) so two browser tabs can't both
redeem the same code.

---

## Handoff code (Redis)

| Property | Value |
|---|---|
| Key | `hs_sso_code:<code>` |
| Value | serialized `{ merchant_id }` (enough to mint the JWT) |
| TTL | 60 s |
| Redemption | atomic `GETDEL` — single use |
| Code format | opaque random, e.g. reuse `auth::generate_api_key`'s byte-source |

All primitives already exist in the codebase: opaque-token generation
(`auth::generate_api_key`), Redis `set_key_with_ttl` / `get_key_string` / `delete_key`, and
the public-router registration pattern.

---

## JWT token type

A `token_type` claim distinguishes redirect sessions from normal login sessions.

| `token_type` | Issued by | Description |
|---|---|---|
| `standard` | `/auth/login`, `/auth/signup` | Normal user session |
| `hs_redirect` | `/auth/admin/merchant-token/exchange` | HS-originated merchant session |

Tokens issued before this claim existed default to `standard`.

**Lifetime:** `hs_redirect` tokens are **short-lived** — `user_auth.hs_redirect_jwt_expiry_seconds`,
default 20 min — decoupled from the global `jwt_expiry_seconds` (24 h). HS can re-mint on demand,
so the redirect session does not need a long life, and a short life caps the blast radius of any
leak. On expiry the SPA does **not** fall back to the DE login page (the synthetic user has no
password); it shows a "reopen from Hyperswitch" screen so the user returns to HS for a fresh
session. `logout` adds the `jti` to the Redis denylist as usual.

---

## Session capabilities (deny-by-default)

The redirect session is scoped to the routing view. Everything identity-related is refused —
this is what makes the synthetic (no `users` row) identity safe.

| Feature | `hs_redirect` | Enforcement |
|---|---|---|
| View / edit routing rules | ✅ | Handlers scope off the request body `created_by`/`merchant_id`; frontend sources it from the token's merchant |
| Enable / disable routing algorithms | ✅ | same |
| Analytics | ✅ | Handlers scope off `claims.merchant_id` |
| `GET /auth/me` | ✅ | Synthetic response from JWT claims (no DB lookup) |
| `logout` | ✅ | `jti` denylist |
| Change password | ❌ 403 | `UnsupportedOperation` guard (already present) |
| Switch merchant | ❌ 403 | `UnsupportedOperation` guard (already present) |
| **Invite / manage members** | ❌ 403 | `UnsupportedOperation` guard (present) — see below |
| **Create merchant** | ❌ 403 | `UnsupportedOperation` guard (present) — see below |

**Why these two guards matter (both verified against the code):**

- `invite_member` is gated only by `role == "admin"`, which the `hs_redirect` token
  satisfies. Without a `token_type` guard, a redirect session can **create persistent real
  user accounts** — converting a transient token leak into a permanent foothold.
- `create_merchant` is on the public router with no `token_type` check. For a synthetic
  `hs_<merchant_id>` user (which has **no `users` row**, and there is **no FK** on
  `user_merchants.user_id`), it silently inserts an orphaned `user_merchants` row and a
  merchant owned by a phantom user.

Both get the same guard already used by `change_password` / `switch_merchant`:
`if claims.token_type == TOKEN_TYPE_HS_REDIRECT → UnsupportedOperation (403)`.

---

## Configuration

`hs_redirect` sessions are gated by `admin_secret.secret` (sent in the `x-admin-secret` header on
the mint endpoint) and signed with `user_auth.jwt_secret`. Their lifetime is
`user_auth.hs_redirect_jwt_expiry_seconds` (default `1200` = 20 min), independent of the 24 h
`jwt_expiry_seconds`. When the redirect token expires the SPA returns the user to Hyperswitch
rather than the DE login page (the synthetic user has no password).

```toml
[admin_secret]
secret = "your_admin_secret_here"      # never leave at the "test_admin" default

[user_auth]
hs_redirect_jwt_expiry_seconds = 1200  # 20 minutes
```
