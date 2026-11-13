CREATE TABLE merchant_api_keys (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    key_id VARCHAR(64) NOT NULL UNIQUE,
    merchant_id VARCHAR(255) NOT NULL,
    key_hash VARCHAR(64) NOT NULL UNIQUE,
    key_prefix VARCHAR(16) NOT NULL,
    description VARCHAR(255),
    is_active TINYINT(1) NOT NULL DEFAULT 1,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
);

CREATE INDEX idx_merchant_api_keys_merchant_id ON merchant_api_keys (merchant_id);
