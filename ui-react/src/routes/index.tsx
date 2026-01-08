import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/')({
  component: Index,
})

function Index() {
  return (
    <div className="p-2">
      <h3 className="text-xl font-bold">Welcome to Weaver UI (React)</h3>
      <p className="mt-2">This is the new React-based UI for OpenTelemetry Weaver.</p>
    </div>
  )
}