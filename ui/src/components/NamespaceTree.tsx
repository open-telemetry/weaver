import { Link } from '@tanstack/react-router'
import { useMemo, useState } from 'react'
import type { SearchResult } from '../lib/api'
import {
  buildNamespaceTree,
  collectFolderPaths,
  maxFolderDepth,
  type TreeItem,
  type TreeNode,
} from '../lib/namespaceTree'
import { getResultLink, getResultMeta } from '../lib/searchResults'
import { InlineMarkdown } from './InlineMarkdown'
import { StabilityDot } from './StabilityBadge'
import { TypeBadge } from './TypeBadge'

// Small result sets (e.g. a filtered search) open fully expanded.
const autoExpandMaxItems = 50
const maxLevelButtons = 5

interface NamespaceTreeProps {
  results: SearchResult[]
}

export function NamespaceTree({ results }: NamespaceTreeProps) {
  const tree = useMemo(() => buildNamespaceTree(results), [results])
  const deepestLevel = useMemo(() => maxFolderDepth(tree), [tree])
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => defaultExpansion(tree))
  const [prevTree, setPrevTree] = useState(tree)

  // Reset expansion whenever a new result set arrives (adjust-state-on-prop-change).
  if (prevTree !== tree) {
    setPrevTree(tree)
    setExpanded(defaultExpansion(tree))
  }

  const handleToggle = (path: string) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(path)) {
        next.delete(path)
      } else {
        next.add(path)
      }
      return next
    })
  }

  const expandToLevel = (level: number) => {
    setExpanded(new Set(collectFolderPaths(tree, level)))
  }

  const levels = Array.from(
    { length: Math.min(deepestLevel, maxLevelButtons) },
    (_, index) => index + 1
  )

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-3 flex-wrap">
        {levels.length > 0 ? (
          <div className="flex items-center gap-2">
            <span className="text-xs font-medium uppercase tracking-wide text-base-content/60">
              Expand to level
            </span>
            <div className="join">
              {levels.map((level) => (
                <button
                  key={level}
                  type="button"
                  className="join-item btn btn-xs"
                  onClick={() => expandToLevel(level)}
                >
                  {level}
                </button>
              ))}
            </div>
          </div>
        ) : null}
        <div className="join">
          <button
            type="button"
            className="join-item btn btn-xs"
            onClick={() => setExpanded(new Set(collectFolderPaths(tree)))}
          >
            Expand all
          </button>
          <button
            type="button"
            className="join-item btn btn-xs"
            onClick={() => setExpanded(new Set())}
          >
            Collapse all
          </button>
        </div>
        <span className="ml-auto text-xs text-base-content/50">
          {tree.itemCount} {tree.itemCount === 1 ? 'name' : 'names'} in {tree.children.length}{' '}
          root {tree.children.length === 1 ? 'namespace' : 'namespaces'}
        </span>
      </div>

      <div className="card bg-base-200">
        <div className="card-body p-3">
          <TreeChildren node={tree} expanded={expanded} onToggle={handleToggle} />
        </div>
      </div>
    </div>
  )
}

function defaultExpansion(tree: TreeNode): ReadonlySet<string> {
  return tree.itemCount <= autoExpandMaxItems ? new Set(collectFolderPaths(tree)) : new Set()
}

interface TreeChildrenProps {
  node: TreeNode
  expanded: ReadonlySet<string>
  onToggle: (path: string) => void
}

function TreeChildren({ node, expanded, onToggle }: TreeChildrenProps) {
  // Items whose full name IS this namespace describe the folder itself and lead.
  const selfItems = node.items.filter((item) => item.id === node.path)
  const childItems = node.items.filter((item) => item.id !== node.path)

  return (
    <ul
      className={
        node.depth === 0 ? 'space-y-px' : 'ml-2.5 space-y-px border-l border-base-300 pl-3'
      }
    >
      {selfItems.map((item) => (
        <LeafRow key={`${item.result.result_type}:${item.id}`} item={item} />
      ))}
      {node.children.map((child) => (
        <FolderRow key={child.path} node={child} expanded={expanded} onToggle={onToggle} />
      ))}
      {childItems.map((item) => (
        <LeafRow key={`${item.result.result_type}:${item.id}`} item={item} />
      ))}
    </ul>
  )
}

interface FolderRowProps {
  node: TreeNode
  expanded: ReadonlySet<string>
  onToggle: (path: string) => void
}

function FolderRow({ node, expanded, onToggle }: FolderRowProps) {
  const isExpanded = expanded.has(node.path)

  return (
    <li>
      <button
        type="button"
        onClick={() => onToggle(node.path)}
        aria-expanded={isExpanded}
        title={node.path}
        className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-base-300"
      >
        <ChevronIcon expanded={isExpanded} />
        <FolderIcon open={isExpanded} />
        <span className="font-mono text-sm font-medium">{node.segment}</span>
        <span className="badge badge-ghost badge-xs font-normal text-base-content/60">
          {node.itemCount}
        </span>
      </button>
      {isExpanded ? <TreeChildren node={node} expanded={expanded} onToggle={onToggle} /> : null}
    </li>
  )
}

function LeafRow({ item }: { item: TreeItem }) {
  const { result } = item

  return (
    <li>
      <Link
        to={getResultLink(result)}
        title={item.id}
        className={`flex min-w-0 items-center gap-2 rounded-md px-2 py-1.5 transition-colors hover:bg-base-300${
          result.deprecated ? ' opacity-50' : ''
        }`}
      >
        {/* Spacer keeps leaves aligned with folder labels (chevron width). */}
        <span className="w-3.5 shrink-0" aria-hidden="true" />
        <TypeBadge type={result.result_type} size="xs" />
        <span className="whitespace-nowrap font-mono text-sm font-semibold">{item.segment}</span>
        {result.stability ? <StabilityDot stability={result.stability} /> : null}
        {result.deprecated ? (
          <span className="badge badge-ghost badge-xs">deprecated</span>
        ) : null}
        {getResultMeta(result).map((info) => (
          <span
            key={info.label}
            className="hidden whitespace-nowrap text-xs text-base-content/60 sm:inline"
          >
            <span className="font-semibold">{info.label}:</span> {info.value}
          </span>
        ))}
        {result.brief ? (
          <span className="hidden min-w-0 flex-1 truncate text-xs text-base-content/50 md:inline">
            <InlineMarkdown content={result.brief} />
          </span>
        ) : null}
      </Link>
    </li>
  )
}

function ChevronIcon({ expanded }: { expanded: boolean }) {
  return (
    <svg
      viewBox="0 0 20 20"
      fill="currentColor"
      aria-hidden="true"
      className={`h-3.5 w-3.5 shrink-0 text-base-content/50 transition-transform${
        expanded ? ' rotate-90' : ''
      }`}
    >
      <path
        fillRule="evenodd"
        d="M8.22 5.22a.75.75 0 0 1 1.06 0l4.25 4.25a.75.75 0 0 1 0 1.06l-4.25 4.25a.75.75 0 0 1-1.06-1.06L11.94 10 8.22 6.28a.75.75 0 0 1 0-1.06Z"
        clipRule="evenodd"
      />
    </svg>
  )
}

function FolderIcon({ open }: { open: boolean }) {
  return open ? (
    <svg
      viewBox="0 0 20 20"
      fill="currentColor"
      aria-hidden="true"
      className="h-4 w-4 shrink-0 text-warning"
    >
      <path
        fillRule="evenodd"
        d="M4.75 3A1.75 1.75 0 0 0 3 4.75v2.752l.104-.002h13.792c.035 0 .07 0 .104.002V6.75A1.75 1.75 0 0 0 15.25 5h-3.836a.25.25 0 0 1-.177-.073L9.823 3.513A1.75 1.75 0 0 0 8.586 3H4.75ZM3.104 9a1.75 1.75 0 0 0-1.673 2.265l1.385 4.5A1.75 1.75 0 0 0 4.488 17h11.023a1.75 1.75 0 0 0 1.673-1.235l1.384-4.5A1.75 1.75 0 0 0 16.896 9H3.104Z"
        clipRule="evenodd"
      />
    </svg>
  ) : (
    <svg
      viewBox="0 0 20 20"
      fill="currentColor"
      aria-hidden="true"
      className="h-4 w-4 shrink-0 text-warning"
    >
      <path d="M3.75 3A1.75 1.75 0 0 0 2 4.75v3.26a3.235 3.235 0 0 1 1.75-.51h12.5c.644 0 1.245.188 1.75.51V6.75A1.75 1.75 0 0 0 16.25 5h-4.836a.25.25 0 0 1-.177-.073L9.823 3.513A1.75 1.75 0 0 0 8.586 3H3.75ZM3.75 9A1.75 1.75 0 0 0 2 10.75v4.5c0 .966.784 1.75 1.75 1.75h12.5A1.75 1.75 0 0 0 18 15.25v-4.5A1.75 1.75 0 0 0 16.25 9H3.75Z" />
    </svg>
  )
}
