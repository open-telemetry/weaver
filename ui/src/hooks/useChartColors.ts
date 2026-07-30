import { useEffect, useState } from 'react'

/**
 * Theme-aware chart palette derived from DaisyUI CSS custom properties.
 *
 * Recharts needs concrete color strings (not `var(--x)` references, which don't
 * resolve inside SVG presentation attributes), so we resolve the computed values
 * off `document.documentElement` and re-read them whenever the `data-theme`
 * attribute changes (the theme toggle in AppLayout sets it).
 */
export interface ChartColors {
  primary: string
  secondary: string
  accent: string
  info: string
  success: string
  warning: string
  error: string
  neutral: string
  /** Neutral ink for axis labels / legend text. */
  baseContent: string
  /** The chart surface color (used for the gap ring between adjacent marks). */
  base100: string
  /** Recessive color for grid lines. */
  grid: string
  /** Fixed categorical order (identity) — never cycled; overflow folds into "Other". */
  categorical: string[]
  /** Status colors keyed by stability level (mirrors StabilityBadge). */
  stability: Record<string, string>
}

function readVar(styles: CSSStyleDeclaration, name: string, fallback: string): string {
  const value = styles.getPropertyValue(name).trim()
  return value || fallback
}

function computeColors(): ChartColors {
  const styles = getComputedStyle(document.documentElement)
  const primary = readVar(styles, '--color-primary', '#570df8')
  const secondary = readVar(styles, '--color-secondary', '#f000b8')
  const accent = readVar(styles, '--color-accent', '#37cdbe')
  const info = readVar(styles, '--color-info', '#3abff8')
  const success = readVar(styles, '--color-success', '#36d399')
  const warning = readVar(styles, '--color-warning', '#fbbd23')
  const error = readVar(styles, '--color-error', '#f87272')
  const neutral = readVar(styles, '--color-neutral', '#2a323c')
  const baseContent = readVar(styles, '--color-base-content', '#1f2937')
  const base100 = readVar(styles, '--color-base-100', '#ffffff')
  const grid = readVar(styles, '--color-base-300', '#d1d5db')

  return {
    primary,
    secondary,
    accent,
    info,
    success,
    warning,
    error,
    neutral,
    baseContent,
    base100,
    grid,
    // Distinct hues in a fixed order; excludes the reserved status colors.
    categorical: [primary, secondary, accent, info, neutral],
    stability: {
      stable: success,
      release_candidate: accent,
      development: warning,
      alpha: info,
      beta: info,
      deprecated: error,
    },
  }
}

export function useChartColors(): ChartColors {
  const [colors, setColors] = useState<ChartColors>(computeColors)

  useEffect(() => {
    const update = () => setColors(computeColors())
    update()
    const observer = new MutationObserver(update)
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['data-theme'],
    })
    return () => observer.disconnect()
  }, [])

  return colors
}
