import useSWR from 'swr'
import { apiPost, fetcher } from '../lib/api'
import { DebitRoutingFlagResponse } from '../types/api'

export function useDebitRoutingFlag(merchantId?: string) {
  const path = merchantId ? `/merchant-account/${merchantId}/debit-routing` : null
  const { data, error, isLoading, mutate } = useSWR<DebitRoutingFlagResponse>(path, fetcher)

  async function setDebitRoutingEnabled(enabled: boolean) {
    if (!merchantId || !path) {
      throw new Error('Set a merchant ID first')
    }

    const response = await apiPost<DebitRoutingFlagResponse>(path, { enabled })
    await mutate(response, false)
    return response
  }

  return {
    data,
    error,
    isLoading,
    isEnabled: Boolean(data?.debit_routing_enabled),
    mutate,
    setDebitRoutingEnabled,
  }
}
