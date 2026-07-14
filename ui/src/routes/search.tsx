import { createRoute, Link, useLocation, useNavigate } from '@tanstack/react-router'
import {
  type ChangeEvent,
  type KeyboardEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react'
import { InlineMarkdown } from '../components/InlineMarkdown'
import { NamespaceTree } from '../components/NamespaceTree'
import { Pagination } from '../components/Pagination'
import { StabilityBadge } from '../components/StabilityBadge'
import { TypeBadge } from '../components/TypeBadge'
import { ListViewIcon, TreeViewIcon } from '../components/ViewModeIcons'
import { search, searchAll } from '../lib/api'
import type { SearchResponse, StabilityFilter, TypeFilter } from '../lib/api'
import { getResultId, getResultLink, getResultMeta } from '../lib/searchResults'
import { Route as RootRoute } from './__root'

const itemsPerPage = 50
const searchDebounceMs = 250
const typeOptions: TypeFilter[] = ['all', 'attribute', 'metric', 'span', 'event', 'entity']
const stabilityOptions: Array<Exclude<StabilityFilter, null>> = [
  'stable',
  'development',
  'alpha',
  'beta',
  'release_candidate',
  'deprecated',
]

type ViewMode = 'list' | 'tree'

interface SearchState {
  query: string
  searchType: TypeFilter
  stabilityFilter: StabilityFilter
  currentPage: number
  view: ViewMode
}

const parseTypeFilter = (value: string | null): TypeFilter =>
  typeOptions.includes(value as TypeFilter) ? (value as TypeFilter) : 'all'

const parseStabilityFilter = (value: string | null): StabilityFilter =>
  stabilityOptions.includes(value as Exclude<StabilityFilter, null>)
    ? (value as Exclude<StabilityFilter, null>)
    : null

const parseViewMode = (value: string | null): ViewMode => (value === 'tree' ? 'tree' : 'list')

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
  const [view, setView] = useState<ViewMode>('list')
  const [results, setResults] = useState<SearchResponse | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [initialized, setInitialized] = useState(false)
  const searchAbortRef = useRef<AbortController | null>(null)
  const requestVersionRef = useRef(0)
  const debounceTimeoutRef = useRef<number | null>(null)

  const totalPages = results ? Math.ceil(results.total / itemsPerPage) : 0

  const updateURL = useCallback(
    (state: SearchState) => {
      const params = new URLSearchParams()
      if (state.query) params.set('q', state.query)
      if (state.searchType !== 'all') params.set('type', state.searchType)
      if (state.stabilityFilter) params.set('stability', state.stabilityFilter)
      if (state.view === 'tree') params.set('view', 'tree')
      if (state.view === 'list' && state.currentPage > 1) {
        params.set('page', state.currentPage.toString())
      }

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
      const nextView = overrides.view ?? view
      const nextOffset = (nextPage - 1) * itemsPerPage

      setLoading(true)
      setError(null)

      requestVersionRef.current += 1
      const requestVersion = requestVersionRef.current
      searchAbortRef.current?.abort()
      const controller = new AbortController()
      searchAbortRef.current = controller

      try {
        const normalizedQuery = nextQuery.trim() || null
        // The tree view needs the complete result set; the list view pages.
        const response =
          nextView === 'tree'
            ? await searchAll(normalizedQuery, nextType, nextStability, {
                signal: controller.signal,
              })
            : await search(normalizedQuery, nextType, nextStability, itemsPerPage, nextOffset, {
                signal: controller.signal,
              })
        if (requestVersion !== requestVersionRef.current) return
        setResults(response)
        updateURL({
          query: nextQuery,
          searchType: nextType,
          stabilityFilter: nextStability,
          currentPage: nextPage,
          view: nextView,
        })
      } catch (err) {
        if (err instanceof DOMException && err.name === 'AbortError') return
        if (requestVersion !== requestVersionRef.current) return
        setError(err instanceof Error ? err.message : 'Unknown error')
        setResults(null)
      } finally {
        if (requestVersion !== requestVersionRef.current) return
        setLoading(false)
      }
    },
    [currentPage, query, searchType, stabilityFilter, updateURL, view]
  )

  useEffect(() => {
    if (!initialized) {
      const params = new URLSearchParams(location.search)
      const initialQuery = params.get('q') ?? ''
      const initialType = parseTypeFilter(params.get('type'))
      const initialStability = parseStabilityFilter(params.get('stability'))
      const initialView = parseViewMode(params.get('view'))
      const parsedPage = Number.parseInt(params.get('page') ?? '1', 10)
      const initialPage = Number.isNaN(parsedPage) || parsedPage < 1 ? 1 : parsedPage

      setQuery(initialQuery)
      setSearchType(initialType)
      setStabilityFilter(initialStability)
      setCurrentPage(initialPage)
      setView(initialView)
      setInitialized(true)

      void performSearch({
        query: initialQuery,
        searchType: initialType,
        stabilityFilter: initialStability,
        currentPage: initialPage,
        view: initialView,
      })
    }
  }, [initialized, location.search, performSearch])

  useEffect(() => {
    return () => {
      searchAbortRef.current?.abort()
      if (debounceTimeoutRef.current !== null) {
        window.clearTimeout(debounceTimeoutRef.current)
      }
    }
  }, [])

  const handleQueryInput = (event: ChangeEvent<HTMLInputElement>) => {
    const nextQuery = event.target.value
    setQuery(nextQuery)
    setCurrentPage(1)
    if (debounceTimeoutRef.current !== null) {
      window.clearTimeout(debounceTimeoutRef.current)
    }
    debounceTimeoutRef.current = window.setTimeout(() => {
      void performSearch({ query: nextQuery, currentPage: 1 })
    }, searchDebounceMs)
  }

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') {
      if (debounceTimeoutRef.current !== null) {
        window.clearTimeout(debounceTimeoutRef.current)
      }
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

  const handleViewChange = (nextView: ViewMode) => {
    if (nextView === view) return
    setView(nextView)
    setCurrentPage(1)
    void performSearch({ view: nextView, currentPage: 1 })
  }

  const handlePageChange = (page: number) => {
    setCurrentPage(page)
    void performSearch({ currentPage: page })
    window.scrollTo({ top: 0, behavior: 'smooth' })
  }

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
        <div className="join" role="group" aria-label="Result view">
          <button
            type="button"
            className={`join-item btn${view === 'list' ? ' btn-active' : ''}`}
            aria-pressed={view === 'list'}
            onClick={() => handleViewChange('list')}
          >
            <ListViewIcon />
            List
          </button>
          <button
            type="button"
            className={`join-item btn${view === 'tree' ? ' btn-active' : ''}`}
            aria-pressed={view === 'tree'}
            onClick={() => handleViewChange('tree')}
          >
            <TreeViewIcon />
            Tree
          </button>
        </div>
      </div>

      {error ? (
        <div className="alert alert-error" role="alert">
          <span>Error: {error}</span>
        </div>
      ) : results ? (
        <>
          <div className="flex items-center justify-between gap-2">
            <p className="flex items-center gap-2 text-sm text-base-content/70">
              {results.query ? (
                <>Found {results.total} results for "{results.query}"</>
              ) : view === 'tree' ? (
                <>Showing all {results.total} items</>
              ) : (
                <>Showing {results.count} of {results.total} items</>
              )}
              {loading ? <span className="loading loading-spinner loading-xs" /> : null}
            </p>

            {view === 'list' && totalPages > 1 ? (
              <Pagination
                total={results.total}
                limit={itemsPerPage}
                offset={(currentPage - 1) * itemsPerPage}
                onPageChange={(newOffset) => handlePageChange(Math.floor(newOffset / itemsPerPage) + 1)}
              />
            ) : null}
          </div>

          {results.results.length === 0 ? (
            <div className="alert" role="alert">
              <span>No results found. Try a different search term or filter.</span>
            </div>
          ) : view === 'tree' ? (
            loading && results.count < results.total ? (
              // A paged (list) response is still on screen while the full set loads.
              <div className="flex justify-center py-12">
                <span className="loading loading-spinner loading-lg text-base-content/40" />
              </div>
            ) : (
              <NamespaceTree results={results.results} />
            )
          ) : (
            <div className="space-y-2">
              {results.results.map((result, index) => (
                <Link
                  key={`${result.result_type}-${getResultId(result)}-${index}`}
                  to={getResultLink(result)}
                  className={`card bg-base-200 hover:bg-base-300 cursor-pointer${
                    result.deprecated ? ' opacity-50' : ''
                  }`}
                >
                  <div className="card-body py-3">
                    <div className="flex items-center gap-2 flex-wrap">
                      <TypeBadge type={result.result_type} />
                      <span className="font-mono font-semibold">{getResultId(result)}</span>
                      {result.stability ? <StabilityBadge stability={result.stability} /> : null}
                      {result.deprecated ? (
                        <span className="badge badge-sm badge-ghost">deprecated</span>
                      ) : null}
                      {getResultMeta(result).map((info) => (
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

          {view === 'list' && totalPages > 1 ? (
            <div className="flex justify-center mt-4">
              <Pagination
                total={results.total}
                limit={itemsPerPage}
                offset={(currentPage - 1) * itemsPerPage}
                onPageChange={(newOffset) => handlePageChange(Math.floor(newOffset / itemsPerPage) + 1)}
              />
            </div>
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
