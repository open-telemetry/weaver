import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/attribute/$key')({
  component: AttributeDetail,
})

function AttributeDetail() {
  const { key } = Route.useParams()
  return (
    <div className="p-2">
      <h3 className="text-xl font-bold">Attribute: {key}</h3>
      <p className="mt-2">Attribute detail page - to be implemented</p>
    </div>
  )
}