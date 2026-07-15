import {
  createRoute,
  Link,
  useElementScrollRestoration,
  useLocation,
  useNavigate,
} from '@tanstack/react-router'
import {
  type ChangeEvent,
  type KeyboardEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import { InlineMarkdown } from '../components/InlineMarkdown'
import { NamespaceTree, NamespaceTreeToolbar } from '../components/NamespaceTree'
import { Pagination } from '../components/Pagination'
import { StabilityBadge } from '../components/StabilityBadge'
import { TypeBadge } from '../components/TypeBadge'
import { ListViewIcon, TreeViewIcon } from '../components/ViewModeIcons'
import { search, searchAll } from '../lib/api'
import type { SearchResponse, StabilityFilter, TypeFilter } from '../lib/api'
import { buildNamespaceTree, collectFolderPaths, defaultExpansion } from '../lib/namespaceTree'
import { getScrollRestorationKey } from '../lib/scrollRestorationKey'
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

// Tree expansion, kept cheap in the URL: a bulk `base` (everything, nothing,
// or down to a level) plus small per-folder overrides - never one entry per
// folder, which would make "Expand all" on a large registry produce a URL of
// thousands of characters. `null` base means the size-based default.
type TreeBase = 'all' | 'none' | number

interface TreeExpansion {
  base: TreeBase | null
  open: string[]
  closed: string[]
}

const noTreeExpansion: TreeExpansion = { base: null, open: [], closed: [] }

const parseTypeFilter = (value: string | null): TypeFilter =>
  typeOptions.includes(value as TypeFilter) ? (value as TypeFilter) : 'all'

const parseStabilityFilter = (value: string | null): StabilityFilter =>
  stabilityOptions.includes(value as Exclude<StabilityFilter, null>)
    ? (value as Exclude<StabilityFilter, null>)
    : null

const parseViewMode = (value: string | null): ViewMode => (value === 'tree' ? 'tree' : 'list')

// A bare digit string (e.g. "1") is valid JSON, and TanStack Router's default
// search codec re-encodes any JSON-parseable string value through
// `JSON.stringify` to keep parse/stringify symmetric - turning `base=1` into
// `base=%221%22` in the address bar. The `lvl` prefix keeps level values from
// ever looking like JSON so they pass through untouched.
const serializeTreeBase = (base: TreeBase): string => (typeof base === 'number' ? `lvl${base}` : base)

const parseTreeBase = (value: string | null): TreeBase | null => {
  if (value === 'all' || value === 'none') return value
  const match = value?.match(/^lvl(\d+)$/)
  if (match) {
    const level = Number.parseInt(match[1], 10)
    if (level > 0) return level
  }
  return null
}

const parseCommaList = (value: string | null): string[] =>
  value ? value.split(',').filter(Boolean) : []

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
  const [treeExpansion, setTreeExpansionState] = useState<TreeExpansion>(noTreeExpansion)
  // Mirrors `treeExpansion` so `updateURL` always reads the current value
  // even when called from a stale closure (e.g. after an awaited fetch).
  const treeExpansionRef = useRef(treeExpansion)
  const setTreeExpansion = (next: TreeExpansion) => {
    treeExpansionRef.current = next
    setTreeExpansionState(next)
  }
  const [results, setResults] = useState<SearchResponse | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [initialized, setInitialized] = useState(false)
  const searchAbortRef = useRef<AbortController | null>(null)
  const requestVersionRef = useRef(0)
  const debounceTimeoutRef = useRef<number | null>(null)

  const totalPages = results ? Math.ceil(results.total / itemsPerPage) : 0
  const tree = useMemo(
    () => (view === 'tree' && results ? buildNamespaceTree(results.results) : null),
    [results, view]
  )
  const expanded = useMemo(() => {
    if (!tree) return new Set<string>()
    const { base, open, closed } = treeExpansion
    const baseSet =
      base === 'all'
        ? new Set(collectFolderPaths(tree))
        : base === 'none'
          ? new Set<string>()
          : typeof base === 'number'
            ? new Set(collectFolderPaths(tree, base))
            : defaultExpansion(tree)
    const result = new Set(baseSet)
    for (const path of open) result.add(path)
    for (const path of closed) result.delete(path)
    return result
  }, [tree, treeExpansion])

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
      // Always sourced from the ref (not the `treeExpansion` state closed
      // over at creation time) so this stays correct even when called from a
      // `performSearch` callback captured before the most recent tree action.
      const { base, open, closed } = treeExpansionRef.current
      if (base !== null) params.set('base', serializeTreeBase(base))
      if (open.length > 0) params.set('open', open.join(','))
      if (closed.length > 0) params.set('closed', closed.join(','))

      const queryString = params.toString()
      const nextSearch = queryString ? `?${queryString}` : ''

      // `location.search` is the *parsed* search object, not the query
      // string (that's `location.searchStr`) - comparing it to `nextSearch`
      // directly would always be true (object !== string) and navigate on
      // every call, even when nothing changed. Each spurious replace-nav
      // re-triggers the router's scroll-restoration snapshot/restore cycle,
      // which is what made restoring scroll position flaky.
      if (location.searchStr !== nextSearch) {
        const searchParams = Object.fromEntries(params.entries())
        navigate({ to: '/search', search: searchParams, replace: true })
      }
    },
    [location.searchStr, navigate]
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
      const params = new URLSearchParams(location.searchStr)
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
      setTreeExpansion({
        base: parseTreeBase(params.get('base')),
        open: parseCommaList(params.get('open')),
        closed: parseCommaList(params.get('closed')),
      })
      setInitialized(true)

      void performSearch({
        query: initialQuery,
        searchType: initialType,
        stabilityFilter: initialStability,
        currentPage: initialPage,
        view: initialView,
      })
    }
  }, [initialized, location.searchStr, performSearch])

  useEffect(() => {
    return () => {
      searchAbortRef.current?.abort()
      if (debounceTimeoutRef.current !== null) {
        window.clearTimeout(debounceTimeoutRef.current)
      }
    }
  }, [])

  // Only the results panel scrolls (the filters above stay put), so scroll
  // restoration targets that panel - identified by a stable
  // data-scroll-restoration-id rather than DOM position - instead of window.
  const resultsScrollRef = useRef<HTMLDivElement>(null)
  const resultsScrollId = 'search-results'

  // Data loads after mount (no route loader - filters are live-typed, which
  // doesn't fit a loader's per-navigation model), so the router's own
  // scroll-restoration attempt fires while the panel is still short (loading
  // state) and comes up empty. Re-apply the cached position once the first
  // load finishes and the panel has grown to its real height.
  const scrollEntry = useElementScrollRestoration({
    id: resultsScrollId,
    getKey: getScrollRestorationKey,
  })
  const appliedInitialScrollRef = useRef(false)
  useEffect(() => {
    if (appliedInitialScrollRef.current || !initialized || loading) return
    appliedInitialScrollRef.current = true
    if (scrollEntry) {
      resultsScrollRef.current?.scrollTo({ top: scrollEntry.scrollY, left: scrollEntry.scrollX })
    }
  }, [initialized, loading, scrollEntry])

  // Tree expansion lives in the URL (replace, not push) so the browser back
  // button returns to the same folders open, same as the filters above. This
  // writes on the explicit user action itself (not a reactive effect on
  // `expanded`) so it can't race a subsequent route navigation - e.g.
  // clicking a leaf link right after the default expansion is applied.
  const currentSearchState = (): SearchState => ({
    query,
    searchType,
    stabilityFilter,
    currentPage,
    view,
  })

  const handleTreeToggle = (path: string) => {
    const isOpen = expanded.has(path)
    const next: TreeExpansion = isOpen
      ? {
          base: treeExpansion.base,
          open: treeExpansion.open.filter((p) => p !== path),
          closed: treeExpansion.closed.includes(path)
            ? treeExpansion.closed
            : [...treeExpansion.closed, path],
        }
      : {
          base: treeExpansion.base,
          open: treeExpansion.open.includes(path)
            ? treeExpansion.open
            : [...treeExpansion.open, path],
          closed: treeExpansion.closed.filter((p) => p !== path),
        }
    setTreeExpansion(next)
    updateURL(currentSearchState())
  }

  const handleExpandAll = () => {
    setTreeExpansion({ base: 'all', open: [], closed: [] })
    updateURL(currentSearchState())
  }

  const handleCollapseAll = () => {
    setTreeExpansion({ base: 'none', open: [], closed: [] })
    updateURL(currentSearchState())
  }

  const handleExpandToLevel = (level: number) => {
    setTreeExpansion({ base: level, open: [], closed: [] })
    updateURL(currentSearchState())
  }

  const handleQueryInput = (event: ChangeEvent<HTMLInputElement>) => {
    const nextQuery = event.target.value
    setQuery(nextQuery)
    setCurrentPage(1)
    setTreeExpansion(noTreeExpansion)
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
      setTreeExpansion(noTreeExpansion)
      void performSearch({ query: nextQuery, currentPage: 1 })
    }
  }

  const handleTypeChange = (event: ChangeEvent<HTMLSelectElement>) => {
    const nextType = parseTypeFilter(event.target.value)
    setSearchType(nextType)
    setCurrentPage(1)
    setTreeExpansion(noTreeExpansion)
    void performSearch({ searchType: nextType, currentPage: 1 })
  }

  const handleStabilityChange = (event: ChangeEvent<HTMLSelectElement>) => {
    const nextStability = parseStabilityFilter(event.target.value)
    setStabilityFilter(nextStability)
    setCurrentPage(1)
    setTreeExpansion(noTreeExpansion)
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
    resultsScrollRef.current?.scrollTo({ top: 0, behavior: 'smooth' })
  }

  // 4rem navbar + 3rem for main's own p-6 (top+bottom) - without both the
  // page still overflows the window by that padding and the outer document
  // scrolls instead of just the results panel below.
  return (
    <div className="flex flex-col h-[calc(100vh-7rem)]">
      <div className="shrink-0 space-y-4 pb-4">
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

        {!error && results ? (
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
        ) : null}

        {/* Matches the scrollable panel's own gate below: while a stale
           (paginated) result set is still on screen waiting for the full
           tree fetch, `tree` would reflect that stale data too. */}
        {!error && results && view === 'tree' && tree && !(loading && results.count < results.total) ? (
          <NamespaceTreeToolbar
            tree={tree}
            onExpandAll={handleExpandAll}
            onCollapseAll={handleCollapseAll}
            onExpandToLevel={handleExpandToLevel}
          />
        ) : null}
      </div>

      <div
        ref={resultsScrollRef}
        data-scroll-restoration-id={resultsScrollId}
        className="flex-1 overflow-y-auto space-y-4"
      >
        {error ? (
          <div className="alert alert-error" role="alert">
            <span>Error: {error}</span>
          </div>
        ) : results ? (
          <>
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
              ) : tree ? (
                <NamespaceTree tree={tree} expanded={expanded} onToggle={handleTreeToggle} />
              ) : null
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
          </>
        ) : !loading ? (
          <div className="text-center text-base-content/70 py-8">
            <p>Enter a search term or leave empty to browse all items.</p>
            <p className="text-sm mt-2">Use the type and stability filters to narrow results.</p>
          </div>
        ) : null}
      </div>
    </div>
  )
}
