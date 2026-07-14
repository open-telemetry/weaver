/** Icons for the search results view toggle (list / namespace tree). */

export function ListViewIcon() {
  return (
    <svg viewBox="0 0 20 20" fill="currentColor" aria-hidden="true" className="h-4 w-4 shrink-0">
      <path
        fillRule="evenodd"
        d="M2 4.75A.75.75 0 0 1 2.75 4h14.5a.75.75 0 0 1 0 1.5H2.75A.75.75 0 0 1 2 4.75Zm0 5.25a.75.75 0 0 1 .75-.75h14.5a.75.75 0 0 1 0 1.5H2.75A.75.75 0 0 1 2 10Zm0 5.25a.75.75 0 0 1 .75-.75h14.5a.75.75 0 0 1 0 1.5H2.75a.75.75 0 0 1-.75-.75Z"
        clipRule="evenodd"
      />
    </svg>
  )
}

export function TreeViewIcon() {
  return (
    <svg
      viewBox="0 0 20 20"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      aria-hidden="true"
      className="h-4 w-4 shrink-0"
    >
      <rect x="2.75" y="2.5" width="5.5" height="4" rx="1" />
      <rect x="11.75" y="8" width="5.5" height="4" rx="1" />
      <rect x="11.75" y="13.5" width="5.5" height="4" rx="1" />
      <path d="M5.5 6.5v9h6.25M5.5 10h6.25" />
    </svg>
  )
}
