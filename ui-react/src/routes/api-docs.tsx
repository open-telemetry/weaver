import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/api-docs')({
  component: ApiDocs,
})

function ApiDocs() {
  return (
    <div className="p-2">
      <h3 className="text-xl font-bold">API Documentation</h3>
      <p className="mt-2">API Docs page - to be implemented</p>
    </div>
  )
}