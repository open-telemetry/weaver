import type { ReactNode } from 'react'

/** A single value in a chart series. */
export interface Datum {
  name: string
  value: number
}

interface ChartCardProps {
  title: string
  subtitle?: string
  height?: number
  children: ReactNode
}

/** A DaisyUI card wrapper giving a chart a titled, sized surface. */
export function ChartCard({ title, subtitle, height = 288, children }: ChartCardProps) {
  return (
    <div className="card bg-base-100 border border-base-300 shadow-sm">
      <div className="card-body gap-1 p-4 sm:p-5">
        <h3 className="card-title text-base">{title}</h3>
        {subtitle ? <p className="text-sm text-base-content/60">{subtitle}</p> : null}
        <div className="mt-2" style={{ width: '100%', height }}>
          {children}
        </div>
      </div>
    </div>
  )
}

interface TooltipEntry {
  name?: string | number
  value?: number | string
  color?: string
}

interface ChartTooltipProps {
  active?: boolean
  payload?: TooltipEntry[]
  label?: string | number
}

/** Theme-aware tooltip (Recharts' default is a hard-coded white box). */
export function ChartTooltip({ active, payload, label }: ChartTooltipProps) {
  if (!active || !payload || payload.length === 0) return null
  return (
    <div className="rounded-box border border-base-300 bg-base-100 px-3 py-2 text-sm shadow-lg">
      {label !== undefined && label !== '' ? (
        <div className="mb-1 font-semibold break-all">{label}</div>
      ) : null}
      {payload.map((entry, index) => (
        <div key={index} className="flex items-center gap-2">
          <span
            className="inline-block h-2.5 w-2.5 shrink-0 rounded-full"
            style={{ backgroundColor: entry.color }}
          />
          <span className="text-base-content/70">{entry.name}</span>
          <span className="ml-4 font-mono">
            {typeof entry.value === 'number' ? entry.value.toLocaleString() : entry.value}
          </span>
        </div>
      ))}
    </div>
  )
}

/** Keep legend text in neutral ink instead of the (colored) series color. */
export function legendFormatter(value: string): ReactNode {
  return <span className="text-sm text-base-content/80">{value}</span>
}

const STABILITY_LABELS: Record<string, string> = {
  stable: 'Stable',
  release_candidate: 'Release Candidate',
  development: 'Development',
  alpha: 'Alpha',
  beta: 'Beta',
  deprecated: 'Deprecated',
}

export function stabilityLabel(key: string): string {
  return STABILITY_LABELS[key] ?? key
}
