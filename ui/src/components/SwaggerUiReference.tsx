import { Suspense, lazy } from 'react'

interface SwaggerUiReferenceProps {
  /** URL of the OpenAPI document to render. */
  specUrl: string
}

// Lazy-load Swagger UI's JS + CSS (large bundle) only when the docs are opened.
// Dark mode is handled in CSS via the app's `[data-theme]` attribute (index.css).
const SwaggerUI = lazy(async () => {
  await import('swagger-ui-react/swagger-ui.css')
  return import('swagger-ui-react')
})

export function SwaggerUiReference({ specUrl }: SwaggerUiReferenceProps) {
  return (
    <Suspense fallback={<div className="p-4">Loading API documentation…</div>}>
      <SwaggerUI url={specUrl} />
    </Suspense>
  )
}
