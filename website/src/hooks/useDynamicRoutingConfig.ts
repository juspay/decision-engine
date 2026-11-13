import { useMemo } from 'react'
import useSWR from 'swr'
import { fetcher } from '../lib/api'

export interface KeyConfig {
  // backend may send either `type` (snake_case) or `data_type` (legacy)
  type?: 'enum' | 'integer' | 'udf' | 'str_value' | 'global_ref'
  data_type?: 'Enum' | 'Integer' | 'Udf' | 'StrValue' | 'GlobalRef' | 'enum' | 'integer' | 'udf' | 'str_value' | 'global_ref'
  values?: string | string[]
  min_value?: number
  max_value?: number
  min_length?: number
  max_length?: number
  exact_length?: number
  regex?: string
}

export interface RoutingConfig {
  // preferred shape from backend: { keys: { payment_method: {...}, ... } }
  keys?: unknown
  // backward-compat shape
  routing_config?: {
    keys?: unknown
  }
}

export interface ParsedRoutingKey {
  key: string
  type: 'enum' | 'integer' | 'udf' | 'str_value' | 'global_ref'
  values?: string[]
  min_value?: number
  max_value?: number
  min_length?: number
  max_length?: number
  exact_length?: number
  regex?: string
}

export type RoutingKeyType = 'enum' | 'integer' | 'udf' | 'str_value' | 'global_ref'

export interface RoutingKeyConfig {
  type: RoutingKeyType
  values: string[]
}

function parseRoutingConfig(config: RoutingConfig | null): ParsedRoutingKey[] {
  if (!config) {
    return []
  }

  const resolveKeys = (source?: unknown): Record<string, KeyConfig> => {
    if (!source || typeof source !== 'object') return {}

    const nested = (source as { keys?: unknown }).keys
    if (nested && typeof nested === 'object') {
      return nested as Record<string, KeyConfig>
    }

    return source as Record<string, KeyConfig>
  }

  const keysMap = {
    ...resolveKeys(config.keys),
    ...resolveKeys(config.routing_config?.keys),
  }

  if (Object.keys(keysMap).length === 0) {
    return []
  }

  return Object.entries(keysMap).map(([key, keyConfig]) => {
    const normalizedType = (keyConfig.type || keyConfig.data_type || 'str_value')
      .toString()
      .toLowerCase() as ParsedRoutingKey['type']

    const parsed: ParsedRoutingKey = {
      key,
      type: normalizedType,
    }

    if (keyConfig.values) {
      parsed.values = Array.isArray(keyConfig.values)
        ? keyConfig.values.map(v => v.trim())
        : keyConfig.values.split(',').map(v => v.trim())
    }

    if (keyConfig.min_value !== undefined) {
      parsed.min_value = keyConfig.min_value
    }

    if (keyConfig.max_value !== undefined) {
      parsed.max_value = keyConfig.max_value
    }

    if (keyConfig.min_length !== undefined) {
      parsed.min_length = keyConfig.min_length
    }

    if (keyConfig.max_length !== undefined) {
      parsed.max_length = keyConfig.max_length
    }

    if (keyConfig.exact_length !== undefined) {
      parsed.exact_length = keyConfig.exact_length
    }

    if (keyConfig.regex) {
      parsed.regex = keyConfig.regex
    }

    return parsed
  })
}

export function useDynamicRoutingConfig() {
  const { data, error, isLoading } = useSWR<RoutingConfig>(
    '/config/routing-keys',
    fetcher,
    {
      refreshInterval: 0,
      revalidateOnFocus: false,
    }
  )

  const parsedKeys = useMemo(() => parseRoutingConfig(data || null), [data])

  const keysByName = useMemo(
    () =>
      parsedKeys.reduce((acc, key) => {
        acc[key.key] = key
        return acc
      }, {} as Record<string, ParsedRoutingKey>),
    [parsedKeys]
  )

  const routingKeysConfig = useMemo(() => {
    const nextConfig: Record<string, RoutingKeyConfig> = {}

    parsedKeys.forEach((key) => {
      nextConfig[key.key] = {
        type: key.type,
        values: key.values || [],
      }
    })

    return nextConfig
  }, [parsedKeys])

  return {
    config: data,
    keys: parsedKeys,
    keysByName,
    routingKeysConfig,
    isLoading,
    error,
    // Helper to get values for a specific key
    getKeyValues: (keyName: string): string[] => {
      return keysByName[keyName]?.values || []
    },
    // Check if key is integer type
    isIntegerKey: (keyName: string): boolean => {
      return keysByName[keyName]?.type === 'integer'
    },
    // Check if key is enum type
    isEnumKey: (keyName: string): boolean => {
      return keysByName[keyName]?.type === 'enum'
    },
  }
}
