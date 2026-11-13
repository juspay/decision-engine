interface ErrorMessageProps {
  error: string | null
}

export function ErrorMessage({ error }: ErrorMessageProps) {
  if (!error) return null
  return (
    <div className="rounded-lg border border-red-500/20 bg-red-500/8 px-4 py-3 text-sm text-red-400 font-mono">
      {error}
    </div>
  )
}
