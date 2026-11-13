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
  SR_SELECTION_MULTI_OBJECTIVE: 'bg-cyan-100 text-cyan-800',
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
