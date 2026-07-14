import type { SearchResult } from './api';
import { getResultId } from './searchResults';

/**
 * Namespace separator used to split names into tree levels. Matches weaver's
 * default; the serve API does not currently expose a configured separator.
 */
export const NAMESPACE_SEPARATOR = '.';

/** A complete name (a "file" in the filesystem analogy) attached to a tree node. */
export interface TreeItem {
  result: SearchResult;
  /** Full name, e.g. `cpu.mode`. */
  id: string;
  /** Final name segment, e.g. `mode`. */
  segment: string;
}

/** A namespace ("folder") in the tree. The root node has an empty path. */
export interface TreeNode {
  segment: string;
  /** Full namespace path, e.g. `config.setting`. */
  path: string;
  /** Root = 0, top-level namespaces = 1, ... */
  depth: number;
  children: TreeNode[];
  items: TreeItem[];
  /** Total items in this subtree (including this node's own items). */
  itemCount: number;
}

interface BuildNode {
  segment: string;
  path: string;
  depth: number;
  children: Map<string, BuildNode>;
  items: TreeItem[];
}

/**
 * Build a namespace tree from search results. Results are deduplicated by
 * (result_type, name) — the flat search results can repeat a name when it
 * matches through multiple groups.
 */
export function buildNamespaceTree(
  results: SearchResult[],
  separator: string = NAMESPACE_SEPARATOR
): TreeNode {
  const root: BuildNode = { segment: '', path: '', depth: 0, children: new Map(), items: [] };
  const seen = new Set<string>();

  for (const result of results) {
    const id = getResultId(result);
    if (!id) continue;
    const dedupeKey = `${result.result_type}:${id}`;
    if (seen.has(dedupeKey)) continue;
    seen.add(dedupeKey);

    const parts = id.split(separator).filter((part) => part.length > 0);
    if (parts.length === 0) continue;

    let node = root;
    for (const part of parts.slice(0, -1)) {
      let child = node.children.get(part);
      if (!child) {
        child = {
          segment: part,
          path: node.path ? `${node.path}${separator}${part}` : part,
          depth: node.depth + 1,
          children: new Map(),
          items: [],
        };
        node.children.set(part, child);
      }
      node = child;
    }
    node.items.push({ result, id, segment: parts[parts.length - 1] });
  }

  relocateFolderItems(root);
  return finalize(root);
}

/**
 * When a complete name is also a namespace (e.g. entity `host` alongside
 * `host.*` attributes), move the item inside its namespace folder so it is
 * presented with the things it describes.
 */
function relocateFolderItems(node: BuildNode): void {
  node.items = node.items.filter((item) => {
    const folder = node.children.get(item.segment);
    if (folder && folder.path === item.id) {
      folder.items.push(item);
      return false;
    }
    return true;
  });
  for (const child of node.children.values()) {
    relocateFolderItems(child);
  }
}

function finalize(node: BuildNode): TreeNode {
  const children = Array.from(node.children.values())
    .map(finalize)
    .sort((a, b) => a.segment.localeCompare(b.segment));
  const items = [...node.items].sort((a, b) => {
    // An item whose full name IS this namespace comes first — it describes the folder.
    const aSelf = a.id === node.path ? 0 : 1;
    const bSelf = b.id === node.path ? 0 : 1;
    if (aSelf !== bSelf) return aSelf - bSelf;
    return (
      a.segment.localeCompare(b.segment) ||
      a.result.result_type.localeCompare(b.result.result_type)
    );
  });
  const itemCount = items.length + children.reduce((sum, child) => sum + child.itemCount, 0);
  return { segment: node.segment, path: node.path, depth: node.depth, children, items, itemCount };
}

/** Depth of the deepest namespace folder in the tree (0 when there are no folders). */
export function maxFolderDepth(node: TreeNode): number {
  return node.children.reduce((max, child) => Math.max(max, maxFolderDepth(child)), node.depth);
}

/** Paths of all folders with depth <= maxDepth (all folders by default). */
export function collectFolderPaths(node: TreeNode, maxDepth: number = Infinity): string[] {
  const paths: string[] = [];
  const walk = (current: TreeNode) => {
    for (const child of current.children) {
      if (child.depth > maxDepth) continue;
      paths.push(child.path);
      walk(child);
    }
  };
  walk(node);
  return paths;
}
