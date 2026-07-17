import type { ParsedLocation } from '@tanstack/react-router'

// Routes in this set rewrite their own URL in place (history.replace) as
// filters and expansion state change. Per-history-entry keys (the default)
// would give every interaction a fresh key and drop the scroll snapshot, so
// these are keyed by pathname instead. Other routes keep the default key.
//
// Shared between the router's own `getScrollRestorationKey` (main.tsx, used
// when TanStack saves/restores automatically) and any route that needs to
// look up a cached position itself via `useElementScrollRestoration` - both
// must derive the exact same key or the lookup silently misses.
const replaceNavigatedRoutes = new Set(['/search'])

export function getScrollRestorationKey(location: ParsedLocation): string {
  return replaceNavigatedRoutes.has(location.pathname)
    ? location.pathname
    : (location.state.__TSR_key ?? location.href)
}
