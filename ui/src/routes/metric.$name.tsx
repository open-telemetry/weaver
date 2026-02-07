import { createRoute, Link } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import { getMetric, type MetricAttribute, type MetricResponse } from '../lib/api'
import { Route as RootRoute } from './__root'
import { StabilityBadge } from '../components/StabilityBadge'
import { Markdown } from '../components/Markdown'
import { InlineMarkdown } from '../components/InlineMarkdown'

export const Route = createRoute({
  getParentRoute: () => RootRoute,
  path: 'metric/$name',
  component: MetricDetail,
})

function formatRequirementLevel(requirement_level: MetricAttribute['requirement_level']): { label: string; badgeClass: string } {
  if (typeof requirement_level === 'string') {
    return {
      label: requirement_level,
      badgeClass: requirement_level === 'required' ? 'badge-error' : 'badge',
    }
  }
  if (requirement_level && typeof requirement_level === 'object' && 'conditionally_required' in requirement_level) {
    return {
      label: 'conditionally required',
      badgeClass: 'badge-warning',
    }
  }
  return {
    label: 'optional',
    badgeClass: 'badge',
  }
}

function formatType(type: MetricAttribute['r#type']): string {
  if (typeof type === 'string') return type
  if (type && typeof type === 'object' && 'members' in type) {
    return 'enum'
  }
  return JSON.stringify(type)
}

function MetricDetail() {
  const { name } = Route.useParams()
  const [data, setData] = useState<MetricResponse | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    let isMounted = true

    getMetric(name)
      .then((responseData) => {
        if (isMounted) {
          setData(responseData)
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
  }, [name])

  function copyToClipboard(text: string) {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    })
  }

  return (
    <div className="space-y-4">
      {error ? (
        <div className="alert alert-error">
          <span>Error: {error}</span>
        </div>
      ) : !data ? (
        <div className="flex justify-center">
          <span className="loading loading-spinner loading-lg"></span>
        </div>
      ) : (
        <>
          <div className="flex items-center gap-4 flex-wrap">
            <h1 className="text-2xl font-bold font-mono">{data.name}</h1>
            <button
              className="btn btn-ghost btn-sm btn-circle"
              onClick={() => copyToClipboard(data.name)}
              type="button"
              title="Copy to clipboard"
            >
              {copied ? (
                <svg xmlns="http://www.w3.org/2000/svg" className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M5 13l4 4L19 7" />
                </svg>
              ) : (
                <svg xmlns="http://www.w3.org/2000/svg" className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                </svg>
              )}
            </button>
            <span className="badge badge-outline">Metric</span>
            <StabilityBadge stability={data.stability} />
            {data.deprecated && (
              <span className="badge badge-warning">deprecated</span>
            )}
          </div>

          {data.deprecated && typeof data.deprecated === 'object' && (
            <div className="alert alert-warning">
              <svg xmlns="http://www.w3.org/2000/svg" className="stroke-current shrink-0 h-6 w-6" fill="none" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
              <div>
                <div className="font-bold">Deprecated</div>
                <div className="text-sm">{data.deprecated.note || 'This metric is deprecated.'}</div>
                {data.deprecated.renamed_to && (
                  <div className="text-sm mt-1">
                    Use <Link to="/metric/$name" params={{ name: data.deprecated.renamed_to }} className="link">{data.deprecated.renamed_to}</Link> instead.
                  </div>
                )}
              </div>
            </div>
          )}

          <div className="card bg-base-200">
            <div className="card-body">
              <h2 className="card-title">Description</h2>
              <div className="text-sm">
                <Markdown content={data.brief || 'No description available.'} />
              </div>
              {data.note && (
                <div className="mt-4">
                  <h3 className="font-semibold">Note</h3>
                  <div className="text-sm">
                    <Markdown content={data.note} />
                  </div>
                </div>
              )}
            </div>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="card bg-base-200">
              <div className="card-body">
                <h2 className="card-title">Instrument</h2>
                <span className="badge badge-lg badge-primary">{data.instrument || 'N/A'}</span>
              </div>
            </div>

            <div className="card bg-base-200">
              <div className="card-body">
                <h2 className="card-title">Unit</h2>
                <code className="bg-base-300 p-2 rounded">{data.unit || 'N/A'}</code>
              </div>
            </div>

            <div className="card bg-base-200">
              <div className="card-body">
                <h2 className="card-title">Attributes</h2>
                <span className="text-2xl font-bold">{data.attributes?.length || 0}</span>
              </div>
            </div>
          </div>

          {data.attributes && data.attributes.length > 0 && (
            <div className="card bg-base-200">
              <div className="card-body">
                <h2 className="card-title">Metric Attributes</h2>
                <div className="overflow-x-auto">
                  <table className="table">
                    <thead>
                      <tr>
                        <th>Attribute</th>
                        <th>Type</th>
                        <th>Requirement</th>
                        <th>Brief</th>
                      </tr>
                    </thead>
                    <tbody>
                      {data.attributes.map((attr, index) => {
                        const { label, badgeClass } = formatRequirementLevel(attr.requirement_level)
                        return (
                          <tr key={index}>
                            <td>
                              <Link to="/attribute/$key" params={{ key: attr.key }} className="link link-primary font-mono text-sm">
                                {attr.key}
                              </Link>
                            </td>
                              <td className="font-mono text-sm">{formatType(attr['r#type'])}</td>
                            <td>
                              <span className={`badge ${badgeClass}`}>{label}</span>
                            </td>
                            <td className="max-w-xs truncate"><InlineMarkdown content={attr.brief || '-'} /></td>
                          </tr>
                        )
                      })}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  )
}
