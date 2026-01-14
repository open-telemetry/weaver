import { Route as RootRoute } from './routes/__root'
import { Route as IndexRoute } from './routes/index'
import { Route as SearchRoute } from './routes/search'
import { Route as StatsRoute } from './routes/stats'
import { Route as SchemaRoute } from './routes/schema'
import { Route as ApiDocsRoute } from './routes/api-docs'
import { Route as AttributeRoute } from './routes/attribute.$key'
import { Route as MetricRoute } from './routes/metric.$name'
import { Route as SpanRoute } from './routes/span.$type'
import { Route as EventRoute } from './routes/event.$name'
import { Route as EntityRoute } from './routes/entity.$type'

export const routeTree = RootRoute.addChildren([
  IndexRoute,
  SearchRoute,
  StatsRoute,
  SchemaRoute,
  ApiDocsRoute,
  AttributeRoute,
  MetricRoute,
  SpanRoute,
  EventRoute,
  EntityRoute,
])
