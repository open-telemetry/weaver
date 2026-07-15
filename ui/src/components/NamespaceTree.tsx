import { Link } from '@tanstack/react-router'
import { useMemo } from 'react'
import { maxFolderDepth, type TreeItem, type TreeNode } from '../lib/namespaceTree'
import { getResultLink, getResultMeta } from '../lib/searchResults'
import { InlineMarkdown } from './InlineMarkdown'
import { StabilityDot } from './StabilityBadge'
import { TypeBadge } from './TypeBadge'

const maxLevelButtons = 5

interface NamespaceTreeToolbarProps {
  tree: TreeNode
  onExpandAll: () => void
  onCollapseAll: () => void
  onExpandToLevel: (level: number) => void
}

// Split out from NamespaceTree so the caller can pin this above a
// scrollable area that holds just the tree content.
export function NamespaceTreeToolbar({
  tree,
  onExpandAll,
  onCollapseAll,
  onExpandToLevel,
}: NamespaceTreeToolbarProps) {
  const deepestLevel = useMemo(() => maxFolderDepth(tree), [tree])

  const levels = Array.from(
    { length: Math.min(deepestLevel, maxLevelButtons) },
    (_, index) => index + 1
  )

  return (
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
                onClick={() => onExpandToLevel(level)}
              >
                {level}
              </button>
            ))}
          </div>
        </div>
      ) : null}
      <div className="join">
        <button type="button" className="join-item btn btn-xs" onClick={onExpandAll}>
          Expand all
        </button>
        <button type="button" className="join-item btn btn-xs" onClick={onCollapseAll}>
          Collapse all
        </button>
      </div>
      <span className="ml-auto text-xs text-base-content/50">
        {tree.itemCount} {tree.itemCount === 1 ? 'name' : 'names'} in {tree.children.length} root{' '}
        {tree.children.length === 1 ? 'namespace' : 'namespaces'}
      </span>
    </div>
  )
}

interface NamespaceTreeProps {
  tree: TreeNode
  expanded: ReadonlySet<string>
  onToggle: (path: string) => void
}

export function NamespaceTree({ tree, expanded, onToggle }: NamespaceTreeProps) {
  return (
    <div className="card bg-base-200">
      <div className="card-body p-3">
        <TreeChildren node={tree} expanded={expanded} onToggle={onToggle} />
      </div>
    </div>
  )
}

interface TreeChildrenProps {
  node: TreeNode
  expanded: ReadonlySet<string>
  onToggle: (path: string) => void
}

type TreeEntry =
  | { kind: 'folder'; segment: string; sortKey: string; node: TreeNode }
  | { kind: 'item'; segment: string; sortKey: string; item: TreeItem }

function TreeChildren({ node, expanded, onToggle }: TreeChildrenProps) {
  // Folders and items are siblings in the same namespace, so they're sorted
  // together into one alphabetical list rather than grouped by kind.
  const entries: TreeEntry[] = [
    ...node.children.map(
      (child): TreeEntry => ({ kind: 'folder', segment: child.segment, sortKey: child.path, node: child })
    ),
    ...node.items.map(
      (item): TreeEntry => ({ kind: 'item', segment: item.segment, sortKey: item.id, item })
    ),
  ].sort((a, b) => a.segment.localeCompare(b.segment) || a.sortKey.localeCompare(b.sortKey))

  return (
    <ul
      className={
        node.depth === 0 ? 'space-y-px' : 'ml-2.5 space-y-px border-l border-base-300 pl-3'
      }
    >
      {entries.map((entry) =>
        entry.kind === 'folder' ? (
          <FolderRow key={entry.node.path} node={entry.node} expanded={expanded} onToggle={onToggle} />
        ) : (
          <LeafRow
            key={`${entry.item.result.result_type}:${entry.item.id}`}
            item={entry.item}
          />
        )
      )}
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
        <span className="whitespace-nowrap font-mono text-sm font-semibold">{item.segment}</span>
        <TypeBadge type={result.result_type} size="xs" />
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

