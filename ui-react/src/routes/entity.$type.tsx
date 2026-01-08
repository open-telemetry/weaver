import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/entity/$type')({
  component: EntityDetail,
})

function EntityDetail() {
  const { type } = Route.useParams()
  return (
    <div className="p-2">
      <h3 className="text-xl font-bold">Entity: {type}</h3>
      <p className="mt-2">Entity detail page - to be implemented</p>
    </div>
  )
}