import { useEffect, useRef, useState } from 'react'
import { QRCodeSVG } from 'qrcode.react'
import { ArrowRight, Download, KeyRound, Loader2, MailCheck, ShieldCheck } from 'lucide-react'
import { apiErrorMessage, apiFetch } from '../../lib/api'
import type { AuthResponse } from '../../hooks/useCompleteSignIn'
import { CopyButton } from '../ui/CopyButton'
import { ErrorMessage } from '../ui/ErrorMessage'
import { AUTH_PRIMARY_BUTTON } from './AuthCardLayout'

export type TwoFactorStep = 'verify_email' | 'setup_totp' | 'verify_totp'

/** What the backend returns in place of a session while a second factor is outstanding. */
export interface TwoFactorChallenge {
  two_factor_required: true
  flow_token: string
  next_step: TwoFactorStep
  email: string
}

export function isTwoFactorChallenge(value: unknown): value is TwoFactorChallenge {
  return (
    typeof value === 'object' &&
    value !== null &&
    (value as { two_factor_required?: unknown }).two_factor_required === true
  )
}

interface TotpSetup {
  secret: string
  otpauth_url: string
}

interface TotpEnabled {
  recovery_codes: string[]
  session: AuthResponse
}

const SECONDARY_LINK =
  'text-sm font-medium text-brand-600 transition-colors hover:text-brand-700 disabled:opacity-50 dark:text-brand-500 dark:hover:text-brand-100'

/**
 * The second half of sign-in: whichever step the backend holds this login at, through to a
 * session. `onAuthenticated` receives that session; `onCancel` returns to the password screen.
 */
export function TwoFactorFlow({
  challenge,
  onAuthenticated,
  onCancel,
}: {
  challenge: TwoFactorChallenge
  onAuthenticated: (session: AuthResponse) => Promise<void> | void
  onCancel: () => void
}) {
  switch (challenge.next_step) {
    case 'verify_totp':
      return (
        <VerifyCodeStep challenge={challenge} onAuthenticated={onAuthenticated} onCancel={onCancel} />
      )
    case 'verify_email':
      return <VerifyEmailStep challenge={challenge} onCancel={onCancel} />
    case 'setup_totp':
      return <SetupStep challenge={challenge} onAuthenticated={onAuthenticated} />
  }
}

function StepHeading({
  icon: Icon,
  title,
  children,
}: {
  icon: typeof ShieldCheck
  title: string
  children: React.ReactNode
}) {
  return (
    <div className="space-y-3 text-center">
      <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-full bg-brand-500/10">
        <Icon size={24} className="text-brand-600" />
      </div>
      <h3 className="text-2xl font-semibold tracking-tight text-slate-950 dark:text-white">{title}</h3>
      <p className="mx-auto max-w-[44ch] text-sm leading-6 text-slate-500 dark:text-[#8a94a7]">
        {children}
      </p>
    </div>
  )
}

function CodeInput({
  value,
  onChange,
  recovery = false,
}: {
  value: string
  onChange: (value: string) => void
  recovery?: boolean
}) {
  return (
    <input
      autoFocus
      required
      value={value}
      onChange={(e) =>
        onChange(recovery ? e.target.value : e.target.value.replace(/\D/g, '').slice(0, 6))
      }
      inputMode={recovery ? 'text' : 'numeric'}
      autoComplete="one-time-code"
      placeholder={recovery ? 'xxxx-xxxx' : '123456'}
      aria-label={recovery ? 'Recovery code' : 'Authentication code'}
      className="h-14 w-full rounded-2xl border border-slate-200 bg-white px-4 text-center font-mono text-lg tracking-[0.3em] text-slate-950 outline-none transition placeholder:tracking-normal placeholder:text-slate-400 focus:border-brand-500 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-white"
    />
  )
}

function SubmitButton({ loading, children }: { loading: boolean; children: React.ReactNode }) {
  return (
    <button type="submit" disabled={loading} className={`${AUTH_PRIMARY_BUTTON} h-14 disabled:opacity-60`}>
      {loading ? <Loader2 size={16} className="animate-spin" /> : children}
    </button>
  )
}

function VerifyCodeStep({
  challenge,
  onAuthenticated,
  onCancel,
}: {
  challenge: TwoFactorChallenge
  onAuthenticated: (session: AuthResponse) => Promise<void> | void
  onCancel: () => void
}) {
  const [useRecovery, setUseRecovery] = useState(false)
  const [code, setCode] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setLoading(true)
    try {
      const session = await apiFetch<AuthResponse>(
        useRecovery ? '/auth/2fa/recovery-code/verify' : '/auth/2fa/totp/verify',
        {
          method: 'POST',
          body: JSON.stringify(
            useRecovery
              ? { flow_token: challenge.flow_token, recovery_code: code }
              : { flow_token: challenge.flow_token, totp: code },
          ),
        },
      )
      await onAuthenticated(session)
    } catch (err) {
      setError(apiErrorMessage(err))
      setCode('')
    } finally {
      setLoading(false)
    }
  }

  return (
    <form onSubmit={submit} className="space-y-6">
      <StepHeading icon={useRecovery ? KeyRound : ShieldCheck} title="Two-factor authentication">
        {useRecovery
          ? 'Enter one of the recovery codes you saved when you set up two-factor authentication. Each code works once.'
          : `Enter the 6-digit code from your authenticator app for ${challenge.email}.`}
      </StepHeading>
      <CodeInput key={String(useRecovery)} value={code} onChange={setCode} recovery={useRecovery} />
      <ErrorMessage error={error} />
      <SubmitButton loading={loading}>
        Verify
        <ArrowRight size={16} />
      </SubmitButton>
      <div className="flex items-center justify-between">
        <button type="button" onClick={onCancel} className={SECONDARY_LINK}>
          Back to sign in
        </button>
        <button
          type="button"
          onClick={() => {
            setUseRecovery((value) => !value)
            setCode('')
            setError(null)
          }}
          className={SECONDARY_LINK}
        >
          {useRecovery ? 'Use authenticator app' : 'Use a recovery code'}
        </button>
      </div>
    </form>
  )
}

function VerifyEmailStep({
  challenge,
  onCancel,
}: {
  challenge: TwoFactorChallenge
  onCancel: () => void
}) {
  const [sending, setSending] = useState(false)
  const [notice, setNotice] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  async function resend() {
    setError(null)
    setNotice(null)
    setSending(true)
    try {
      const res = await apiFetch<{ message: string }>('/auth/2fa/email/resend', {
        method: 'POST',
        body: JSON.stringify({ flow_token: challenge.flow_token }),
      })
      setNotice(res.message)
    } catch (err) {
      setError(apiErrorMessage(err))
    } finally {
      setSending(false)
    }
  }

  return (
    <div className="space-y-6">
      <StepHeading icon={MailCheck} title="Verify your email">
        Two-factor authentication is required for your account. To set it up, open the link we
        sent to <span className="font-medium text-slate-700 dark:text-slate-200">{challenge.email}</span>.
        Setup continues from that link, and it expires in 15 minutes.
      </StepHeading>
      {notice ? (
        <div className="rounded-lg border border-sky-500/30 bg-sky-50 px-4 py-3 text-sm text-sky-700 dark:border-sky-500/20 dark:bg-sky-500/8 dark:text-sky-300">
          {notice}
        </div>
      ) : null}
      <ErrorMessage error={error} />
      <div className="flex items-center justify-between">
        <button type="button" onClick={onCancel} className={SECONDARY_LINK}>
          Back to sign in
        </button>
        <button type="button" onClick={resend} disabled={sending} className={SECONDARY_LINK}>
          {sending ? 'Sending…' : 'Resend email'}
        </button>
      </div>
    </div>
  )
}

function SetupStep({
  challenge,
  onAuthenticated,
}: {
  challenge: TwoFactorChallenge
  onAuthenticated: (session: AuthResponse) => Promise<void> | void
}) {
  const [setup, setSetup] = useState<TotpSetup | null>(null)
  const [enabled, setEnabled] = useState<TotpEnabled | null>(null)
  const [code, setCode] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const requested = useRef(false)

  useEffect(() => {
    // Each call replaces the secret, so a re-render must not ask twice and invalidate the QR code
    // the user is already scanning.
    if (requested.current) return
    requested.current = true
    apiFetch<TotpSetup>('/auth/2fa/totp/setup', {
      method: 'POST',
      body: JSON.stringify({ flow_token: challenge.flow_token }),
    })
      .then(setSetup)
      .catch((err: unknown) => setError(apiErrorMessage(err)))
  }, [challenge.flow_token])

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setLoading(true)
    try {
      const res = await apiFetch<TotpEnabled>('/auth/2fa/totp/enable', {
        method: 'POST',
        body: JSON.stringify({ flow_token: challenge.flow_token, totp: code }),
      })
      setEnabled(res)
    } catch (err) {
      setError(apiErrorMessage(err))
      setCode('')
    } finally {
      setLoading(false)
    }
  }

  if (enabled) {
    return (
      <RecoveryCodes
        codes={enabled.recovery_codes}
        email={challenge.email}
        onDone={() => onAuthenticated(enabled.session)}
      />
    )
  }

  return (
    <form onSubmit={submit} className="space-y-6">
      <StepHeading icon={ShieldCheck} title="Set up your authenticator">
        Scan this QR code with an authenticator app (Google Authenticator, 1Password, Authy…), then
        enter the 6-digit code it shows.
      </StepHeading>
      {setup ? (
        <div className="space-y-4">
          <div className="mx-auto w-fit rounded-2xl border border-slate-200 bg-white p-4 dark:border-[#2a303a]">
            <QRCodeSVG value={setup.otpauth_url} size={176} marginSize={0} />
          </div>
          <div className="text-center text-xs text-slate-500 dark:text-[#8a94a7]">
            Can't scan? Enter this key manually:
            <div className="mt-1 flex items-center justify-center gap-2 font-mono text-sm text-slate-800 dark:text-slate-200">
              <span className="break-all">{setup.secret}</span>
              <CopyButton text={setup.secret} label="Copy setup key" size={14} />
            </div>
          </div>
        </div>
      ) : !error ? (
        <div className="flex justify-center py-10">
          <Loader2 size={24} className="animate-spin text-brand-600" />
        </div>
      ) : null}
      <CodeInput value={code} onChange={setCode} />
      <ErrorMessage error={error} />
      <SubmitButton loading={loading || !setup}>
        Turn on two-factor authentication
        <ArrowRight size={16} />
      </SubmitButton>
    </form>
  )
}

/** Shows recovery codes once, with copy and download, until the user confirms they are saved. */
export function RecoveryCodes({
  codes,
  email,
  onDone,
  doneLabel = 'I have saved these codes — continue',
}: {
  codes: string[]
  email: string
  onDone: () => Promise<void> | void
  doneLabel?: string
}) {
  const [busy, setBusy] = useState(false)
  const text = codes.join('\n')

  function download() {
    const blob = new Blob(
      [`Juspay Decision Engine recovery codes for ${email}\nEach code works once.\n\n${text}\n`],
      { type: 'text/plain' },
    )
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = 'decision-engine-recovery-codes.txt'
    link.click()
    URL.revokeObjectURL(url)
  }

  return (
    <div className="space-y-6">
      <StepHeading icon={KeyRound} title="Save your recovery codes">
        If you lose access to your authenticator app, each of these codes signs you in once. They
        will not be shown again.
      </StepHeading>
      <ul className="grid grid-cols-2 gap-2 rounded-2xl border border-slate-200 bg-slate-50 p-4 font-mono text-sm text-slate-800 dark:border-[#2a303a] dark:bg-[#161b24] dark:text-slate-200">
        {codes.map((code) => (
          <li key={code} className="text-center">
            {code}
          </li>
        ))}
      </ul>
      <div className="flex items-center justify-center gap-6 text-sm">
        <span className="inline-flex items-center gap-1.5 text-slate-600 dark:text-slate-300">
          <CopyButton text={text} label="Copy recovery codes" size={14} /> Copy
        </span>
        <button type="button" onClick={download} className={`${SECONDARY_LINK} inline-flex items-center gap-1.5`}>
          <Download size={14} /> Download
        </button>
      </div>
      <button
        type="button"
        disabled={busy}
        onClick={async () => {
          setBusy(true)
          try {
            await onDone()
          } finally {
            setBusy(false)
          }
        }}
        className={`${AUTH_PRIMARY_BUTTON} h-14 disabled:opacity-60`}
      >
        {busy ? <Loader2 size={16} className="animate-spin" /> : doneLabel}
      </button>
    </div>
  )
}
