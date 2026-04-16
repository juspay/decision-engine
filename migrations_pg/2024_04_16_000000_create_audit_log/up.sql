CREATE TABLE IF NOT EXISTS audit_log (
    id TEXT PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    endpoint TEXT NOT NULL,
    method TEXT NOT NULL,
    request_headers JSONB,
    request_body JSONB,
    response_status INTEGER NOT NULL,
    response_body JSONB,
    latency_ms INTEGER NOT NULL,
    merchant_id TEXT,
    request_id TEXT NOT NULL
);

CREATE INDEX idx_audit_log_timestamp ON audit_log (timestamp DESC);
CREATE INDEX idx_audit_log_endpoint_method ON audit_log (endpoint, method);
CREATE INDEX idx_audit_log_request_id ON audit_log (request_id);
CREATE INDEX idx_audit_log_merchant_id ON audit_log (merchant_id) WHERE merchant_id IS NOT NULL;
