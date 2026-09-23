import type { DeprecatedField, StabilityLevel } from '../lib/api'

export type { StabilityLevel }

interface StabilityBadgeProps {
  stability?: StabilityLevel | null
  size?: 'xs' | 'sm' | 'md' | 'lg'
}

const stabilityLabels: Record<StabilityLevel, string> = {
  'stable': 'Stable',
  'development': 'Development',
  'alpha': 'Alpha',
  'beta': 'Beta',
  'release_candidate': 'Release Candidate',
}

export function StabilityBadge({ stability, size }: StabilityBadgeProps) {
  if (!stability) return null
  const badgeClass = {
    'stable': 'badge-success',
    'development': 'badge-warning',
    'alpha': 'badge-info',
    'beta': 'badge-info',
    'release_candidate': 'badge-accent',
  }[stability] || 'badge-ghost'

  const sizeClass = size ? ` badge-${size}` : ''
  const label = stabilityLabels[stability] || stability

  return <span className={`badge ${badgeClass}${sizeClass}`}>{label}</span>
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

interface DeprecatedBadgeProps {
  deprecated?: DeprecatedField | null
  size?: 'xs' | 'sm' | 'md' | 'lg'
}

export function DeprecatedBadge({ deprecated, size }: DeprecatedBadgeProps) {
  if (!deprecated) return null

  const sizeClass = size ? ` badge-${size}` : ''
  let title = 'Deprecated'
  if (typeof deprecated === 'object') {
    if (deprecated.reason && deprecated.note) {
      title = `Deprecated (${deprecated.reason}): ${deprecated.note}`
    } else if (deprecated.note) {
      title = `Deprecated: ${deprecated.note}`
    } else if (deprecated.reason) {
      title = `Deprecated (${deprecated.reason})`
    }
  }

  return (
    <span className={`badge badge-error${sizeClass}`} title={title}>
      Deprecated
    </span>
  )
}
