ALTER TABLE users MODIFY COLUMN merchant_id VARCHAR(255) NULL DEFAULT NULL;

CREATE TABLE user_merchants (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    user_id VARCHAR(64) NOT NULL,
    merchant_id VARCHAR(255) NOT NULL,
    role VARCHAR(50) NOT NULL DEFAULT 'admin',
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uk_user_merchant (user_id, merchant_id)
);

CREATE INDEX idx_user_merchants_user_id ON user_merchants (user_id);
CREATE INDEX idx_user_merchants_merchant_id ON user_merchants (merchant_id);

ALTER TABLE merchant_account ADD COLUMN merchant_name VARCHAR(255) NULL;
