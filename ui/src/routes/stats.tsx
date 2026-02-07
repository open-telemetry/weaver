import { createRoute, Link } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import { getRegistryStats } from '../lib/api'
import type { RegistryStats } from '../lib/api'
import { Route as RootRoute } from './__root'

export const Route = createRoute({
  getParentRoute: () => RootRoute,
  path: 'stats',
  component: Stats,
})

function Stats() {
  const [stats, setStats] = useState<RegistryStats | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let isMounted = true

    getRegistryStats()
      .then((data) => {
        if (isMounted) {
          setStats(data)
        }
      })
      .catch((err: unknown) => {
        if (isMounted) {
          setError(err instanceof Error ? err.message : 'Unknown error')
        }
      })

    return () => {
      isMounted = false
    }
  }, [])

  const statItems = stats
    ? [
        {
          label: 'Attributes',
          value: stats.counts.attributes,
          description: 'Semantic attributes',
          type: 'attribute',
        },
        {
          label: 'Metrics',
          value: stats.counts.metrics,
          description: 'Metric definitions',
          type: 'metric',
        },
        {
          label: 'Spans',
          value: stats.counts.spans,
          description: 'Span types',
          type: 'span',
        },
        {
          label: 'Events',
          value: stats.counts.events,
          description: 'Event definitions',
          type: 'event',
        },
        {
          label: 'Entities',
          value: stats.counts.entities,
          description: 'Entity types',
          type: 'entity',
        },
      ]
    : []

  return (
    <div className="space-y-6">
      <h1 className="text-3xl font-bold">Registry Stats</h1>

      {error ? (
        <div className="alert alert-error">
          <span>Error loading registry stats: {error}</span>
        </div>
      ) : !stats ? (
        <div className="flex justify-center">
          <span className="loading loading-spinner loading-lg"></span>
        </div>
      ) : (
        <>
          {stats.registry_url ? (
            <p className="text-sm text-base-content/70">
              Source:{' '}
              <a href={stats.registry_url} target="_blank" className="link" rel="noreferrer">
                {stats.registry_url}
              </a>
            </p>
          ) : null}

          <div className="stats stats-vertical lg:stats-horizontal shadow w-full">
            {statItems.map((item) => (
              <Link
                key={item.type}
                to="/search"
                search={{ type: item.type }}
                className="stat hover:bg-base-300 cursor-pointer transition-colors"
              >
                <div className="stat-title">{item.label}</div>
                <div className="stat-value">{item.value}</div>
                <div className="stat-desc">{item.description}</div>
              </Link>
            ))}
          </div>
        </>
      )}
    </div>
  )
}
