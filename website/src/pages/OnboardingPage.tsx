import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { ArrowRight, Building2, Loader2 } from 'lucide-react'
import { useAuthStore, MerchantInfo } from '../store/authStore'
import { useMerchantStore } from '../store/merchantStore'
import { apiFetch } from '../lib/api'
import { ErrorMessage } from '../components/ui/ErrorMessage'

interface CreateMerchantResponse {
  token: string
  merchant_id: string
  merchant_name: string
  merchants: MerchantInfo[]
}

function Pill({ children }: { children: React.ReactNode }) {
  return (
    <span className="inline-flex items-center rounded-full border border-slate-200 bg-white px-4 py-2 text-sm font-medium text-slate-700 dark:border-[#2a2d35] dark:bg-[#111318] dark:text-[#d0d7e2]">
      {children}
    </span>
  )
}

function Field({
  label,
  children,
  footer,
}: {
  label: string
  children: React.ReactNode
  footer?: string
}) {
  return (
    <div className="space-y-3">
      <label className="block text-xs font-semibold uppercase tracking-[0.24em] text-slate-500 dark:text-[#8d95a3]">
        {label}
      </label>
      {children}
      {footer ? (
        <p className="text-sm leading-6 text-slate-500 dark:text-[#707786]">{footer}</p>
      ) : null}
    </div>
  )
}

export function OnboardingPage() {
  const navigate = useNavigate()
  const { updateMerchant } = useAuthStore()
  const { setMerchantId } = useMerchantStore()
  const assetBaseUrl = import.meta.env.BASE_URL

  const [merchantName, setMerchantName] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setLoading(true)

    try {
      const res = await apiFetch<CreateMerchantResponse>('/onboarding/merchant', {
        method: 'POST',
        body: JSON.stringify({ merchant_name: merchantName }),
      })

      updateMerchant(res.token, res.merchant_id, res.merchants)
      setMerchantId(res.merchant_id)
      navigate('/', { replace: true })
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Something went wrong'
      const match = msg.match(/API error \d+: (.+)/)
      if (match) {
        try {
          const parsed = JSON.parse(match[1])
          setError(parsed.message ?? msg)
        } catch {
          setError(match[1])
        }
      } else {
        setError(msg)
      }
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="dark relative min-h-screen overflow-hidden bg-white text-slate-900 dark:bg-[#030507] dark:text-white">
      <div className="pointer-events-none absolute inset-0 bg-[linear-gradient(180deg,_rgba(255,255,255,1),_rgba(248,250,252,1))] dark:bg-[linear-gradient(180deg,_rgba(3,5,7,1),_rgba(5,8,12,1))]" />
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_18%_18%,_rgba(59,130,246,0.06),_transparent_24%),radial-gradient(circle_at_78%_20%,_rgba(14,165,233,0.04),_transparent_18%),radial-gradient(circle_at_50%_100%,_rgba(14,165,233,0.03),_transparent_24%)] dark:bg-[radial-gradient(circle_at_20%_22%,_rgba(56,189,248,0.05),_transparent_24%),radial-gradient(circle_at_74%_18%,_rgba(59,130,246,0.04),_transparent_18%),radial-gradient(circle_at_50%_100%,_rgba(14,165,233,0.03),_transparent_24%)]" />
      <div className="pointer-events-none absolute inset-0 opacity-[0.05] dark:opacity-[0.08] [background-image:linear-gradient(rgba(148,163,184,0.08)_1px,transparent_1px),linear-gradient(90deg,rgba(148,163,184,0.08)_1px,transparent_1px)] [background-size:56px_56px]" />

      <div className="relative z-10 grid min-h-screen lg:grid-cols-[1.08fr_0.92fr]">
        <section className="flex min-h-[42vh] flex-col border-b border-slate-200 dark:border-white/6 lg:min-h-screen lg:border-b-0 lg:border-r lg:border-[#1d1d23] px-6 py-8 sm:px-10 lg:px-14 lg:py-12 xl:px-16">
          <div className="pt-2">
            <img
              src={`${assetBaseUrl}logo/decision-engine-dark.svg`}
              alt="Juspay Decision Engine"
              className="h-10 w-auto sm:h-11"
            />
          </div>

          <div className="flex flex-1 items-center py-12 lg:py-0">
            <div className="max-w-[620px] space-y-9">
              <div className="space-y-6">
                <h1 className="max-w-[10ch] text-[clamp(3.8rem,6.8vw,6rem)] font-semibold leading-[0.92] tracking-[-0.065em] text-slate-950 dark:text-white">
                  Configure the merchant workspace before first traffic.
                </h1>
                <p className="max-w-[38rem] text-lg leading-9 text-slate-600 dark:text-[#9aa4b6] sm:text-[1.3rem]">
                  Create the merchant shell once, then move straight into routing rules,
                  analytics, and payment audit from the same control plane.
                </p>
              </div>

              <div className="flex flex-wrap gap-3">
                <Pill>Merchant workspace</Pill>
                <Pill>Routing controls</Pill>
                <Pill>Analytics ready</Pill>
              </div>
            </div>
          </div>
        </section>

        <section className="flex min-h-[58vh] items-center justify-center px-6 py-10 sm:px-10 lg:min-h-screen lg:px-14 lg:py-12 xl:px-16">
          <div className="w-full max-w-[540px]">
            <div className="text-center">
              <h2 className="text-[clamp(2.15rem,3.6vw,3.05rem)] font-semibold tracking-[-0.05em] text-slate-950 dark:text-white">
                Create your merchant
              </h2>
              <p className="mt-3 text-base text-slate-500 dark:text-[#8a94a7]">
                Set up the merchant account that will own your Decision Engine workspace.
              </p>
            </div>

            <div className="mt-10 border-t border-slate-200 pt-10 dark:border-[#1d1d23]">
              <form onSubmit={handleSubmit} className="space-y-6">
                <Field
                  label="Merchant name"
                  footer="You can adjust downstream rules, connectors, and analytics after the workspace is created."
                >
                  <div className="relative">
                    <Building2
                      size={16}
                      className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-slate-500 dark:text-[#798191]"
                    />
                    <input
                      type="text"
                      required
                      autoFocus
                      value={merchantName}
                      onChange={(e) => setMerchantName(e.target.value)}
                      placeholder="e.g. Acme Corp"
                      className="h-16 w-full rounded-2xl border border-slate-200 bg-white pl-12 pr-5 text-[15px] text-slate-900 outline-none transition-all placeholder:text-slate-400 focus:border-blue-500 focus:ring-2 focus:ring-blue-500/20 dark:border-[#2a2d35] dark:bg-[#1a1d25] dark:text-white dark:placeholder:text-[#6e7684] dark:focus:border-blue-500"
                    />
                  </div>
                </Field>

                <ErrorMessage error={error} />

                <button
                  type="submit"
                  disabled={loading}
                  className="group inline-flex h-14 w-full items-center justify-center gap-2 rounded-2xl bg-[linear-gradient(90deg,#4371ff_0%,#3a63f4_100%)] px-5 text-sm font-semibold text-white transition-all duration-200 hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-60"
                >
                  {loading ? (
                    <>
                      <Loader2 size={16} className="animate-spin" />
                      Creating workspace
                    </>
                  ) : (
                    <>
                      Create merchant
                      <ArrowRight
                        size={16}
                        className="transition-transform duration-200 group-hover:translate-x-0.5"
                      />
                    </>
                  )}
                </button>
              </form>

              <div className="mt-10 border-t border-slate-200 pt-6 text-center text-xs text-slate-500 dark:border-[#1d1d23] dark:text-[#667085]">
                <p>Workspace ownership and merchant-scoped access will be linked to your current operator session.</p>
                <p className="mt-4 text-slate-400 dark:text-[#525866]">Juspay Decision Engine</p>
              </div>
            </div>
          </div>
        </section>
      </div>
    </div>
  )
}
