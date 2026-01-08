import { createRootRoute, createRoute, createFileRoute } from '@tanstack/react-router'
import * as React from 'react'

const RootComponent = React.lazy(() => import('./routes/__root'))
const IndexComponent = React.lazy(() => import('./routes/index'))
const SearchComponent = React.lazy(() => import('./routes/search'))
const StatsComponent = React.lazy(() => import('./routes/stats'))
const SchemaComponent = React.lazy(() => import('./routes/schema'))
const ApiDocsComponent = React.lazy(() => import('./routes/api-docs'))
const AttributeComponent = React.lazy(() => import('./routes/attribute.$key'))
const MetricComponent = React.lazy(() => import('./routes/metric.$name'))
const SpanComponent = React.lazy(() => import('./routes/span.$type'))
const EventComponent = React.lazy(() => import('./routes/event.$name'))
const EntityComponent = React.lazy(() => import('./routes/entity.$type'))

export const routeTree = createRootRoute({
  component: RootComponent,
}).children([
  createFileRoute('/')({
    component: IndexComponent,
  }),
  createFileRoute('/search')({
    component: SearchComponent,
  }),
  createFileRoute('/stats')({
    component: StatsComponent,
  }),
  createFileRoute('/schema')({
    component: SchemaComponent,
  }),
  createFileRoute('/api-docs')({
    component: ApiDocsComponent,
  }),
  createFileRoute('/attribute/$key')({
    component: AttributeComponent,
  }),
  createFileRoute('/metric/$name')({
    component: MetricComponent,
  }),
  createFileRoute('/span/$type')({
    component: SpanComponent,
  }),
  createFileRoute('/event/$name')({
    component: EventComponent,
  }),
  createFileRoute('/entity/$type')({
    component: EntityComponent,
  }),
])
