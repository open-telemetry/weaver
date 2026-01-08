import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/metric/$name')({
  component: MetricDetail,
})

function MetricDetail() {
  const { name } = Route.useParams()
  return (
    <div className="p-2">
      <h3 className="text-xl font-bold">Metric: {name}</h3>
      <p className="mt-2">Metric detail page - to be implemented</p>
    </div>
  )
}