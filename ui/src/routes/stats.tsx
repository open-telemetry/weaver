import { createRoute, Link } from '@tanstack/react-router'
import { useEffect, useMemo, useState } from 'react'
import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  LabelList,
  Legend,
  Pie,
  PieChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { getRegistryStats } from '../lib/api'
import type { Breakdown, CommonSignalStats, RegistryStats } from '../lib/api'
import { useChartColors } from '../hooks/useChartColors'
import type { ChartColors } from '../hooks/useChartColors'
import {
  ChartCard,
  ChartTooltip,
  legendFormatter,
  stabilityLabel,
  type Datum,
} from '../components/charts'
import { Route as RootRoute } from './__root'

export const Route = createRoute({
  getParentRoute: () => RootRoute,
  path: 'stats',
  component: Stats,
})

// --- data transforms -------------------------------------------------------

/** Order stability segments deterministically, known levels first. */
const STABILITY_ORDER = ['stable', 'release_candidate', 'development', 'alpha', 'beta']

function sortedData(map: Breakdown): Datum[] {
  return Object.entries(map)
    .map(([name, value]) => ({ name, value }))
    .sort((a, b) => b.value - a.value || a.name.localeCompare(b.name))
}

function topN(data: Datum[], n: number): Datum[] {
  if (data.length <= n) return data
  const head = data.slice(0, n)
  const rest = data.slice(n).reduce((sum, d) => sum + d.value, 0)
  return rest > 0 ? [...head, { name: 'Other', value: rest }] : head
}

/** Collapse the many `enum(card:NNN)` buckets into a single `enum` bar. */
function groupAttributeTypes(map: Breakdown): Datum[] {
  const grouped: Record<string, number> = {}
  for (const [key, value] of Object.entries(map)) {
    const label = key.startsWith('enum(') ? 'enum' : key
    grouped[label] = (grouped[label] ?? 0) + value
  }
  return sortedData(grouped)
}

/**
 * Turn the `enum(card:NNN)` buckets into a cardinality distribution: how many
 * enum attributes have a given number of members. Ordered by member count.
 */
function enumCardinalityDistribution(map: Breakdown): Datum[] {
  return Object.entries(map)
    .map(([key, value]) => {
      const match = key.match(/^enum\(card:(\d+)\)$/)
      return match ? { name: String(Number(match[1])), value } : null
    })
    .filter((datum): datum is Datum => datum !== null)
    .sort((a, b) => Number(a.name) - Number(b.name))
}

interface StabilityRow {
  name: string
  [level: string]: string | number
}

// --- charts ----------------------------------------------------------------

const axisTick = (colors: ChartColors) => ({ fill: colors.baseContent, fontSize: 12 })

/** Horizontal stacked bar of stability composition across every signal type. */
function StabilityChart({ rows, colors }: { rows: StabilityRow[]; colors: ChartColors }) {
  const levels = useMemo(() => {
    const present = new Set<string>()
    for (const row of rows) {
      for (const key of Object.keys(row)) {
        if (key !== 'name') present.add(key)
      }
    }
    return [...present].sort(
      (a, b) => STABILITY_ORDER.indexOf(a) - STABILITY_ORDER.indexOf(b)
    )
  }, [rows])

  return (
    <ResponsiveContainer width="100%" height="100%">
      <BarChart data={rows} layout="vertical" margin={{ left: 8, right: 16, top: 4 }}>
        <CartesianGrid horizontal={false} stroke={colors.grid} strokeDasharray="3 3" />
        <XAxis type="number" allowDecimals={false} tick={axisTick(colors)} stroke={colors.grid} />
        <YAxis
          type="category"
          dataKey="name"
          width={72}
          tick={axisTick(colors)}
          stroke={colors.grid}
        />
        <Tooltip content={<ChartTooltip />} cursor={{ fill: colors.grid, opacity: 0.2 }} isAnimationActive={false} />
        <Legend formatter={legendFormatter} />
        {levels.map((level) => (
          <Bar
            key={level}
            dataKey={level}
            name={stabilityLabel(level)}
            stackId="stability"
            fill={colors.stability[level] ?? colors.neutral}
            stroke={colors.base100}
            strokeWidth={2}
            radius={2}
          />
        ))}
      </BarChart>
    </ResponsiveContainer>
  )
}

/** Horizontal magnitude bars, single hue, with value labels at the ends. */
function HorizontalBars({
  data,
  colors,
  color,
  yWidth = 120,
}: {
  data: Datum[]
  colors: ChartColors
  color: string
  yWidth?: number
}) {
  return (
    <ResponsiveContainer width="100%" height="100%">
      <BarChart data={data} layout="vertical" margin={{ left: 8, right: 40, top: 4 }}>
        <CartesianGrid horizontal={false} stroke={colors.grid} strokeDasharray="3 3" />
        <XAxis type="number" allowDecimals={false} tick={axisTick(colors)} stroke={colors.grid} />
        <YAxis
          type="category"
          dataKey="name"
          width={yWidth}
          tick={axisTick(colors)}
          stroke={colors.grid}
          interval={0}
        />
        <Tooltip content={<ChartTooltip />} cursor={{ fill: colors.grid, opacity: 0.2 }} isAnimationActive={false} />
        <Bar dataKey="value" name="Count" fill={color} radius={[0, 4, 4, 0]}>
          <LabelList
            dataKey="value"
            position="right"
            style={{ fill: colors.baseContent, fontSize: 11 }}
          />
        </Bar>
      </BarChart>
    </ResponsiveContainer>
  )
}

/** Vertical magnitude bars for a small distribution. */
function VerticalBars({
  data,
  colors,
  color,
  xLabel,
  showLabels = true,
}: {
  data: Datum[]
  colors: ChartColors
  color: string
  xLabel?: string
  showLabels?: boolean
}) {
  return (
    <ResponsiveContainer width="100%" height="100%">
      <BarChart data={data} margin={{ left: 4, right: 8, top: 20, bottom: xLabel ? 20 : 4 }}>
        <CartesianGrid vertical={false} stroke={colors.grid} strokeDasharray="3 3" />
        <XAxis
          dataKey="name"
          tick={axisTick(colors)}
          stroke={colors.grid}
          label={
            xLabel
              ? {
                  value: xLabel,
                  position: 'insideBottom',
                  offset: -8,
                  fill: colors.baseContent,
                  fontSize: 12,
                }
              : undefined
          }
        />
        <YAxis allowDecimals={false} tick={axisTick(colors)} stroke={colors.grid} width={36} />
        <Tooltip content={<ChartTooltip />} cursor={{ fill: colors.grid, opacity: 0.2 }} isAnimationActive={false} />
        <Bar dataKey="value" name="Count" fill={color} radius={[4, 4, 0, 0]}>
          {showLabels ? (
            <LabelList
              dataKey="value"
              position="top"
              style={{ fill: colors.baseContent, fontSize: 11 }}
            />
          ) : null}
        </Bar>
      </BarChart>
    </ResponsiveContainer>
  )
}

/** Donut for a categorical breakdown. */
function DonutChart({ data, colors }: { data: Datum[]; colors: ChartColors }) {
  return (
    <ResponsiveContainer width="100%" height="100%">
      <PieChart>
        <Pie
          data={data}
          dataKey="value"
          nameKey="name"
          innerRadius="55%"
          outerRadius="82%"
          paddingAngle={2}
          stroke={colors.base100}
          strokeWidth={2}
        >
          {data.map((entry, index) => (
            <Cell key={entry.name} fill={colors.categorical[index % colors.categorical.length]} />
          ))}
        </Pie>
        <Tooltip content={<ChartTooltip />} isAnimationActive={false} />
        <Legend formatter={legendFormatter} />
      </PieChart>
    </ResponsiveContainer>
  )
}

interface CoverageRow {
  name: string
  documented: number
  undocumented: number
}

/** Stacked documentation coverage (documented vs. undocumented) per signal type. */
function CoverageChart({ rows, colors }: { rows: CoverageRow[]; colors: ChartColors }) {
  return (
    <ResponsiveContainer width="100%" height="100%">
      <BarChart data={rows} layout="vertical" margin={{ left: 8, right: 16, top: 4 }}>
        <CartesianGrid horizontal={false} stroke={colors.grid} strokeDasharray="3 3" />
        <XAxis type="number" allowDecimals={false} tick={axisTick(colors)} stroke={colors.grid} />
        <YAxis
          type="category"
          dataKey="name"
          width={72}
          tick={axisTick(colors)}
          stroke={colors.grid}
        />
        <Tooltip content={<ChartTooltip />} cursor={{ fill: colors.grid, opacity: 0.2 }} isAnimationActive={false} />
        <Legend formatter={legendFormatter} />
        <Bar
          dataKey="documented"
          name="Documented"
          stackId="cov"
          fill={colors.success}
          stroke={colors.base100}
          strokeWidth={2}
          radius={2}
        />
        <Bar
          dataKey="undocumented"
          name="Undocumented"
          stackId="cov"
          fill={colors.warning}
          stroke={colors.base100}
          strokeWidth={2}
          radius={2}
        />
      </BarChart>
    </ResponsiveContainer>
  )
}

function SummaryStat({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="rounded-box bg-base-200/50 px-4 py-3">
      <div className="text-xs uppercase tracking-wide text-base-content/60">{label}</div>
      <div className="mt-1 font-mono text-2xl font-semibold">
        {typeof value === 'number' ? value.toLocaleString() : value}
      </div>
    </div>
  )
}

/** A compact key-figures panel that partners a chart for a signal type. */
function SignalSummary({ title, common }: { title: string; common: CommonSignalStats }) {
  const documentedPct = common.count
    ? Math.round((common.total_with_note / common.count) * 100)
    : 0
  return (
    <div className="card bg-base-100 border border-base-300 shadow-sm">
      <div className="card-body justify-center gap-1 p-4 sm:p-5">
        <h3 className="card-title text-base">{title}</h3>
        <div className="mt-2 grid grid-cols-2 gap-3">
          <SummaryStat label="Total" value={common.count} />
          <SummaryStat label="Stable" value={common.stability_breakdown.stable ?? 0} />
          <SummaryStat label="Deprecated" value={common.deprecated_count} />
          <SummaryStat label="Documented" value={`${documentedPct}%`} />
        </div>
      </div>
    </div>
  )
}

// --- page ------------------------------------------------------------------

function Stats() {
  const [stats, setStats] = useState<RegistryStats | null>(null)
  const [error, setError] = useState<string | null>(null)
  const colors = useChartColors()

  useEffect(() => {
    let isMounted = true
    getRegistryStats()
      .then((data) => {
        if (isMounted) setStats(data)
      })
      .catch((err: unknown) => {
        if (isMounted) setError(err instanceof Error ? err.message : 'Unknown error')
      })
    return () => {
      isMounted = false
    }
  }, [])

  if (error) {
    return (
      <div className="space-y-6">
        <h1 className="text-3xl font-bold">Registry Stats</h1>
        <div className="alert alert-error" role="alert">
          <span>Error loading registry stats: {error}</span>
        </div>
      </div>
    )
  }

  if (!stats) {
    return (
      <div className="space-y-6">
        <h1 className="text-3xl font-bold">Registry Stats</h1>
        <div className="flex justify-center py-12">
          <span className="loading loading-spinner loading-lg" aria-label="Loading" />
        </div>
      </div>
    )
  }

  const r = stats.registry

  const kpis = [
    { label: 'Attributes', value: r.attributes.attribute_count, type: 'attribute' },
    { label: 'Metrics', value: r.metrics.common.count, type: 'metric' },
    { label: 'Spans', value: r.spans.common.count, type: 'span' },
    { label: 'Events', value: r.events.common.count, type: 'event' },
    { label: 'Entities', value: r.entities.common.count, type: 'entity' },
  ]

  // Only include signal types that actually have members, so empty rows/sections
  // don't clutter the dashboard.
  const signalBreakdowns = [
    {
      name: 'Attributes',
      count: r.attributes.attribute_count,
      stability: r.attributes.stability_breakdown,
      deprecated: r.attributes.deprecated_count,
    },
    {
      name: 'Metrics',
      count: r.metrics.common.count,
      stability: r.metrics.common.stability_breakdown,
      deprecated: r.metrics.common.deprecated_count,
    },
    {
      name: 'Spans',
      count: r.spans.common.count,
      stability: r.spans.common.stability_breakdown,
      deprecated: r.spans.common.deprecated_count,
    },
    {
      name: 'Events',
      count: r.events.common.count,
      stability: r.events.common.stability_breakdown,
      deprecated: r.events.common.deprecated_count,
    },
    {
      name: 'Entities',
      count: r.entities.common.count,
      stability: r.entities.common.stability_breakdown,
      deprecated: r.entities.common.deprecated_count,
    },
  ].filter((signal) => signal.count > 0)

  const stabilityRows: StabilityRow[] = signalBreakdowns.map((signal) => ({
    name: signal.name,
    ...signal.stability,
  }))

  const deprecatedData: Datum[] = signalBreakdowns.map((signal) => ({
    name: signal.name,
    value: signal.deprecated,
  }))

  const attributeTypeData = groupAttributeTypes(r.attributes.attribute_type_breakdown)
  const enumCardinalityData = enumCardinalityDistribution(r.attributes.attribute_type_breakdown)
  const instrumentData = sortedData(r.metrics.instrument_breakdown)
  const unitData = topN(sortedData(r.metrics.unit_breakdown), 12)
  const spanKindData = sortedData(r.spans.span_kind_breakdown)

  const identityData = Object.entries(r.entities.entity_identity_length_distribution)
    .map(([name, value]) => ({ name, value }))
    .sort((a, b) => Number(a.name) - Number(b.name))

  const coverageRows: CoverageRow[] = [
    { name: 'Metrics', signal: r.metrics.common },
    { name: 'Spans', signal: r.spans.common },
    { name: 'Events', signal: r.events.common },
    { name: 'Entities', signal: r.entities.common },
  ]
    .filter(({ signal }) => signal.count > 0)
    .map(({ name, signal }) => ({
      name,
      documented: signal.total_with_note,
      undocumented: Math.max(0, signal.count - signal.total_with_note),
    }))

  const totalDeprecated = deprecatedData.reduce((sum, d) => sum + d.value, 0)

  return (
    <div className="space-y-8">
      <div className="space-y-1">
        <h1 className="text-3xl font-bold">Registry Stats</h1>
        {stats.schema_url ? (
          <p className="text-sm text-base-content/70">
            Source:{' '}
            <a href={stats.schema_url} target="_blank" className="link" rel="noreferrer">
              {stats.schema_url}
            </a>
          </p>
        ) : null}
      </div>

      {/* KPI tiles */}
      <div className="stats stats-vertical lg:stats-horizontal shadow w-full">
        {kpis.map((kpi) => (
          <Link
            key={kpi.type}
            to="/search"
            search={{ type: kpi.type }}
            className="stat hover:bg-base-300 cursor-pointer transition-colors"
          >
            <div className="stat-title">{kpi.label}</div>
            <div className="stat-value">{kpi.value.toLocaleString()}</div>
            <div className="stat-desc">Click to browse</div>
          </Link>
        ))}
      </div>

      {/* Stability & deprecation */}
      {signalBreakdowns.length > 0 ? (
        <section className="space-y-4">
          <h2 className="text-xl font-semibold">Stability &amp; Deprecation</h2>
          <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
            <ChartCard
              title="Stability by signal type"
              subtitle="Maturity composition across the registry"
            >
              <StabilityChart rows={stabilityRows} colors={colors} />
            </ChartCard>
            <ChartCard
              title="Deprecated items"
              subtitle={`${totalDeprecated.toLocaleString()} deprecated definitions in total`}
            >
              <HorizontalBars
                data={deprecatedData}
                colors={colors}
                color={colors.error}
                yWidth={72}
              />
            </ChartCard>
          </div>
        </section>
      ) : null}

      {/* Attributes */}
      {r.attributes.attribute_count > 0 ? (
        <section className="space-y-4">
          <h2 className="text-xl font-semibold">Attributes</h2>
          <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
            <ChartCard
              title="Attribute types"
              subtitle="Enum variants are grouped into a single bucket"
              height={320}
            >
              <HorizontalBars
                data={attributeTypeData}
                colors={colors}
                color={colors.primary}
                yWidth={140}
              />
            </ChartCard>
            <ChartCard
              title="Enum cardinality"
              subtitle="Enum attributes grouped by number of members"
              height={320}
            >
              {enumCardinalityData.length > 0 ? (
                <VerticalBars
                  data={enumCardinalityData}
                  colors={colors}
                  color={colors.secondary}
                  xLabel="Members"
                  showLabels={enumCardinalityData.length <= 12}
                />
              ) : (
                <div className="flex h-full items-center justify-center text-sm text-base-content/60">
                  No enum attributes
                </div>
              )}
            </ChartCard>
          </div>
        </section>
      ) : null}

      {/* Metrics */}
      {r.metrics.common.count > 0 ? (
        <section className="space-y-4">
          <h2 className="text-xl font-semibold">Metrics</h2>
          <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
            <ChartCard title="Instruments" subtitle="Distribution of metric instrument kinds">
              <DonutChart data={instrumentData} colors={colors} />
            </ChartCard>
            <SignalSummary title="Metric summary" common={r.metrics.common} />
          </div>
          <ChartCard
            title="Top units"
            subtitle="Most common metric units (rest grouped as Other)"
          >
            <HorizontalBars data={unitData} colors={colors} color={colors.secondary} yWidth={110} />
          </ChartCard>
        </section>
      ) : null}

      {/* Spans */}
      {r.spans.common.count > 0 ? (
        <section className="space-y-4">
          <h2 className="text-xl font-semibold">Spans</h2>
          <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
            <ChartCard title="Span kinds" subtitle="Distribution of span kinds">
              <DonutChart data={spanKindData} colors={colors} />
            </ChartCard>
            <SignalSummary title="Span summary" common={r.spans.common} />
          </div>
        </section>
      ) : null}

      {/* Entities */}
      {r.entities.common.count > 0 ? (
        <section className="space-y-4">
          <h2 className="text-xl font-semibold">Entities</h2>
          <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
            <ChartCard
              title="Entity identity size"
              subtitle="Number of identifying attributes per entity"
            >
              <VerticalBars
                data={identityData}
                colors={colors}
                color={colors.accent}
                xLabel="Identity attributes"
              />
            </ChartCard>
            <SignalSummary title="Entity summary" common={r.entities.common} />
          </div>
        </section>
      ) : null}

      {/* Documentation coverage */}
      {coverageRows.length > 0 ? (
        <section className="space-y-4">
          <h2 className="text-xl font-semibold">Documentation Coverage</h2>
          <ChartCard
            title="Documented vs. undocumented"
            subtitle="Signals carrying an explanatory note"
          >
            <CoverageChart rows={coverageRows} colors={colors} />
          </ChartCard>
        </section>
      ) : null}
    </div>
  )
}
