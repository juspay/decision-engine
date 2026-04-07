export const ROUTING_KEYS = {
  payment_method: {
    type: 'enum' as const,
    values: ['card', 'card_redirect', 'pay_later', 'wallet', 'bank_redirect', 'bank_transfer', 'crypto', 'bank_debit', 'reward', 'real_time_payment', 'upi', 'voucher', 'gift_card', 'open_banking', 'mobile_payment'],
  },
  amount: { type: 'integer' as const, values: [] },
  currency: {
    type: 'enum' as const,
    values: ['USD', 'EUR', 'GBP', 'INR', 'JPY', 'CAD', 'AUD', 'SGD', 'AED', 'AFN', 'ALL', 'AMD', 'ANG', 'AOA', 'ARS', 'AWG', 'AZN', 'BAM', 'BBD', 'BDT', 'BGN', 'BHD', 'BIF', 'BMD', 'BND', 'BOB', 'BRL', 'BSD', 'BTN', 'BWP', 'BYN', 'BZD', 'CDF', 'CHF', 'CLF', 'CLP', 'CNY', 'COP', 'CRC', 'CUC', 'CUP', 'CVE', 'CZK', 'DJF', 'DKK', 'DOP', 'DZD', 'EGP', 'ERN', 'ETB', 'FJD', 'FKP', 'GEL', 'GHS', 'GIP', 'GMD', 'GNF', 'GTQ', 'GYD', 'HKD', 'HNL', 'HRK', 'HTG', 'HUF', 'IDR', 'ILS', 'IQD', 'IRR', 'ISK', 'JMD', 'JOD', 'KES', 'KGS', 'KHR', 'KMF', 'KPW', 'KRW', 'KWD', 'KYD', 'KZT', 'LAK', 'LBP', 'LKR', 'LRD', 'LSL', 'LYD', 'MAD', 'MDL', 'MGA', 'MKD', 'MMK', 'MNT', 'MOP', 'MRU', 'MUR', 'MVR', 'MWK', 'MXN', 'MYR', 'MZN', 'NAD', 'NGN', 'NIO', 'NOK', 'NPR', 'NZD', 'OMR', 'PAB', 'PEN', 'PGK', 'PHP', 'PKR', 'PLN', 'PYG', 'QAR', 'RON', 'RSD', 'RUB', 'RWF', 'SAR', 'SBD', 'SCR', 'SDG', 'SEK', 'SHP', 'SLE', 'SLL', 'SOS', 'SRD', 'SSP', 'STD', 'STN', 'SVC', 'SYP', 'SZL', 'THB', 'TJS', 'TMT', 'TND', 'TOP', 'TRY', 'TTD', 'TWD', 'TZS', 'UAH', 'UGX', 'UYU', 'UZS', 'VES', 'VND', 'VUV', 'WST', 'XAF', 'XCD', 'XOF', 'XPF', 'YER', 'ZAR', 'ZMW', 'ZWL'],
  },
  payment_type: {
    type: 'enum' as const,
    values: ['normal', 'new_mandate', 'setup_mandate', 'recurring_mandate', 'non_mandate'],
  },
  card_network: {
    type: 'enum' as const,
    values: ['visa', 'mastercard', 'amex', 'jcb', 'diners_club', 'discover', 'cartes_bancaires', 'union_pay', 'interac', 'rupay', 'maestro'],
  },
  payment_card_type: {
    type: 'enum' as const,
    values: ['CREDIT', 'DEBIT'],
  },
  payment_card_issuer_country: {
    type: 'enum' as const,
    values: ['AF', 'AX', 'AL', 'DZ', 'AS', 'AD', 'AO', 'AI', 'AQ', 'AG', 'AR', 'AM', 'AW', 'AU', 'AT', 'AZ', 'BS', 'BH', 'BD', 'BB', 'BY', 'BE', 'BZ', 'BJ', 'BM', 'BT', 'BO', 'BQ', 'BA', 'BW', 'BV', 'BR', 'IO', 'BN', 'BG', 'BF', 'BI', 'KH', 'CM', 'CA', 'CV', 'KY', 'CF', 'TD', 'CL', 'CN', 'CX', 'CC', 'CO', 'KM', 'CG', 'CD', 'CK', 'CR', 'CI', 'HR', 'CU', 'CW', 'CY', 'CZ', 'DK', 'DJ', 'DM', 'DO', 'EC', 'EG', 'SV', 'GQ', 'ER', 'EE', 'ET', 'FK', 'FO', 'FJ', 'FI', 'FR', 'GF', 'PF', 'TF', 'GA', 'GM', 'GE', 'DE', 'GH', 'GI', 'GR', 'GL', 'GD', 'GP', 'GU', 'GT', 'GG', 'GN', 'GW', 'GY', 'HT', 'HM', 'VA', 'HN', 'HK', 'HU', 'IS', 'IN', 'ID', 'IR', 'IQ', 'IE', 'IM', 'IL', 'IT', 'JM', 'JP', 'JE', 'JO', 'KZ', 'KE', 'KI', 'KP', 'KR', 'KW', 'KG', 'LA', 'LV', 'LB', 'LS', 'LR', 'LY', 'LI', 'LT', 'LU', 'MO', 'MK', 'MG', 'MW', 'MY', 'MV', 'ML', 'MT', 'MH', 'MQ', 'MR', 'MU', 'YT', 'MX', 'FM', 'MD', 'MC', 'MN', 'ME', 'MS', 'MA', 'MZ', 'MM', 'NA', 'NR', 'NP', 'NL', 'NC', 'NZ', 'NI', 'NE', 'NG', 'NU', 'NF', 'MP', 'NO', 'OM', 'PK', 'PW', 'PS', 'PA', 'PG', 'PY', 'PE', 'PH', 'PN', 'PL', 'PT', 'PR', 'QA', 'RE', 'RO', 'RU', 'RW', 'BL', 'SH', 'KN', 'LC', 'MF', 'PM', 'VC', 'WS', 'SM', 'ST', 'SA', 'SN', 'RS', 'SC', 'SL', 'SG', 'SX', 'SK', 'SI', 'SB', 'SO', 'ZA', 'GS', 'SS', 'ES', 'LK', 'SD', 'SR', 'SJ', 'SZ', 'SE', 'CH', 'SY', 'TW', 'TJ', 'TZ', 'TH', 'TL', 'TG', 'TK', 'TO', 'TT', 'TN', 'TR', 'TM', 'TC', 'TV', 'UG', 'UA', 'AE', 'GB', 'UM', 'US', 'UY', 'UZ', 'VU', 'VE', 'VN', 'VG', 'VI', 'WF', 'EH', 'YE', 'ZM', 'ZW'],
  },
  authentication_type: {
    type: 'enum' as const,
    values: ['three_ds', 'no_three_ds'],
  },
  card_type: {
    type: 'enum' as const,
    values: ['debit', 'credit'],
  },
  card: {
    type: 'enum' as const,
    values: ['debit', 'credit'],
  },
  txn_is_emi: {
    type: 'enum' as const,
    values: ['true', 'false'],
  },
  billing_country: {
    type: 'enum' as const,
    values: ['India', 'UnitedStates', 'Afghanistan', 'Albania', 'Algeria', 'Argentina', 'Australia', 'Austria', 'Belgium', 'Brazil', 'Canada', 'Chile', 'China', 'Colombia', 'Denmark', 'Finland', 'France', 'Germany', 'Greece', 'HongKong', 'Hungary', 'Indonesia', 'Ireland', 'Israel', 'Italy', 'Japan', 'Malaysia', 'Mexico', 'Netherlands', 'NewZealand', 'Norway', 'Pakistan', 'Philippines', 'Poland', 'Portugal', 'Romania', 'Russia', 'SaudiArabia', 'Singapore', 'SouthAfrica', 'SouthKorea', 'Spain', 'Sweden', 'Switzerland', 'Thailand', 'Turkey', 'UnitedArabEmirates', 'UnitedKingdom', 'Vietnam'],
  },
  payment_method_type: {
    type: 'enum' as const,
    values: ['card', 'credit', 'debit', 'upi_collect', 'upi_intent', 'apple_pay', 'google_pay', 'paypal', 'klarna', 'afterpay_clearpay', 'affirm', 'sepa', 'ach', 'bacs', 'ideal', 'sofort', 'giropay', 'bancontact_card', 'blik', 'eps', 'pix', 'boleto', 'oxxo', 'multibanco', 'pse', 'trustly', 'interac', 'bizum', 'venmo', 'cashapp', 'amazon_pay', 'samsung_pay', 'twint', 'swish', 'vipps', 'we_chat_pay', 'ali_pay', 'dana', 'gcash', 'momo', 'kakao_pay', 'touch_n_go', 'go_pay', 'walley', 'alma', 'atome', 'pay_bright'],
  },
}

export type RoutingKey = keyof typeof ROUTING_KEYS

export const ROUTING_APPROACH_COLORS: Record<string, string> = {
  SR_SELECTION_V3_ROUTING: 'bg-blue-100 text-blue-800',
  PRIORITY_LOGIC: 'bg-purple-100 text-purple-800',
  NTW_BASED_ROUTING: 'bg-green-100 text-green-800',
  SR_SELECTION_V3_ROUTING_WITH_HEDGING: 'bg-orange-100 text-orange-800',
  HEDGING: 'bg-orange-100 text-orange-800',
}

export const SR_DIMENSION_OPTIONS = [
  { key: 'currency', label: 'Currency' },
  { key: 'country', label: 'Country' },
  { key: 'auth_type', label: 'Auth Type' },
  { key: 'card_is_in', label: 'Card Is In' },
  { key: 'card_network', label: 'Card Network' },
]

export const PAYMENT_METHOD_TYPES = [
  'card',
  'card_redirect',
  'pay_later',
  'wallet',
  'bank_redirect',
  'bank_transfer',
  'crypto',
  'bank_debit',
  'reward',
  'real_time_payment',
  'upi',
  'voucher',
  'gift_card',
  'open_banking',
  'mobile_payment',
]

export const PAYMENT_METHODS: Record<string, string[]> = {
  card: ['credit', 'debit'],
  bank_debit: ['ach', 'sepa', 'bacs', 'becs'],
  bank_transfer: ['ach', 'sepa', 'sepa_bank_transfer', 'bacs', 'multibanco', 'pix', 'pse', 'permata_bank_transfer', 'bca_bank_transfer', 'bni_va', 'bri_va', 'cimb_va', 'danamon_va', 'mandiri_va', 'local_bank_transfer', 'instant_bank_transfer'],
  wallet: ['amazon_pay', 'apple_pay', 'google_pay', 'paypal', 'ali_pay', 'ali_pay_hk', 'dana', 'mb_way', 'mobile_pay', 'samsung_pay', 'twint', 'vipps', 'touch_n_go', 'swish', 'we_chat_pay', 'go_pay', 'gcash', 'momo', 'kakao_pay', 'cashapp', 'mifinity', 'paze'],
  pay_later: ['affirm', 'alma', 'afterpay_clearpay', 'klarna', 'pay_bright', 'atome', 'walley'],
  upi: ['upi_collect', 'upi_intent'],
  voucher: ['boleto', 'efecty', 'pago_efectivo', 'red_compra', 'red_pagos', 'indomaret', 'alfamart', 'oxxo', 'seven_eleven', 'lawson', 'mini_stop', 'family_mart', 'seicomart', 'pay_easy'],
  bank_redirect: ['giropay', 'ideal', 'sofort', 'eft', 'eps', 'bancontact_card', 'blik', 'local_bank_redirect', 'online_banking_thailand', 'online_banking_czech_republic', 'online_banking_finland', 'online_banking_fpx', 'online_banking_poland', 'online_banking_slovakia', 'przelewy24', 'trustly', 'bizum', 'interac', 'open_banking_uk', 'open_banking_pis'],
  gift_card: ['givex', 'pay_safe_card'],
  card_redirect: ['knet', 'benefit', 'momo_atm', 'card_redirect'],
  real_time_payment: ['fps', 'duit_now', 'prompt_pay', 'viet_qr'],
  crypto: ['crypto_currency'],
  reward: ['evoucher', 'classic_reward'],
  open_banking: ['open_banking_pis'],
  mobile_payment: ['direct_carrier_billing'],
}
