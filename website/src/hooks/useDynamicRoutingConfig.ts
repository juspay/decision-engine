import useSWR from 'swr'
import { fetcher } from '../lib/api'

export interface KeyConfig {
  data_type: 'Enum' | 'Integer' | 'Udf' | 'StrValue' | 'GlobalRef'
  values?: string
  min_value?: number
  max_value?: number
  min_length?: number
  max_length?: number
  exact_length?: number
  regex?: string
}

export interface RoutingConfig {
  keys: {
    keys: Record<string, KeyConfig>
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
  if (!config || !config.keys || !config.keys.keys) {
    return []
  }

  return Object.entries(config.keys.keys).map(([key, keyConfig]) => {
    const parsed: ParsedRoutingKey = {
      key,
      type: keyConfig.data_type.toLowerCase() as ParsedRoutingKey['type'],
    }

    if (keyConfig.values) {
      parsed.values = keyConfig.values.split(',').map(v => v.trim())
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

  const parsedKeys = parseRoutingConfig(data || null)

  // Build a lookup map for easy access
  const keysByName = parsedKeys.reduce((acc, key) => {
    acc[key.key] = key
    return acc
  }, {} as Record<string, ParsedRoutingKey>)

  // Convert to the format expected by components (matching old ROUTING_KEYS structure)
  const routingKeysConfig: Record<string, RoutingKeyConfig> = {}
  
  parsedKeys.forEach((key) => {
    routingKeysConfig[key.key] = {
      type: key.type,
      values: key.values || [],
    }
  })

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
