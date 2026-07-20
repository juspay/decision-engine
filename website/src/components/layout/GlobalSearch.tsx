import { useCallback, useEffect, useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Command } from 'cmdk'
import {
  Activity,
  BarChart3,
  BellRing,
  BookOpen,
  Coins,
  Database,
  FlaskConical,
  GitBranch,
  Gauge,
  LineChart,
  Key,
  KeyRound,
  LayoutDashboard,
  Network,
  PieChart,
  Search,
  SlidersHorizontal,
  ToggleRight,
  TrendingUp,
  Users,
} from 'lucide-react'

interface SearchItem {
  /** Route to navigate to on select (may include a query string). */
  to: string
  label: string
  /** Breadcrumb-style hint shown on the right, e.g. "Multi Objective". */
  hint?: string
  icon: React.ElementType
  /** Extra terms used for fuzzy matching but not displayed. */
  keywords?: string[]
}

interface SearchGroup {
  heading: string
  items: SearchItem[]
}

// Curated index of navigable destinations. Mirrors the sidebar plus the
// in-page tabs that are worth jumping to directly. Keep in sync with the
// route table in App.tsx and the links in Sidebar.tsx.
const SEARCH_GROUPS: SearchGroup[] = [
  {
    heading: 'General',
    items: [
      { to: '/', label: 'Overview', icon: LayoutDashboard, keywords: ['home', 'dashboard'] },
      { to: '/analytics', label: 'Analytics', icon: BarChart3, keywords: ['metrics', 'charts', 'stats'] },
      { to: '/analytics', label: 'Multi-objective', hint: 'Analytics', icon: BarChart3, keywords: ['sr', 'success rate', 'multi objective', 'transactions'] },
      { to: '/analytics?view=rule_based', label: 'Rule & Volume', hint: 'Analytics', icon: BarChart3, keywords: ['rule based', 'volume', 'rules', 'analytics'] },
      { to: '/audit', label: 'Decision Audit', icon: Activity, keywords: ['payment', 'inspect', 'logs', 'transactions'] },
      { to: '/audit', label: 'Multi-objective', hint: 'Decision Audit', icon: Activity, keywords: ['sr', 'success rate', 'multi objective', 'transactions'] },
      { to: '/audit?mode=rule_based', label: 'Rule & Volume', hint: 'Decision Audit', icon: Activity, keywords: ['rule based', 'volume', 'rules'] },
      { to: '/audit?mode=debit_routing', label: 'Debit Routing', hint: 'Decision Audit', icon: Network, keywords: ['debit', 'network', 'card'] },
      { to: '/events', label: 'Routing Events', icon: BellRing, keywords: ['stream', 'live', 'feed'] },
    ],
  },
  {
    heading: 'Routing',
    items: [
      { to: '/routing', label: 'Routing Hub', icon: GitBranch, keywords: ['strategies', 'algorithms', 'overview'] },
      { to: '/routing/sr', label: 'Multi Objective', icon: TrendingUp, keywords: ['sr', 'success rate', 'auth rate'] },
      { to: '/routing/sr?tab=autopilot', label: 'Autopilot', hint: 'Multi Objective', icon: Gauge, keywords: ['sr', 'auto', 'automatic'] },
      { to: '/routing/sr?tab=manual', label: 'Manual Config', hint: 'Multi Objective', icon: SlidersHorizontal, keywords: ['sr', 'weights', 'tuning'] },
      { to: '/routing/sr?tab=manual&section=scoring', label: 'Scoring Defaults', hint: 'Manual Config', icon: SlidersHorizontal, keywords: ['sr', 'scoring', 'defaults', 'weights'] },
      { to: '/routing/sr?tab=manual&section=elimination', label: 'Elimination', hint: 'Manual Config', icon: SlidersHorizontal, keywords: ['sr', 'eliminate', 'threshold', 'latency'] },
      { to: '/routing/sr?tab=manual&section=dimensions', label: 'SR Dimensions', hint: 'Manual Config', icon: SlidersHorizontal, keywords: ['sr', 'dimensions', 'split', 'clusters', 'udf'] },
      { to: '/routing/sr?tab=flags', label: 'Feature Flags', hint: 'Multi Objective', icon: ToggleRight, keywords: ['sr', 'toggles', 'settings'] },
      { to: '/routing/sr?tab=cost', label: 'Cost Estimation', hint: 'Multi Objective', icon: Coins, keywords: ['cost', 'pricing', 'fees', 'connectors', 'clusters'] },
      { to: '/routing/sr?tab=cost&section=ingestion', label: 'Data Ingestion', hint: 'Cost Estimation', icon: Database, keywords: ['cost', 'ingest', 'connector', 'settlement', 'upload', 'report'] },
      { to: '/routing/sr?tab=cost&section=data', label: 'Ingested Data', hint: 'Cost Estimation', icon: LineChart, keywords: ['cost', 'coverage', 'history', 'data'] },
      { to: '/routing/sr?tab=cost', label: 'Manual Overrides', hint: 'Cost Estimation', icon: SlidersHorizontal, keywords: ['cost', 'override', 'blended fee', 'fee'] },
      { to: '/routing/rules', label: 'Rule-Based', icon: BookOpen, keywords: ['euclid', 'rules', 'conditions'] },
      { to: '/routing/volume', label: 'Volume Split', icon: PieChart, keywords: ['distribution', 'percentage', 'weights'] },
      { to: '/routing/debit', label: 'Debit Routing', icon: Network, keywords: ['debit', 'network', 'card'] },
      { to: '/routing/ab-testing', label: 'A/B Testing', hint: 'Beta', icon: FlaskConical, keywords: ['experiment', 'split test', 'ab'] },
    ],
  },
  {
    heading: 'Simulation',
    items: [
      { to: '/decisions/simulator', label: 'Decision Simulator', icon: FlaskConical, keywords: ['simulate', 'test', 'preview'] },
    ],
  },
  {
    heading: 'Settings',
    items: [
      { to: '/members', label: 'Members', icon: Users, keywords: ['team', 'users', 'roles', 'invite', 'access'] },
      { to: '/api-keys', label: 'API Keys', icon: Key, keywords: ['tokens', 'credentials', 'secrets'] },
      { to: '/account', label: 'Change Password', hint: 'Account', icon: KeyRound, keywords: ['account', 'password', 'security'] },
    ],
  },
]

// cmdk's default matcher is a loose fuzzy/subsequence score that mis-ranks
// short queries (e.g. "members" scoring "Volume Split" above "Members").
// Use strict substring matching, ranking label hits above keyword hits so the
// auto-selected top result is always the obvious one.
function searchFilter(value: string, search: string, keywords?: string[]): number {
  const q = search.trim().toLowerCase()
  if (!q) return 1
  const label = value.toLowerCase()
  if (label.startsWith(q)) return 1
  if (label.includes(q)) return 0.9
  const kw = (keywords ?? []).join(' ').toLowerCase()
  if (kw.includes(q)) return 0.6
  return 0
}

export function GlobalSearch() {
  const navigate = useNavigate()
  const [open, setOpen] = useState(false)

  // Toggle with ⌘K / Ctrl+K from anywhere in the app.
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.key.toLowerCase() === 'k' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault()
        setOpen((value) => !value)
      }
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [])

  const handleSelect = useCallback(
    (to: string) => {
      setOpen(false)
      navigate(to)
    },
    [navigate],
  )

  return (
    <>
      <SearchTrigger onClick={() => setOpen(true)} />
      <SearchDialog open={open} onOpenChange={setOpen} onSelect={handleSelect} />
    </>
  )
}

function SearchTrigger({ onClick }: { onClick: () => void }) {
  const isMac = useMemo(
    () => typeof navigator !== 'undefined' && /Mac|iPod|iPhone|iPad/.test(navigator.platform),
    [],
  )

  return (
    <button
      type="button"
      onClick={onClick}
      className="group flex h-9 w-full max-w-md items-center gap-2 rounded-lg border border-[#e6e6ee] bg-white px-3 text-slate-400 transition-colors hover:border-slate-300 hover:text-slate-500 dark:border-[#1a1a24] dark:bg-[#121218] dark:hover:border-[#2a2a38] dark:hover:text-slate-300"
      aria-label="Search"
    >
      <Search size={14} className="shrink-0" />
      <span className="flex-1 text-left text-[13px] font-medium">Search…</span>
      <kbd className="hidden shrink-0 items-center gap-0.5 rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5 text-[10px] font-semibold text-slate-400 sm:flex dark:border-[#22262f] dark:bg-[#0a0d12] dark:text-[#6d768a]">
        {isMac ? '⌘' : 'Ctrl'} K
      </kbd>
    </button>
  )
}

function SearchDialog({
  open,
  onOpenChange,
  onSelect,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onSelect: (to: string) => void
}) {
  return (
    <Command.Dialog
      open={open}
      onOpenChange={onOpenChange}
      label="Global search"
      filter={searchFilter}
      loop
      className="flex min-h-0 flex-1 flex-col"
      overlayClassName="fixed inset-0 z-[100] bg-slate-900/40 backdrop-blur-sm dark:bg-black/60"
      contentClassName="fixed left-1/2 top-[12vh] z-[101] flex max-h-[76vh] w-[calc(100vw-2rem)] max-w-xl -translate-x-1/2 flex-col overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-[0_30px_80px_-40px_rgba(15,23,42,0.55)] focus:outline-none dark:border-[#22262f] dark:bg-[#0a0d12]"
    >
      <div className="flex items-center gap-3 border-b border-slate-200 px-4 dark:border-[#22262f]">
        <Search size={17} className="shrink-0 text-slate-400" />
        <Command.Input
          autoFocus
          placeholder="Search pages and settings…"
          className="h-12 w-full bg-transparent text-[15px] text-slate-900 outline-none placeholder:text-slate-400 dark:text-white dark:placeholder:text-[#6d768a]"
        />
      </div>

      <Command.List className="min-h-0 flex-1 overflow-y-auto overflow-x-hidden p-2">
        <Command.Empty className="px-3 py-8 text-center text-sm text-slate-400">
          No results found.
        </Command.Empty>

        {SEARCH_GROUPS.map((group) => (
          <Command.Group
            key={group.heading}
            heading={group.heading}
            className="[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:pb-1 [&_[cmdk-group-heading]]:pt-3 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:font-bold [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-widest [&_[cmdk-group-heading]]:text-slate-400 dark:[&_[cmdk-group-heading]]:text-[#6d768a]"
          >
            {group.items.map((item) => {
              const Icon = item.icon
              return (
                <Command.Item
                  key={`${item.to}::${item.label}`}
                  // Fold the hint into the match value so items that share a
                  // display label (e.g. "Multi-objective" under both Analytics
                  // and Decision Audit) stay unique, and the breadcrumb is searchable.
                  value={item.hint ? `${item.label} ${item.hint}` : item.label}
                  keywords={item.keywords ?? []}
                  onSelect={() => onSelect(item.to)}
                  className="flex cursor-pointer items-center gap-3 rounded-xl px-3 py-2.5 text-[14px] text-slate-600 aria-selected:bg-slate-100 aria-selected:text-slate-900 dark:text-[#8d96aa] dark:aria-selected:bg-white/[0.06] dark:aria-selected:text-white"
                >
                  <Icon size={16} className="shrink-0 text-slate-400" />
                  <span className="flex-1 truncate font-medium">{item.label}</span>
                  {item.hint ? (
                    <span className="shrink-0 text-[12px] text-slate-400 dark:text-[#6d768a]">
                      {item.hint}
                    </span>
                  ) : null}
                </Command.Item>
              )
            })}
          </Command.Group>
        ))}
      </Command.List>
    </Command.Dialog>
  )
}
