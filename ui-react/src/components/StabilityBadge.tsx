type StabilityLevel = 'stable' | 'development' | 'alpha' | 'beta' | 'release_candidate' | 'deprecated'

interface StabilityBadgeProps {
  stability: StabilityLevel
}

export function StabilityBadge({ stability }: StabilityBadgeProps) {
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