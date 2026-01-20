import { createRoute, Link, useLocation, useNavigate } from '@tanstack/react-router'
import {
  type ChangeEvent,
  type KeyboardEvent,
  useCallback,
  useEffect,
  useState,
} from 'react'
import { InlineMarkdown } from '../components/InlineMarkdown'
import { StabilityBadge } from '../components/StabilityBadge'
import { search } from '../lib/api'
import type { SearchResponse, SearchResult, StabilityFilter, TypeFilter } from '../lib/api'
import { Route as RootRoute } from './__root'

const itemsPerPage = 50
const typeOptions: TypeFilter[] = ['all', 'attribute', 'metric', 'span', 'event', 'entity']
const stabilityOptions: Array<Exclude<StabilityFilter, null>> = [
  'stable',
  'development',
  'alpha',
  'beta',
  'release_candidate',
  'deprecated',
]

interface SearchState {
  query: string
  searchType: TypeFilter
  stabilityFilter: StabilityFilter
  currentPage: number
}

const parseTypeFilter = (value: string | null): TypeFilter =>
  typeOptions.includes(value as TypeFilter) ? (value as TypeFilter) : 'all'

const parseStabilityFilter = (value: string | null): StabilityFilter =>
  stabilityOptions.includes(value as Exclude<StabilityFilter, null>)
    ? (value as Exclude<StabilityFilter, null>)
    : null

export const Route = createRoute({
  getParentRoute: () => RootRoute,
  path: 'search',
  component: Search,
})

function Search() {
  const location = useLocation()
  const navigate = useNavigate()
  const [query, setQuery] = useState('')
  const [searchType, setSearchType] = useState<TypeFilter>('all')
  const [stabilityFilter, setStabilityFilter] = useState<StabilityFilter>(null)
  const [currentPage, setCurrentPage] = useState(1)
  const [results, setResults] = useState<SearchResponse | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [initialized, setInitialized] = useState(false)

  const totalPages = results ? Math.ceil(results.total / itemsPerPage) : 0

  const updateURL = useCallback(
    (state: SearchState) => {
      const params = new URLSearchParams()
      if (state.query) params.set('q', state.query)
      if (state.searchType !== 'all') params.set('type', state.searchType)
      if (state.stabilityFilter) params.set('stability', state.stabilityFilter)
      if (state.currentPage > 1) params.set('page', state.currentPage.toString())

      const queryString = params.toString()
      const nextSearch = queryString ? `?${queryString}` : ''

      if (location.search !== nextSearch) {
        const searchParams = Object.fromEntries(params.entries())
        navigate({ to: '/search', search: searchParams })
      }
    },
    [location.search, navigate]
  )

  const performSearch = useCallback(
    async (overrides: Partial<SearchState> = {}) => {
      const nextQuery = overrides.query ?? query
      const nextType = overrides.searchType ?? searchType
      const nextStability = overrides.stabilityFilter ?? stabilityFilter
      const nextPage = overrides.currentPage ?? currentPage
      const nextOffset = (nextPage - 1) * itemsPerPage

      setLoading(true)
      setError(null)

      try {
        const normalizedQuery = nextQuery.trim() || null
        const response = await search(
          normalizedQuery,
          nextType,
          nextStability,
          itemsPerPage,
          nextOffset
        )
        setResults(response)
        updateURL({
          query: nextQuery,
          searchType: nextType,
          stabilityFilter: nextStability,
          currentPage: nextPage,
        })
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Unknown error')
        setResults(null)
      } finally {
        setLoading(false)
      }
    },
    [currentPage, query, searchType, stabilityFilter, updateURL]
  )

  useEffect(() => {
    if (!initialized) {
      const params = new URLSearchParams(location.search)
      const initialQuery = params.get('q') ?? ''
      const initialType = parseTypeFilter(params.get('type'))
      const initialStability = parseStabilityFilter(params.get('stability'))
      const parsedPage = Number.parseInt(params.get('page') ?? '1', 10)
      const initialPage = Number.isNaN(parsedPage) || parsedPage < 1 ? 1 : parsedPage

      setQuery(initialQuery)
      setSearchType(initialType)
      setStabilityFilter(initialStability)
      setCurrentPage(initialPage)
      setInitialized(true)

      void performSearch({
        query: initialQuery,
        searchType: initialType,
        stabilityFilter: initialStability,
        currentPage: initialPage,
      })
    }
  }, [initialized, location.search, performSearch])

  const handleQueryInput = (event: ChangeEvent<HTMLInputElement>) => {
    const nextQuery = event.target.value
    setQuery(nextQuery)
    setCurrentPage(1)
    void performSearch({ query: nextQuery, currentPage: 1 })
  }

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') {
      const nextQuery = event.currentTarget.value
      setQuery(nextQuery)
      setCurrentPage(1)
      void performSearch({ query: nextQuery, currentPage: 1 })
    }
  }

  const handleTypeChange = (event: ChangeEvent<HTMLSelectElement>) => {
    const nextType = parseTypeFilter(event.target.value)
    setSearchType(nextType)
    setCurrentPage(1)
    void performSearch({ searchType: nextType, currentPage: 1 })
  }

  const handleStabilityChange = (event: ChangeEvent<HTMLSelectElement>) => {
    const nextStability = parseStabilityFilter(event.target.value)
    setStabilityFilter(nextStability)
    setCurrentPage(1)
    void performSearch({ stabilityFilter: nextStability, currentPage: 1 })
  }

  const handlePageChange = (page: number) => {
    setCurrentPage(page)
    void performSearch({ currentPage: page })
    window.scrollTo({ top: 0, behavior: 'smooth' })
  }

  const getItemLink = (result: SearchResult) => {
    switch (result.result_type) {
      case 'attribute':
        return `/attribute/${result.key ?? ''}`
      case 'metric':
        return `/metric/${result.name ?? ''}`
      case 'span':
        return `/span/${result.type ?? ''}`
      case 'event':
        return `/event/${result.name ?? ''}`
      case 'entity':
        return `/entity/${result.type ?? ''}`
      default:
        return '#'
    }
  }

  const getItemId = (result: SearchResult) => {
    if (result.result_type === 'span' || result.result_type === 'entity') {
      return String(result.type ?? '')
    }
    return String(result.key ?? result.name ?? result.type ?? '')
  }

  const formatType = (type: SearchResult['type']) => {
    if (typeof type === 'string') return type
    if (type && typeof type === 'object') {
      const typed = type as { members?: unknown[]; type?: string }
      if (Array.isArray(typed.members)) return 'enum'
      if (typeof typed.type === 'string') return typed.type
    }
    return type ? JSON.stringify(type) : ''
  }

  const getTypeSpecificInfo = (result: SearchResult) => {
    switch (result.result_type) {
      case 'attribute':
        return [{ label: 'Type', value: formatType(result.type) }]
      case 'metric':
        return [
          { label: 'Instrument', value: result.instrument ?? '-' },
          { label: 'Unit', value: result.unit || '-' },
        ]
      case 'span':
        return [{ label: 'Kind', value: result.kind || '-' }]
      case 'event':
      case 'entity':
      default:
        return []
    }
  }

  const renderPagination = () => (
    <div className="join">
      <button
        className="join-item btn btn-sm"
        disabled={currentPage === 1}
        onClick={() => handlePageChange(currentPage - 1)}
        type="button"
      >
        «
      </button>
      <button className="join-item btn btn-sm" type="button">
        Page {currentPage} of {totalPages}
      </button>
      <button
        className="join-item btn btn-sm"
        disabled={currentPage === totalPages}
        onClick={() => handlePageChange(currentPage + 1)}
        type="button"
      >
        »
      </button>
    </div>
  )

  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-bold">Search</h1>

      <div className="flex gap-4 flex-wrap">
        <input
          type="text"
          placeholder="Search attributes, metrics, spans, events, entities..."
          className="input input-bordered flex-1 min-w-64"
          value={query}
          onChange={handleQueryInput}
          onKeyDown={handleKeyDown}
        />
        <select className="select select-bordered" value={searchType} onChange={handleTypeChange}>
          <option value="all">All Types</option>
          <option value="attribute">Attributes</option>
          <option value="metric">Metrics</option>
          <option value="span">Spans</option>
          <option value="event">Events</option>
          <option value="entity">Entities</option>
        </select>
        <select
          className="select select-bordered"
          value={stabilityFilter ?? ''}
          onChange={handleStabilityChange}
        >
          <option value="">All Stability</option>
          <option value="stable">Stable</option>
          <option value="development">Development</option>
          <option value="alpha">Alpha</option>
          <option value="beta">Beta</option>
          <option value="release_candidate">Release Candidate</option>
          <option value="deprecated">Deprecated</option>
        </select>
      </div>

      {error ? (
        <div className="alert alert-error">
          <span>Error: {error}</span>
        </div>
      ) : results ? (
        <>
          <div className="flex items-center justify-between">
            <p className="text-sm text-base-content/70">
              {results.query ? (
                <>Found {results.total} results for "{results.query}"</>
              ) : (
                <>Showing {results.count} of {results.total} items</>
              )}
            </p>

            {totalPages > 1 ? renderPagination() : null}
          </div>

          {results.results.length === 0 ? (
            <div className="alert">
              <span>No results found. Try a different search term or filter.</span>
            </div>
          ) : (
            <div className="space-y-2">
              {results.results.map((result, index) => (
                <Link
                  key={`${result.result_type}-${getItemId(result)}-${index}`}
                  to={getItemLink(result)}
                  className={`card bg-base-200 hover:bg-base-300 cursor-pointer${
                    result.deprecated ? ' opacity-50' : ''
                  }`}
                >
                  <div className="card-body py-3">
                    <div className="flex items-center gap-2 flex-wrap">
                      <span className="badge badge-outline">{result.result_type}</span>
                      <span className="font-mono font-semibold">{getItemId(result)}</span>
                      {result.stability ? <StabilityBadge stability={result.stability} /> : null}
                      {result.deprecated ? (
                        <span className="badge badge-sm badge-ghost">deprecated</span>
                      ) : null}
                      {getTypeSpecificInfo(result).map((info) => (
                        <span key={info.label} className="text-xs text-base-content/60">
                          <span className="font-semibold">{info.label}:</span> {info.value}
                        </span>
                      ))}
                      <span className="text-xs text-base-content/50 ml-auto">
                        score: {result.score}
                      </span>
                    </div>
                    <p className="text-sm text-base-content/70 truncate">
                      <InlineMarkdown content={result.brief || 'No description'} />
                    </p>
                  </div>
                </Link>
              ))}
            </div>
          )}

          {totalPages > 1 ? (
            <div className="flex justify-center mt-4">{renderPagination()}</div>
          ) : null}
        </>
      ) : !loading ? (
        <div className="text-center text-base-content/70 py-8">
          <p>Enter a search term or leave empty to browse all items.</p>
          <p className="text-sm mt-2">Use the type and stability filters to narrow results.</p>
        </div>
      ) : null}
    </div>
  )
}
