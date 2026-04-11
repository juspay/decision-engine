// Static routing keys configuration - fallback when API is unavailable
// These should match the values in the backend config
export type RoutingKeyType = 'enum' | 'integer' | 'udf' | 'str_value' | 'global_ref'

export interface StaticRoutingKeyConfig {
  type: RoutingKeyType
  values: string[]
}

export const STATIC_ROUTING_KEYS: Record<string, StaticRoutingKeyConfig> = {
  payment_method: {
    type: 'enum',
    values: [
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
    ],
  },
  amount: { type: 'integer', values: [] },
  currency: {
    type: 'enum',
    values: ['USD', 'EUR', 'GBP', 'INR', 'JPY', 'CAD', 'AUD', 'SGD', 'AED'],
  },
  payment_type: {
    type: 'enum',
    values: ['normal', 'new_mandate', 'setup_mandate', 'recurring_mandate', 'non_mandate'],
  },
  card_network: {
    type: 'enum',
    values: ['visa', 'mastercard', 'amex', 'jcb', 'diners_club', 'discover', 'cartes_bancaires', 'union_pay', 'interac', 'rupay', 'maestro'],
  },
  authentication_type: {
    type: 'enum',
    values: ['three_ds', 'no_three_ds'],
  },
  card_type: {
    type: 'enum',
    values: ['debit', 'credit'],
  },
  card: {
    type: 'enum',
    values: ['debit', 'credit'],
  },
}

export type RoutingKey = keyof typeof STATIC_ROUTING_KEYS

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

// Full payment method types for reference (backend validates against config)
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
