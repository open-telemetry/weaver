import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/search')({
  component: Search,
})

function Search() {
  return (
    <div className="p-2">
      <h3 className="text-xl font-bold">Search</h3>
      <p className="mt-2">Search page - to be implemented</p>
    </div>
  )
}