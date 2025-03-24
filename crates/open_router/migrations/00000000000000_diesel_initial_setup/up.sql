CREATE TABLE card_details (
    id VARCHAR(36) PRIMARY KEY,
    merchant_id VARCHAR(50) NOT NULL,
    customer_id VARCHAR(50),
    card_number VARCHAR(255) NOT NULL,
    card_exp_month VARCHAR(2) NOT NULL,
    card_exp_year VARCHAR(4) NOT NULL,
    card_holder_name VARCHAR(255),
    card_network VARCHAR(50),
    card_type VARCHAR(50),
    card_issuer VARCHAR(100),
    card_issuing_country VARCHAR(3),
    card_fingerprint VARCHAR(64) NOT NULL,
    last_used_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_merchant_customer (merchant_id, customer_id),
    INDEX idx_fingerprint (card_fingerprint)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;