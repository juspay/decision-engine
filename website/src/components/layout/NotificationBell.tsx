import { useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { ArrowRightLeft, Bell, Target, TrendingDown } from 'lucide-react'
import { describeRoutingEvent, useRoutingEvents } from '../../hooks/useRoutingEvents'
import { RoutingEventType } from '../../types/api'

const EVENT_ICONS: Record<RoutingEventType, React.ElementType> = {
  leader_changed: ArrowRightLeft,
  gateway_entered_auth_band: Target,
  gateway_exited_auth_band: TrendingDown,
}

const EVENT_ICON_CLASSES: Record<RoutingEventType, string> = {
  leader_changed: 'text-sky-600 dark:text-sky-300',
  gateway_entered_auth_band: 'text-emerald-600 dark:text-emerald-300',
  gateway_exited_auth_band: 'text-amber-600 dark:text-amber-300',
}

function formatEventTime(bucketMs: number) {
  return new Date(bucketMs).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

export function NotificationBell() {
  const navigate = useNavigate()
  const { events, unseenCount, markAllSeen, isUnavailable } = useRoutingEvents('1h')
  const [open, setOpen] = useState(false)
  const dropdownRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  const recentEvents = events.slice(0, 8)

  return (
    <div className="relative" ref={dropdownRef}>
      <button
        onClick={() => setOpen((v) => !v)}
        aria-label="Routing events"
        className="relative flex items-center justify-center h-8 w-8 rounded-lg border border-[#e6e6ee] dark:border-[#1a1a24] bg-white dark:bg-[#121218] hover:bg-slate-50 dark:hover:bg-[#18181f] transition-colors text-slate-700 dark:text-slate-300"
      >
        <Bell size={14} className="text-slate-400" />
        {unseenCount > 0 && (
          <span className="absolute -top-1.5 -right-1.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-brand-600 px-1 text-[9px] font-semibold text-white">
            {unseenCount > 99 ? '99+' : unseenCount}
          </span>
        )}
      </button>

      {open && (
        <div className="absolute right-0 top-10 w-96 bg-white dark:bg-[#0c0c10] border border-[#e6e6ee] dark:border-[#1a1a24] rounded-lg shadow-lg py-1 z-50">
          <div className="flex items-center justify-between px-3 py-1.5">
            <p className="text-[10px] font-semibold uppercase tracking-widest text-slate-400 dark:text-slate-500">
              Routing events
            </p>
            {events.length > 0 && (
              <button
                onClick={markAllSeen}
                className="text-[11px] font-medium text-brand-600 hover:underline"
              >
                Mark all read
              </button>
            )}
          </div>

          {recentEvents.length === 0 ? (
            <p className="px-3 py-4 text-[12px] text-slate-400 dark:text-slate-500">
              {isUnavailable
                ? 'Routing events are unavailable — analytics pipeline is offline.'
                : 'No routing events in the last hour.'}
            </p>
          ) : (
            recentEvents.map((event) => {
              const Icon = EVENT_ICONS[event.event_type]
              return (
                <div
                  key={event.id}
                  className="flex items-start gap-2.5 px-3 py-2 hover:bg-slate-50 dark:hover:bg-[#13131a] transition-colors"
                >
                  <div className="mt-0.5 shrink-0">
                    <Icon size={14} className={EVENT_ICON_CLASSES[event.event_type]} />
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="text-[12px] text-slate-700 dark:text-slate-300">
                      {describeRoutingEvent(event)}
                    </p>
                    <p className="text-[11px] text-slate-400">{formatEventTime(event.bucket_ms)}</p>
                  </div>
                </div>
              )
            })
          )}

          <div className="border-t border-[#e6e6ee] dark:border-[#1a1a24] mt-1 pt-1">
            <button
              onClick={() => { setOpen(false); navigate('/events') }}
              className="w-full px-3 py-2 text-left text-[13px] font-medium text-brand-600 hover:bg-slate-50 dark:hover:bg-[#13131a] transition-colors"
            >
              View all events
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
