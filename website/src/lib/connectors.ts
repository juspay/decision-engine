import { getDashboardConnectors } from './dashboardConnectors'

export const ROUTABLE_CONNECTORS: readonly string[] = [
  'absa_sanlam',
  'aci',
  'adyen',
  'adyenplatform',
  'affirm',
  'airwallex',
  'amazonpay',
  'archipel',
  'authipay',
  'authorizedotnet',
  'bambora',
  'bamboraapac',
  'bankofamerica',
  'barclaycard',
  'billwerk',
  'bitpay',
  'blackhawknetwork',
  'bluesnap',
  'boku',
  'braintree',
  'breadpay',
  'calida',
  'cardinal',
  'cashtocode',
  'celero',
  'chargebee',
  'checkbook',
  'checkout',
  'coinbase',
  'coingate',
  'cryptopay',
  'ctp_mastercard',
  'ctp_visa',
  'custombilling',
  'cybersource',
  'cybersourcedecisionmanager',
  'datatrans',
  'deutschebank',
  'digitalvirgo',
  'dlocal',
  'dwolla',
  'ebanx',
  'elavon',
  'envoy',
  'facilitapay',
  'finix',
  'fiserv',
  'fiservcommercehub',
  'fiservemea',
  'fiuu',
  'flexiti',
  'forte',
  'getnet',
  'gigadat',
  'givepayments',
  'globalpay',
  'globepay',
  'gocardless',
  'helcim',
  'hipay',
  'hyperpg',
  'iatapay',
  'imerchantsolutions',
  'inespay',
  'interpayments',
  'itaubank',
  'jpmorgan',
  'juspaythreedsserver',
  'klarna',
  'loonio',
  'mifinity',
  'mollie',
  'moneris',
  'multisafepay',
  'netcetera',
  'nexinets',
  'nexixpay',
  'nmi',
  'nomupay',
  'noon',
  'nordea',
  'novalnet',
  'nuvei',
  'opennode',
  'paybox',
  'payconex',
  'payjustnow',
  'payjustnowinstore',
  'payload',
  'payme',
  'payone',
  'paypal',
  'paysafe',
  'paystack',
  'paytm',
  'payu',
  'peachpayments',
  'phonepe',
  'placetopay',
  'plaid',
  'powertranz',
  'prophetpay',
  'rapyd',
  'razorpay',
  'recurly',
  'redsys',
  'revolv3',
  'riskified',
  'santander',
  'shift4',
  'signifyd',
  'silverflow',
  'square',
  'stax',
  'stripe',
  'stripebilling',
  'tesouro',
  'threedsecureio',
  'tokenio',
  'truelayer',
  'trustly',
  'trustpay',
  'trustpayments',
  'tsys',
  'tsys_transit',
  'vgs',
  'volt',
  'wellsfargo',
  'wise',
  'worldline',
  'worldpay',
  'worldpaymodular',
  'worldpayvantiv',
  'worldpayxml',
  'xendit',
  'zen',
  'zift',
  'zsl',
]

export interface GatewayOption {
  name: string
  /** The connector's merchant connector account id, when the dashboard handed one over. */
  gatewayId?: string
}

/**
 * Options for a gateway field.
 *
 * Opened from the Hyperswitch dashboard, the hand-off names the connectors that actually exist on
 * the profile, so it replaces the generic catalogue outright — offering a connector the merchant
 * has not configured would build a rule that can never route.
 *
 * Standalone, it falls back to the previous behaviour: names already used elsewhere in the rule
 * first (they are the likely next pick, and may be merchant-specific names absent from the
 * connector list), then every routable connector, minus whatever this field has already collected.
 */
export function gatewayOptions(
  suggestions: readonly string[],
  alreadyPicked: readonly string[] = [],
): GatewayOption[] {
  const picked = new Set(alreadyPicked.filter(Boolean))
  const handoff = getDashboardConnectors()

  if (handoff) {
    return handoff
      .filter((connector) => !picked.has(connector.connector_name))
      .map((connector) => ({
        name: connector.connector_name,
        gatewayId: connector.merchant_connector_id,
      }))
  }

  const seen = new Set<string>()
  return [...suggestions, ...ROUTABLE_CONNECTORS]
    .filter((name) => {
      if (!name || picked.has(name) || seen.has(name)) return false
      seen.add(name)
      return true
    })
    .map((name) => ({ name }))
}
