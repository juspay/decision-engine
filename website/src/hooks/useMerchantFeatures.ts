import useSWR from 'swr'
import { apiPost, fetcher } from '../lib/api'
import { MerchantFeaturesResponse } from '../types/api'

export type KnownFeature =
  | 'gsm-scoring-filter'
  | 'explore-exploit-srv3'
  | 'ab-test-real-payments'
  | 'multi-objective-routing'

export function useMerchantFeatures(merchantId?: string) {
  const path = merchantId ? `/merchant-account/${merchantId}/features` : null
  const { data, error, isLoading, mutate } = useSWR<MerchantFeaturesResponse>(path, fetcher, {
    revalidateOnFocus: false,
    dedupingInterval: 300_000,
  })

  function isEnabled(feature: KnownFeature): boolean {
    return data?.features.find((f) => f.feature === feature)?.enabled ?? false
  }

  async function setFeatureEnabled(feature: KnownFeature, enabled: boolean) {
    if (!merchantId) throw new Error('Set a merchant ID first')
    const updated = await apiPost<MerchantFeaturesResponse>(
      `/merchant-account/${merchantId}/features/${feature}`,
      { enabled },
    )
    await mutate(updated, false)
    return updated
  }

  return {
    data,
    error,
    isLoading,
    isEnabled,
    setFeatureEnabled,
  }
}
