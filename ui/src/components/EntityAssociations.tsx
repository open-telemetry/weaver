import { Fragment } from 'react'
import { Link } from '@tanstack/react-router'
import type { EntityAssociation } from '../lib/api'

function isAllOf(node: EntityAssociation): node is { all_of: EntityAssociation[] } {
  return typeof node === 'object' && node !== null && 'all_of' in node
}

function EntityLink({ name }: { name: string }) {
  return (
    <Link to="/entity/$type" params={{ type: name }} className="link link-primary font-mono">
      {name}
    </Link>
  )
}

function Connector({ word }: { word: string }) {
  return <span className="text-xs font-semibold uppercase text-base-content/50">{word}</span>
}

function Paren({ children }: { children: string }) {
  return <span className="text-base-content/40 font-mono">{children}</span>
}

/**
 * Renders an entity association expression inline as a boolean expression, e.g.
 * `tenant AND (host OR container)`. `nested` is true when this node is contained
 * within another combinator, in which case a multi-element group is wrapped in
 * parentheses to keep the precedence clear.
 */
function Expr({ node, nested }: { node: EntityAssociation; nested: boolean }) {
  if (typeof node === 'string') {
    return <EntityLink name={node} />
  }

  const children = isAllOf(node) ? node.all_of : node.one_of
  const word = isAllOf(node) ? 'and' : 'or'
  const wrap = nested && children.length > 1

  return (
    <span className="inline-flex flex-wrap items-center gap-x-2 gap-y-1">
      {wrap && <Paren>(</Paren>}
      {children.map((child, index) => (
        <Fragment key={index}>
          {index > 0 && <Connector word={word} />}
          <Expr node={child} nested />
        </Fragment>
      ))}
      {wrap && <Paren>)</Paren>}
    </span>
  )
}

/**
 * Renders the entity associations declared on a signal (span, metric or event).
 *
 * The top-level list is an implicit `one_of`: the signal must be associated with
 * at least one of the entries. Each entry may be a bare entity reference or a
 * nested `one_of` / `all_of` expression. The whole thing is rendered as a single
 * readable boolean expression with links to each referenced entity.
 *
 * Returns `null` when there are no associations so callers can render it
 * unconditionally.
 */
export function EntityAssociations({
  associations,
}: {
  associations?: EntityAssociation[]
}) {
  if (!associations || associations.length === 0) {
    return null
  }

  // A single top-level entry renders on its own; multiple entries are an implicit
  // `one_of`. In both cases the outermost expression needs no surrounding parens.
  const root: EntityAssociation =
    associations.length === 1 ? associations[0] : { one_of: associations }

  return (
    <div className="card bg-base-200">
      <div className="card-body">
        <h2 className="card-title">Entity Associations</h2>
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm">
          <Expr node={root} nested={false} />
        </div>
      </div>
    </div>
  )
}
