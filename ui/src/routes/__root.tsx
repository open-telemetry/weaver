import { createRootRoute } from '@tanstack/react-router'
import { TanStackRouterDevtools } from '@tanstack/react-router-devtools'
import { AppLayout } from '../components/AppLayout'

export const Route = createRootRoute({
  component: () => (
    <>
      <AppLayout />
      <TanStackRouterDevtools initialIsOpen={false} />
    </>
  ),
})
