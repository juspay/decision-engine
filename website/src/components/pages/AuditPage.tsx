import { useCallback, useEffect, useState } from 'react'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Spinner } from '../ui/Spinner'
import { ErrorMessage } from '../ui/ErrorMessage'
import { apiFetch } from '../../lib/api'
import {
  ScrollText,
  ChevronDown,
  ChevronRight,
  Clock,
  ArrowUpDown,
  Search,
  ChevronLeft,
} from 'lucide-react'

interface EndpointStats {
  endpoint: string
  method: string
  count: number
  avg_latency_ms: number
  error_count: number
  last_hit: string | null
}

interface AuditLogEntry {
  id: string
  timestamp: string
  endpoint: string
  method: string
  request_headers: Record<string, string> | null
  request_body: unknown | null
  response_status: number
  response_body: unknown | null
  latency_ms: number
  merchant_id: string | null
  request_id: string
}

interface PaginatedRequests {
  data: AuditLogEntry[]
  page: number
  per_page: number
  total: number
}

type TimeRange = '1h' | '6h' | '24h' | '7d'
type SortField = 'endpoint' | 'count' | 'avg_latency_ms' | 'error_count' | 'last_hit'
type SortDir = 'asc' | 'desc'

const METHOD_COLORS: Record<string, 'green' | 'blue' | 'red' | 'orange' | 'purple'> = {
  GET: 'blue',
  POST: 'green',
  PUT: 'orange',
  DELETE: 'red',
  PATCH: 'purple',
}

function statusBadgeVariant(status: number): 'green' | 'red' | 'orange' | 'blue' {
  if (status >= 200 && status < 300) return 'green'
  if (status >= 400 && status < 500) return 'orange'
  if (status >= 500) return 'red'
  return 'blue'
}

function formatTimestamp(ts: string): string {
  try {
    const d = new Date(ts)
    return d.toLocaleString()
  } catch {
    return ts
  }
}

function JsonBlock({ data, label }: { data: unknown; label: string }) {
  if (data === null || data === undefined) {
    return (
      <div className="mb-4">
        <p className="text-xs font-semibold text-slate-400 dark:text-[#66666e] uppercase tracking-wider mb-2">{label}</p>
        <p className="text-sm text-slate-500 dark:text-[#55555e] italic">Empty</p>
      </div>
    )
  }
  return (
    <div className="mb-4">
      <p className="text-xs font-semibold text-slate-400 dark:text-[#66666e] uppercase tracking-wider mb-2">{label}</p>
      <pre className="bg-slate-100 dark:bg-[#0a0a0c] rounded-xl p-4 text-xs text-slate-700 dark:text-slate-300 overflow-x-auto max-h-80 overflow-y-auto border border-slate-200 dark:border-[#1c1c1f]">
        {typeof data === 'string' ? data : JSON.stringify(data, null, 2)}
      </pre>
    </div>
  )
}

export function AuditPage() {
  const [range, setRange] = useState<TimeRange>('24h')
  const [searchTerm, setSearchTerm] = useState('')
  const [stats, setStats] = useState<EndpointStats[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [sortField, setSortField] = useState<SortField>('count')
  const [sortDir, setSortDir] = useState<SortDir>('desc')

  // Expanded endpoint state
  const [expandedEndpoint, setExpandedEndpoint] = useState<string | null>(null)
  const [requests, setRequests] = useState<AuditLogEntry[]>([])
  const [requestsLoading, setRequestsLoading] = useState(false)
  const [requestsPage, setRequestsPage] = useState(1)
  const [requestsTotal, setRequestsTotal] = useState(0)

  // Expanded request detail
  const [expandedRequest, setExpandedRequest] = useState<string | null>(null)

  const fetchStats = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const params = new URLSearchParams({ range })
      const data = await apiFetch<EndpointStats[]>(`/audit/stats?${params}`)
      setStats(data ?? [])
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to fetch audit stats')
      setStats([])
    } finally {
      setLoading(false)
    }
  }, [range])

  useEffect(() => {
    fetchStats()
  }, [fetchStats])

  const fetchRequests = useCallback(async (endpoint: string, page: number) => {
    setRequestsLoading(true)
    try {
      const params = new URLSearchParams({
        endpoint,
        range,
        page: String(page),
        per_page: '20',
      })
      const data = await apiFetch<PaginatedRequests>(`/audit/requests?${params}`)
      if (data) {
        setRequests(data.data)
        setRequestsTotal(data.total)
      }
    } catch {
      setRequests([])
    } finally {
      setRequestsLoading(false)
    }
  }, [range])

  const handleEndpointClick = useCallback((key: string, endpoint: string) => {
    if (expandedEndpoint === key) {
      setExpandedEndpoint(null)
      setRequests([])
      setExpandedRequest(null)
    } else {
      setExpandedEndpoint(key)
      setRequestsPage(1)
      setExpandedRequest(null)
      fetchRequests(endpoint, 1)
    }
  }, [expandedEndpoint, fetchRequests])

  const handlePageChange = useCallback((endpoint: string, page: number) => {
    setRequestsPage(page)
    fetchRequests(endpoint, page)
  }, [fetchRequests])

  // Sort and filter
  const filtered = stats.filter(s => {
    if (!searchTerm) return true
    const term = searchTerm.toLowerCase()
    return s.endpoint.toLowerCase().includes(term) || s.method.toLowerCase().includes(term)
  })

  const sorted = [...filtered].sort((a, b) => {
    let cmp = 0
    switch (sortField) {
      case 'endpoint': cmp = a.endpoint.localeCompare(b.endpoint); break
      case 'count': cmp = a.count - b.count; break
      case 'avg_latency_ms': cmp = a.avg_latency_ms - b.avg_latency_ms; break
      case 'error_count': cmp = a.error_count - b.error_count; break
      case 'last_hit': cmp = (a.last_hit ?? '').localeCompare(b.last_hit ?? ''); break
    }
    return sortDir === 'desc' ? -cmp : cmp
  })

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDir(d => d === 'asc' ? 'desc' : 'asc')
    } else {
      setSortField(field)
      setSortDir('desc')
    }
  }

  const totalRequests = stats.reduce((sum, s) => sum + s.count, 0)
  const totalErrors = stats.reduce((sum, s) => sum + s.error_count, 0)
  const avgLatency = stats.length > 0
    ? Math.round(stats.reduce((sum, s) => sum + s.avg_latency_ms, 0) / stats.length)
    : 0

  const totalPages = Math.ceil(requestsTotal / 20)

  return (
    <div className="space-y-8">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 bg-brand-500/10 dark:bg-[#151518] rounded-2xl flex items-center justify-center">
            <ScrollText size={20} className="text-brand-600 dark:text-white" />
          </div>
          <div>
            <h1 className="text-2xl font-bold text-slate-900 dark:text-white">Audit Log</h1>
            <p className="text-sm text-slate-500 dark:text-[#66666e]">API request monitoring and inspection</p>
          </div>
        </div>

        {/* Time range selector */}
        <div className="flex items-center gap-2 bg-slate-100 dark:bg-[#0c0c0e] rounded-xl p-1 border border-slate-200 dark:border-[#1c1c1f]">
          {(['1h', '6h', '24h', '7d'] as TimeRange[]).map(r => (
            <button
              key={r}
              onClick={() => setRange(r)}
              className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition-all ${
                range === r
                  ? 'bg-white dark:bg-[#1c1c1f] text-brand-600 dark:text-white shadow-sm'
                  : 'text-slate-500 dark:text-[#66666e] hover:text-slate-700 dark:hover:text-white'
              }`}
            >
              {r}
            </button>
          ))}
        </div>
      </div>

      {/* Summary cards */}
      <div className="grid grid-cols-4 gap-4">
        <Card>
          <CardBody>
            <p className="text-xs font-semibold text-slate-400 dark:text-[#66666e] uppercase tracking-wider">Total Requests</p>
            <p className="text-2xl font-bold text-slate-900 dark:text-white mt-1">{totalRequests.toLocaleString()}</p>
          </CardBody>
        </Card>
        <Card>
          <CardBody>
            <p className="text-xs font-semibold text-slate-400 dark:text-[#66666e] uppercase tracking-wider">Endpoints</p>
            <p className="text-2xl font-bold text-slate-900 dark:text-white mt-1">{stats.length}</p>
          </CardBody>
        </Card>
        <Card>
          <CardBody>
            <p className="text-xs font-semibold text-slate-400 dark:text-[#66666e] uppercase tracking-wider">Errors</p>
            <p className="text-2xl font-bold text-red-500 mt-1">{totalErrors.toLocaleString()}</p>
          </CardBody>
        </Card>
        <Card>
          <CardBody>
            <p className="text-xs font-semibold text-slate-400 dark:text-[#66666e] uppercase tracking-wider">Avg Latency</p>
            <p className="text-2xl font-bold text-slate-900 dark:text-white mt-1">{avgLatency}ms</p>
          </CardBody>
        </Card>
      </div>

      {/* Main table */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <h2 className="text-base font-bold text-slate-900 dark:text-white">API Endpoints</h2>
            <div className="relative">
              <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400 dark:text-[#55555e]" />
              <input
                type="text"
                placeholder="Filter endpoints..."
                value={searchTerm}
                onChange={e => setSearchTerm(e.target.value)}
                className="pl-9 pr-4 py-2 text-sm rounded-xl bg-white dark:bg-[#0a0a0c] border border-slate-200 dark:border-[#1c1c1f] text-slate-900 dark:text-white placeholder:text-slate-400 dark:placeholder:text-[#55555e] focus:outline-none focus:ring-2 focus:ring-brand-500/30 w-64"
              />
            </div>
          </div>
        </CardHeader>
        <CardBody className="p-0">
          {loading ? (
            <div className="flex justify-center py-12"><Spinner /></div>
          ) : error ? (
            <div className="p-6"><ErrorMessage error={error} /></div>
          ) : sorted.length === 0 ? (
            <div className="text-center py-12 text-slate-500 dark:text-[#66666e]">
              No audit data found for the selected time range
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-slate-200 dark:border-[#1c1c1f]">
                    <th className="w-8 px-4 py-3"></th>
                    <SortableHeader field="endpoint" label="Endpoint" current={sortField} dir={sortDir} onSort={handleSort} />
                    <th className="px-4 py-3 text-left text-xs font-semibold text-slate-400 dark:text-[#66666e] uppercase tracking-wider">Method</th>
                    <SortableHeader field="count" label="Hits" current={sortField} dir={sortDir} onSort={handleSort} />
                    <SortableHeader field="avg_latency_ms" label="Avg Latency" current={sortField} dir={sortDir} onSort={handleSort} />
                    <SortableHeader field="error_count" label="Errors" current={sortField} dir={sortDir} onSort={handleSort} />
                    <SortableHeader field="last_hit" label="Last Hit" current={sortField} dir={sortDir} onSort={handleSort} />
                  </tr>
                </thead>
                <tbody>
                  {sorted.map(s => {
                    const key = `${s.endpoint}:${s.method}`
                    const isExpanded = expandedEndpoint === key
                    const errorRate = s.count > 0 ? ((s.error_count / s.count) * 100).toFixed(1) : '0.0'

                    return (
                      <EndpointRow
                        key={key}
                        stat={s}
                        rowKey={key}
                        isExpanded={isExpanded}
                        errorRate={errorRate}
                        onToggle={() => handleEndpointClick(key, s.endpoint)}
                        requests={requests}
                        requestsLoading={requestsLoading}
                        requestsPage={requestsPage}
                        totalPages={totalPages}
                        requestsTotal={requestsTotal}
                        expandedRequest={expandedRequest}
                        onExpandRequest={setExpandedRequest}
                        onPageChange={(p) => handlePageChange(s.endpoint, p)}
                      />
                    )
                  })}
                </tbody>
              </table>
            </div>
          )}
        </CardBody>
      </Card>
    </div>
  )
}

function SortableHeader({
  field, label, current, dir: _dir, onSort
}: {
  field: SortField; label: string; current: SortField; dir: SortDir; onSort: (f: SortField) => void
}) {
  return (
    <th
      className="px-4 py-3 text-left text-xs font-semibold text-slate-400 dark:text-[#66666e] uppercase tracking-wider cursor-pointer hover:text-slate-600 dark:hover:text-white select-none"
      onClick={() => onSort(field)}
    >
      <span className="inline-flex items-center gap-1">
        {label}
        <ArrowUpDown size={12} className={current === field ? 'text-brand-500' : 'opacity-30'} />
      </span>
    </th>
  )
}

function EndpointRow({
  stat, rowKey: _rowKey, isExpanded, errorRate, onToggle,
  requests, requestsLoading, requestsPage, totalPages, requestsTotal,
  expandedRequest, onExpandRequest, onPageChange,
}: {
  stat: EndpointStats
  rowKey: string
  isExpanded: boolean
  errorRate: string
  onToggle: () => void
  requests: AuditLogEntry[]
  requestsLoading: boolean
  requestsPage: number
  totalPages: number
  requestsTotal: number
  expandedRequest: string | null
  onExpandRequest: (id: string | null) => void
  onPageChange: (page: number) => void
}) {
  return (
    <>
      <tr
        className="border-b border-slate-100 dark:border-[#151518] hover:bg-slate-50 dark:hover:bg-[#0c0c0e] cursor-pointer transition-colors"
        onClick={onToggle}
      >
        <td className="px-4 py-3">
          {isExpanded
            ? <ChevronDown size={14} className="text-slate-400" />
            : <ChevronRight size={14} className="text-slate-400" />}
        </td>
        <td className="px-4 py-3 font-mono text-xs text-slate-900 dark:text-white">{stat.endpoint}</td>
        <td className="px-4 py-3"><Badge variant={METHOD_COLORS[stat.method] ?? 'gray'}>{stat.method}</Badge></td>
        <td className="px-4 py-3 font-semibold text-slate-900 dark:text-white">{stat.count.toLocaleString()}</td>
        <td className="px-4 py-3 text-slate-600 dark:text-slate-400">{stat.avg_latency_ms}ms</td>
        <td className="px-4 py-3">
          <span className={stat.error_count > 0 ? 'text-red-500 font-semibold' : 'text-slate-500 dark:text-[#66666e]'}>
            {stat.error_count.toLocaleString()} ({errorRate}%)
          </span>
        </td>
        <td className="px-4 py-3 text-slate-500 dark:text-[#66666e] text-xs">
          {stat.last_hit ? (
            <span className="inline-flex items-center gap-1">
              <Clock size={12} />
              {formatTimestamp(stat.last_hit)}
            </span>
          ) : '—'}
        </td>
      </tr>

      {isExpanded && (
        <tr>
          <td colSpan={7} className="bg-slate-50 dark:bg-[#0a0a0c] border-b border-slate-200 dark:border-[#1c1c1f]">
            <div className="px-6 py-4">
              <div className="flex items-center justify-between mb-3">
                <h3 className="text-sm font-bold text-slate-700 dark:text-slate-300">
                  Recent Requests ({requestsTotal.toLocaleString()} total)
                </h3>
                {totalPages > 1 && (
                  <div className="flex items-center gap-2">
                    <button
                      onClick={(e) => { e.stopPropagation(); onPageChange(requestsPage - 1) }}
                      disabled={requestsPage <= 1}
                      className="p-1 rounded-lg text-slate-400 hover:text-slate-700 dark:hover:text-white disabled:opacity-30 disabled:cursor-not-allowed"
                    >
                      <ChevronLeft size={16} />
                    </button>
                    <span className="text-xs text-slate-500 dark:text-[#66666e]">
                      Page {requestsPage} of {totalPages}
                    </span>
                    <button
                      onClick={(e) => { e.stopPropagation(); onPageChange(requestsPage + 1) }}
                      disabled={requestsPage >= totalPages}
                      className="p-1 rounded-lg text-slate-400 hover:text-slate-700 dark:hover:text-white disabled:opacity-30 disabled:cursor-not-allowed"
                    >
                      <ChevronRight size={16} />
                    </button>
                  </div>
                )}
              </div>

              {requestsLoading ? (
                <div className="flex justify-center py-6"><Spinner /></div>
              ) : requests.length === 0 ? (
                <p className="text-sm text-slate-500 dark:text-[#66666e] py-4">No requests found</p>
              ) : (
                <div className="space-y-1">
                  {requests.map(req => {
                    const isReqExpanded = expandedRequest === req.id
                    return (
                      <div key={req.id}>
                        <div
                          className="flex items-center gap-4 px-4 py-2.5 rounded-xl hover:bg-white dark:hover:bg-[#151518] cursor-pointer transition-colors"
                          onClick={(e) => { e.stopPropagation(); onExpandRequest(isReqExpanded ? null : req.id) }}
                        >
                          {isReqExpanded
                            ? <ChevronDown size={12} className="text-slate-400 shrink-0" />
                            : <ChevronRight size={12} className="text-slate-400 shrink-0" />}
                          <span className="text-xs text-slate-500 dark:text-[#66666e] w-40 shrink-0">{formatTimestamp(req.timestamp)}</span>
                          <span className="text-xs font-mono text-slate-600 dark:text-slate-400 w-48 shrink-0 truncate">{req.request_id}</span>
                          <Badge variant={statusBadgeVariant(req.response_status)}>{req.response_status}</Badge>
                          <span className="text-xs text-slate-500 dark:text-[#66666e]">{req.latency_ms}ms</span>
                          {req.merchant_id && (
                            <span className="text-xs text-slate-400 dark:text-[#55555e] ml-auto">merchant: {req.merchant_id}</span>
                          )}
                        </div>

                        {isReqExpanded && (
                          <div className="ml-10 mr-4 mt-2 mb-4 p-4 bg-white dark:bg-[#0c0c0e] rounded-xl border border-slate-200 dark:border-[#1c1c1f]" onClick={e => e.stopPropagation()}>
                            <JsonBlock data={req.request_headers} label="Request Headers" />
                            <JsonBlock data={req.request_body} label="Request Body" />
                            <JsonBlock data={req.response_body} label="Response Body" />
                          </div>
                        )}
                      </div>
                    )
                  })}
                </div>
              )}
            </div>
          </td>
        </tr>
      )}
    </>
  )
}
