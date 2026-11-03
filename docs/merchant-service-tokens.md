# Merchant service token verifier foundation

`src/auth/merchant_service.rs` provides a verifier library for short-lived tokens targeting one
merchant. This change does not issue tokens, add an HTTP endpoint, accept these tokens in middleware,
or change existing dashboard JWTs/API keys. It introduces no platform account or platform identifier.

The future flow is: authenticate a shared API key and independently establish the merchant's identity
through merchant authentication or a trusted issuer's proof, authorize the key's grant to that proven
merchant, issue a short-lived merchant service token, and check that token and the current grant on
each call. Possession of the shared key plus a claimed merchant ID is never sufficient to issue a
token. The shared API key is a credential to authenticate, never a JWT signing secret. A server-held
private signing key issues tokens; verification receives only trusted public keys.

## Enforced contract

The caller constructs a `MerchantServiceTokenVerifier` with a server-owned issuer, audience,
maximum lifetime in seconds (1–300), and public P-256 JWKs. Each signing key must have a nonempty,
unique `kid`. Private and symmetric keys are refused. ES256 is pinned by the verifier, not selected by
the token. The token's `kid` only selects an already trusted signing key; token-supplied keys and URLs
do not supply trust. Multiple public keys allow an overlapping signing-key rotation window.

The signed payload must contain all of the following:

```json
{
  "iss": "decision-engine",
  "aud": "decision-engine-api",
  "token_type": "merchant_service",
  "key_id": "key_shared_1",
  "tenant_id": "public",
  "merchant_id": "merchant_1",
  "permissions": ["routing:execute", "feedback:write"],
  "grant_version": 2,
  "key_auth_version": 3,
  "jti": "unique-token-id",
  "iat": 1700000000,
  "exp": 1700000300
}
```

- `iss`, a single string `aud`, and `token_type` must exactly match the verifier policy and dedicated
  service token type. Dashboard token types are refused. String claims must be nonempty without
  leading/trailing whitespace.
- `key_id` identifies the shared API key; it is distinct from the signing key's header `kid`.
  `key_id`, `tenant_id`, and `merchant_id` must exactly match the supplied expected binding. Runtime
  callers must obtain the key/tenant binding from trusted server state, not accept it from token
  claims as its own proof. Merchant identifiers have the existing Decision Engine routing-scope
  meaning, including profile-backed scopes.
- `permissions` is a required, nonempty array with no duplicates. Recognized values are
  `routing:execute`, `routing:read`, `routing:write`, `feedback:write`, and `analytics:read`. Unknown or
  malformed values are refused. There is no fallback to dashboard permissions or unrestricted access.
  The service permission enum is independent of dashboard roles and permissions.
- `grant_version` and `key_auth_version` must be positive unsigned integers. Their presence/type is
  checked here; matching them against live authorization state is future integration work.
- `jti` is mandatory. `iat` and `exp` are integer Unix seconds; `exp > iat`, `exp > now`, and the
  lifetime cannot exceed policy. Future issuance is refused. Optional `nbf` must be an integer no
  later than now and strictly before expiration. There is no clock leeway. Tokens expire exactly at
  `exp`. Input is capped at 8 KiB.

The public `verify` method reads the system clock. Its result exposes validated claims but does not
construct an `AuthContext` or `AuthorizedMerchant` and does not authorize a route.

## Required before runtime enablement

Future integration must check the live shared-key status, tenant binding, current merchant grant,
grant/key authorization versions, and required permission before constructing request auth/scope.
Token issuance must require independent merchant authentication or a trusted issuer's proof of that
merchant's identity in addition to a currently authenticated shared key. The shared key and a
caller-supplied merchant ID alone must never mint a token, even if the key has a grant to that merchant.
The resulting merchant must match the independent proof, and token permissions must be the
intersection of the key's and merchant grant's allowed permissions. Version claims must map to the
storage authorization-version fields.
Revoking a key, removing a merchant grant, or reducing permissions must invalidate authorization
with a defined cache/revocation policy, rather than relying solely on this token's expiration.
Token IDs enable audit correlation; this verifier does not implement a replay cache or token denylist.
Permission names alone do not enable feedback, analytics, or any other endpoint.

Signed-token unit tests cover signature/key trust, algorithm rejection, issuer/audience/type, scope
binding, mandatory and malformed claims, versions, permissions, expiry, lifetime, and `nbf`.
