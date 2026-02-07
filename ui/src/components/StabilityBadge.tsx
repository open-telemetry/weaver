import type { StabilityFilter } from '../lib/api'

export type StabilityLevel = Exclude<StabilityFilter, null>

interface StabilityBadgeProps {
  stability?: StabilityLevel | null
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

  const label = {
    'stable': 'Stable',
    'development': 'Development',
    'alpha': 'Alpha',
    'beta': 'Beta', 
    'release_candidate': 'Release Candidate',
    'deprecated': 'Deprecated',
  }[stability] || stability

  return stability ? (
    <span className={`badge ${badgeClass}`}>{label}</span>
  ) : null
}
