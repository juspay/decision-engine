#!/usr/bin/env bash
# Generates a short-lived JWT signed with the local dev secret.
# Usage: source scripts/gen_local_token.sh   → sets $LOCAL_TOKEN
#        scripts/gen_local_token.sh           → prints token

python3 - <<'EOF'
import base64, json, hmac, hashlib, time, sys, uuid

secret = "change_me_in_production_use_32chars!!"
header = base64.urlsafe_b64encode(
    json.dumps({"alg":"HS256","typ":"JWT"}, separators=(',',':')).encode()
).rstrip(b'=').decode()

now = int(time.time())
payload = base64.urlsafe_b64encode(
    json.dumps({
        "sub":         "16ec6f4a-9677-4c79-9838-f24e14809921",
        "user_id":     "16ec6f4a-9677-4c79-9838-f24e14809921",
        "email":       "test@gmail.com",
        "merchant_id": "merchant_baea25c53626",
        "role":        "admin",
        "jti":         str(uuid.uuid4()),
        "iat":         now,
        "exp":         now + 86400,
    }, separators=(',',':')).encode()
).rstrip(b'=').decode()

msg = f"{header}.{payload}"
sig = base64.urlsafe_b64encode(
    hmac.new(secret.encode(), msg.encode(), hashlib.sha256).digest()
).rstrip(b'=').decode()

print(f"{msg}.{sig}")
EOF
