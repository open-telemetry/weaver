import type { StabilityFilter } from '../lib/api'

export type StabilityLevel = Exclude<StabilityFilter, null>

interface StabilityBadgeProps {
  stability?: StabilityLevel | null
}

const stabilityLabels: Record<StabilityLevel, string> = {
  'stable': 'Stable',
  'development': 'Development',
  'alpha': 'Alpha',
  'beta': 'Beta',
  'release_candidate': 'Release Candidate',
  'deprecated': 'Deprecated',
}

export function StabilityBadge({ stability }: StabilityBadgeProps) {
  if (!stability) return null
  const badgeClass = {
    'stable': 'badge-success',
    'development': 'badge-warning',
    'alpha': 'badge-info',
    'beta': 'badge-info',
    'release_candidate': 'badge-accent',
    'deprecated': 'badge-error',
  }[stability] || 'badge-ghost'

  const label = stabilityLabels[stability] || stability

  return <span className={`badge ${badgeClass}`}>{label}</span>
}

/** Compact colored dot for tight layouts (e.g. tree rows); label appears on hover. */
export function StabilityDot({ stability }: StabilityBadgeProps) {
  if (!stability) return null
  const dotClass = {
    'stable': 'bg-success',
    'development': 'bg-warning',
    'alpha': 'bg-info',
    'beta': 'bg-info',
    'release_candidate': 'bg-accent',
    'deprecated': 'bg-error',
  }[stability] || 'bg-base-content/30'

  const label = stabilityLabels[stability] || stability

  return (
    <span
      className={`inline-block h-2 w-2 shrink-0 rounded-full ${dotClass}`}
      title={label}
      role="img"
      aria-label={label}
    />
  )
}
