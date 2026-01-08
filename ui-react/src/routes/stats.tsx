import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/stats')({
  component: Stats,
})

function Stats() {
  return (
    <div className="p-2">
      <h3 className="text-xl font-bold">Registry Statistics</h3>
      <p className="mt-2">Stats page - to be implemented</p>
    </div>
  )
}