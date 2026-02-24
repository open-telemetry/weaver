import { createRootRoute } from '@tanstack/react-router'
import { TanStackRouterDevtools } from '@tanstack/react-router-devtools'
import { AppLayout } from '../components/AppLayout'
import { ErrorBoundary } from '../components/ErrorBoundary'

export const Route = createRootRoute({
  component: () => (
    <ErrorBoundary>
      <AppLayout />
      <TanStackRouterDevtools initialIsOpen={false} />
    </ErrorBoundary>
  ),
})
