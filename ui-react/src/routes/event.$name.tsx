import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/event/$name')({
  component: EventDetail,
})

function EventDetail() {
  const { name } = Route.useParams()
  return (
    <div className="p-2">
      <h3 className="text-xl font-bold">Event: {name}</h3>
      <p className="mt-2">Event detail page - to be implemented</p>
    </div>
  )
}