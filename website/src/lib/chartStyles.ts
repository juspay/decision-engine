import type { CSSProperties } from 'react'

export const CHART_TOOLTIP_STYLE: CSSProperties = {
  backgroundColor: 'var(--chart-tooltip-bg)',
  border: '1px solid var(--chart-tooltip-border)',
  borderRadius: '14px',
  color: 'var(--chart-tooltip-text)',
  boxShadow: 'var(--chart-tooltip-shadow)',
}

export const CHART_TOOLTIP_LABEL_STYLE: CSSProperties = {
  color: 'var(--chart-tooltip-label)',
  fontWeight: 600,
  marginBottom: 8,
}

export const CHART_TOOLTIP_ITEM_STYLE: CSSProperties = {
  color: 'var(--chart-tooltip-text)',
}
