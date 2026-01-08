import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/span/$type')({
  component: SpanDetail,
})

function SpanDetail() {
  const { type } = Route.useParams()
  return (
    <div className="p-2">
      <h3 className="text-xl font-bold">Span: {type}</h3>
      <p className="mt-2">Span detail page - to be implemented</p>
    </div>
  )
}