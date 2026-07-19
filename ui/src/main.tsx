import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider, createRouter } from '@tanstack/react-router'
import { getScrollRestorationKey } from './lib/scrollRestorationKey'
import { routeTree } from './routeTree'
import './index.css'

const router = createRouter({
  routeTree,
  scrollRestoration: true,
  getScrollRestorationKey,
})

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
)
