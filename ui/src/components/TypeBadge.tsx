import type { SearchResult } from '../lib/api'

const typeClasses: Record<SearchResult['result_type'], string> = {
  attribute: 'badge-primary',
  metric: 'badge-secondary',
  span: 'badge-accent',
  event: 'badge-info',
  entity: 'badge-success',
}

interface TypeBadgeProps {
  type: SearchResult['result_type']
  size?: 'xs' | 'sm' | 'md'
}

/** Colored badge for a result type, shared by the search list and tree views. */
export function TypeBadge({ type, size = 'md' }: TypeBadgeProps) {
  const sizeClass = size === 'xs' ? ' badge-xs' : size === 'sm' ? ' badge-sm' : ''
  return (
    <span className={`badge badge-soft ${typeClasses[type] ?? 'badge-ghost'}${sizeClass}`}>
      {type}
    </span>
  )
}
