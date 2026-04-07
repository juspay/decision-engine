export const ROUTING_KEYS = {
  payment_method: {
    type: 'enum' as const,
    values: ['card', 'bank_debit', 'bank_transfer', 'wallet', 'pay_later'],
  },
  payment_amount: { type: 'integer' as const, values: [] },
  payment_currency: {
    type: 'enum' as const,
    values: ['USD', 'EUR', 'GBP', 'INR', 'JPY', 'CAD', 'AUD', 'SGD'],
  },
  payment_type: {
    type: 'enum' as const,
    values: ['CARD', 'UPI', 'NB', 'WALLET'],
  },
  payment_card_brand: {
    type: 'enum' as const,
    values: ['VISA', 'MASTERCARD', 'AMEX', 'RUPAY', 'DINERS', 'JCB'],
  },
  payment_card_type: {
    type: 'enum' as const,
    values: ['CREDIT', 'DEBIT', 'PREPAID'],
  },
  payment_card_issuer_country: {
    type: 'enum' as const,
    values: ['INDIA', 'US', 'UK', 'SINGAPORE', 'UAE'],
  },
  payment_auth_type: {
    type: 'enum' as const,
    values: ['THREE_DS', 'NO_THREE_DS'],
  },
  txn_is_emi: {
    type: 'enum' as const,
    values: ['true', 'false'],
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
  'bank_debit',
  'bank_transfer',
  'wallet',
  'pay_later',
]

export const PAYMENT_METHODS: Record<string, string[]> = {
  card: ['credit', 'debit', 'prepaid'],
  bank_debit: ['ach', 'sepa', 'bacs'],
  bank_transfer: ['wire', 'ach_credit'],
  wallet: ['apple_pay', 'google_pay', 'paypal'],
  pay_later: ['klarna', 'afterpay', 'affirm'],
}
