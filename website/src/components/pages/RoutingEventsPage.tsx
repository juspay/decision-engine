import { useMemo, useState } from 'react'
import { ArrowRightLeft, BellRing, Target, TrendingDown } from 'lucide-react'
import { describeRoutingEvent, useRoutingEvents } from '../../hooks/useRoutingEvents'
import { AnalyticsRangeValue, RoutingEvent, RoutingEventType } from '../../types/api'
import { Badge } from '../ui/Badge'
import { Card, CardBody, CardHeader } from '../ui/Card'
import { Spinner } from '../ui/Spinner'

const PRESET_OPTIONS: { value: AnalyticsRangeValue; label: string }[] = [
  { value: '1h', label: 'Last 1 hour' },
  { value: '12h', label: 'Last 12 hours' },
  { value: '1d', label: 'Last 1 day' },
  { value: '1w', label: 'Last 1 week' },
]

const EVENT_TYPE_META: Record<
  RoutingEventType,
  { label: string; badge: 'blue' | 'green' | 'orange'; icon: React.ElementType }
> = {
  leader_changed: { label: 'Leader change', badge: 'blue', icon: ArrowRightLeft },
  gateway_entered_auth_band: { label: 'Entered auth band', badge: 'green', icon: Target },
  gateway_exited_auth_band: { label: 'Exited auth band', badge: 'orange', icon: TrendingDown },
}

const ALL_EVENT_TYPES = Object.keys(EVENT_TYPE_META) as RoutingEventType[]

function formatEventDateTime(bucketMs: number) {
  return new Date(bucketMs).toLocaleString([], {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}

function eventDimension(event: RoutingEvent) {
  return [event.payment_method_type, event.payment_method].filter(Boolean).join(' / ') || 'all'
}

export function RoutingEventsPage() {
  const [range, setRange] = useState<AnalyticsRangeValue>('12h')
  const [activeTypes, setActiveTypes] = useState<Set<RoutingEventType>>(new Set(ALL_EVENT_TYPES))
  const { events, isLoading, isUnavailable, markAllSeen, unseenCount } = useRoutingEvents(range)

  const visibleEvents = useMemo(
    () => events.filter((event) => activeTypes.has(event.event_type)),
    [events, activeTypes],
  )

  function toggleType(type: RoutingEventType) {
    setActiveTypes((previous) => {
      const next = new Set(previous)
      if (next.has(type)) {
        next.delete(type)
      } else {
        next.add(type)
      }
      return next
    })
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold text-slate-900 dark:text-white">Routing events</h1>
          <p className="mt-1 text-sm text-slate-500 dark:text-[#8d96aa]">
            Live changes in success-rate based routing — leader flips and gateways entering or exiting the auth band.
          </p>
        </div>
        <div className="flex items-center gap-2">
          {unseenCount > 0 && (
            <button
              onClick={markAllSeen}
              className="text-[13px] font-medium text-brand-600 hover:underline"
            >
              Mark all read ({unseenCount})
            </button>
          )}
          <select
            value={range}
            onChange={(event) => setRange(event.target.value as AnalyticsRangeValue)}
            className="h-9 rounded-lg border border-[#e6e6ee] bg-white px-3 text-[13px] font-medium text-slate-700 dark:border-[#1a1a24] dark:bg-[#121218] dark:text-slate-300"
          >
            {PRESET_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {ALL_EVENT_TYPES.map((type) => {
          const meta = EVENT_TYPE_META[type]
          const active = activeTypes.has(type)
          return (
            <button
              key={type}
              onClick={() => toggleType(type)}
              className={`flex items-center gap-1.5 rounded-full border px-3 py-1.5 text-[12px] font-medium transition-colors ${
                active
                  ? 'border-brand-600/40 bg-brand-50 text-brand-700 dark:border-sky-400/40 dark:bg-sky-400/10 dark:text-sky-200'
                  : 'border-[#e6e6ee] bg-white text-slate-500 hover:bg-slate-50 dark:border-[#1a1a24] dark:bg-[#121218] dark:text-slate-400 dark:hover:bg-[#18181f]'
              }`}
            >
              <meta.icon size={13} />
              {meta.label}
            </button>
          )
        })}
      </div>

      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <p className="text-sm font-semibold text-slate-900 dark:text-white">
              {visibleEvents.length} event{visibleEvents.length === 1 ? '' : 's'}
            </p>
            <p className="text-[12px] text-slate-400">Refreshes every 30 seconds</p>
          </div>
        </CardHeader>
        <CardBody className="p-0">
          {isLoading ? (
            <div className="flex items-center justify-center py-16">
              <Spinner />
            </div>
          ) : isUnavailable ? (
            <div className="px-6 py-14 text-center">
              <BellRing size={28} className="mx-auto text-slate-300 dark:text-slate-600" />
              <p className="mt-3 text-sm font-medium text-slate-700 dark:text-slate-300">
                Routing events are unavailable
              </p>
              <p className="mt-1 text-[13px] text-slate-400">
                The analytics pipeline (Kafka → ClickHouse) is offline or disabled for this environment.
              </p>
            </div>
          ) : visibleEvents.length === 0 ? (
            <div className="px-6 py-14 text-center">
              <BellRing size={28} className="mx-auto text-slate-300 dark:text-slate-600" />
              <p className="mt-3 text-sm font-medium text-slate-700 dark:text-slate-300">
                No routing events in this window
              </p>
              <p className="mt-1 text-[13px] text-slate-400">
                Events appear once success-rate feedback flows through update-gateway-score and gateway
                scores start shifting.
              </p>
            </div>
          ) : (
            <ul className="divide-y divide-slate-100 dark:divide-[#1a1f2a]">
              {visibleEvents.map((event) => {
                const meta = EVENT_TYPE_META[event.event_type]
                return (
                  <li key={event.id} className="flex items-start gap-4 px-6 py-4">
                    <div className="mt-0.5 shrink-0">
                      <Badge variant={meta.badge}>{meta.label}</Badge>
                    </div>
                    <div className="min-w-0 flex-1">
                      <p className="text-sm text-slate-800 dark:text-slate-200">
                        {describeRoutingEvent(event)}
                      </p>
                      <p className="mt-0.5 text-[12px] text-slate-400">
                        {formatEventDateTime(event.bucket_ms)} · dimension {eventDimension(event)}
                        {event.transaction_count != null && ` · ${event.transaction_count} txns in SR window`}
                      </p>
                    </div>
                  </li>
                )
              })}
            </ul>
          )}
        </CardBody>
      </Card>
    </div>
  )
}
