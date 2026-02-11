import { createRoute, Link } from '@tanstack/react-router'
import { getAttribute, type AttributeResponse } from '../lib/api'
import { Route as RootRoute } from './__root'
import { StabilityBadge } from '../components/StabilityBadge'
import { Markdown } from '../components/Markdown'
import { InlineMarkdown } from '../components/InlineMarkdown'
import { useCopyToClipboard } from '../hooks/useCopyToClipboard'
import { useResourceFetch } from '../hooks/useResourceFetch'

export const Route = createRoute({
  getParentRoute: () => RootRoute,
  path: 'attribute/$key',
  component: AttributeDetail,
})

function formatType(type: AttributeResponse['type']): string {
  if (typeof type === 'string') return type
  if (type && typeof type === 'object' && 'members' in type) {
    return `enum { ${type.members.map(m => m.value || m.id).join(', ')} }`
  }
  return JSON.stringify(type)
}

function AttributeDetail() {
  const { key } = Route.useParams()
  const { data, error } = useResourceFetch<AttributeResponse>(key, getAttribute)
  const { copied, copyToClipboard } = useCopyToClipboard()

  return (
    <div className="space-y-4">
      {error ? (
        <div className="alert alert-error" role="alert">
          <span>Error: {error}</span>
        </div>
      ) : !data ? (
        <div className="flex justify-center">
          <span className="loading loading-spinner loading-lg"></span>
        </div>
      ) : (
        <>
          <div className="flex items-center gap-4 flex-wrap">
            <h1 className="text-2xl font-bold font-mono">{data.key}</h1>
            <button
              className="btn btn-ghost btn-sm btn-circle"
              onClick={() => copyToClipboard(data.key)}
              type="button"
              title="Copy to clipboard"
              aria-label="Copy attribute key"
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
            <span className="badge badge-outline">Attribute</span>
            <StabilityBadge stability={data.stability} />
            {data.deprecated && (
              <span className="badge badge-warning">deprecated</span>
            )}
          </div>

          {data.deprecated && typeof data.deprecated === 'object' && (
            <div className="alert alert-warning" role="alert">
              <svg xmlns="http://www.w3.org/2000/svg" className="stroke-current shrink-0 h-6 w-6" fill="none" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
              <div>
                <div className="font-bold">Deprecated</div>
                <div className="text-sm">{data.deprecated.note || 'This attribute is deprecated.'}</div>
                {data.deprecated.renamed_to && (
                  <div className="text-sm mt-1">
                    Use <Link to="/attribute/$key" params={{ key: data.deprecated.renamed_to }} className="link">{data.deprecated.renamed_to}</Link> instead.
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

          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="card bg-base-200">
              <div className="card-body">
                <h2 className="card-title">Type</h2>
                <code className="bg-base-300 p-2 rounded">{formatType(data.type)}</code>

                {data.type && typeof data.type === 'object' && 'members' in data.type && (
                  <div className="mt-4">
                    <h3 className="font-semibold mb-2">Enum Values</h3>
                    <div className="overflow-x-auto">
                      <table className="table table-sm">
                        <caption className="sr-only">Attribute enum values and descriptions</caption>
                        <thead>
                          <tr>
                            <th>Value</th>
                            <th>Description</th>
                          </tr>
                        </thead>
                        <tbody>
                          {data.type.members.map((member, index) => (
                            <tr key={index}>
                              <td className="font-mono">{member.value || member.id}</td>
                              <td><InlineMarkdown content={member.brief || '-'} /></td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </div>
                )}
              </div>
            </div>

            <div className="card bg-base-200">
              <div className="card-body">
                <h2 className="card-title">Examples</h2>
                {data.examples && data.examples.length > 0 ? (
                  <ul className="list-disc list-inside">
                    {data.examples.map((example, index) => (
                      <li key={index} className="font-mono">{JSON.stringify(example)}</li>
                    ))}
                  </ul>
                ) : (
                  <p className="text-base-content/70">No examples available.</p>
                )}
              </div>
            </div>
          </div>
        </>
      )}
    </div>
  )
}
