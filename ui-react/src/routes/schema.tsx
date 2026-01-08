import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/schema')({
  component: Schema,
})

function Schema() {
  return (
    <div className="p-2">
      <h3 className="text-xl font-bold">Schema</h3>
      <p className="mt-2">Schema page - to be implemented</p>
    </div>
  )
}