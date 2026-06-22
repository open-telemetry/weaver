import { createRoute } from '@tanstack/react-router'
import { useEffect } from 'react'
import { Route as RootRoute } from './__root'

export const Route = createRoute({
  getParentRoute: () => RootRoute,
  path: 'api-docs',
  component: ApiDocs,
})

// Swagger UI lives in AppLayout (kept alive across navigation); this route only
// owns the page title.
function ApiDocs() {
  useEffect(() => {
    document.title = 'API Documentation - Weaver'
  }, [])

  return null
}
