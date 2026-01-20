import { createRoute } from '@tanstack/react-router'
import { createElement, useEffect, useRef, useState } from 'react'
import { Route as RootRoute } from './__root'

type Theme = 'light' | 'dark'

const RAPIDOC_SCRIPT_SRC = 'https://unpkg.com/rapidoc/dist/rapidoc-min.js'
const RAPIDOC_SCRIPT_ATTR = 'data-rapidoc-script'

export const Route = createRoute({
  getParentRoute: () => RootRoute,
  path: 'api-docs',
  component: ApiDocs,
})

function ApiDocs() {
  const rapidocRef = useRef<HTMLElement | null>(null)
  const [currentTheme, setCurrentTheme] = useState<Theme>('light')
  const themeRef = useRef<Theme>('light')

  const applyTheme = (theme: Theme) => {
    if (themeRef.current !== theme) {
      themeRef.current = theme
      setCurrentTheme(theme)
    }

    const rapidocElement = rapidocRef.current
    if (!rapidocElement) return

    const isDark = theme === 'dark'
    rapidocElement.setAttribute('theme', isDark ? 'dark' : 'light')
    rapidocElement.setAttribute('bg-color', isDark ? '#1d232a' : '#ffffff')
    rapidocElement.setAttribute('text-color', isDark ? '#a6adba' : '#000000')
    rapidocElement.setAttribute('header-color', isDark ? '#1d232a' : '#f3f4f6')
    rapidocElement.setAttribute('primary-color', isDark ? '#3abff8' : '#0ea5e9')
    rapidocElement.setAttribute('nav-bg-color', isDark ? '#1d232a' : '#f3f4f6')
    rapidocElement.setAttribute('nav-text-color', isDark ? '#a6adba' : '#1f2937')
    rapidocElement.setAttribute('nav-hover-bg-color', isDark ? '#2a323c' : '#e5e7eb')
    rapidocElement.setAttribute('nav-hover-text-color', isDark ? '#ffffff' : '#000000')
    rapidocElement.setAttribute('nav-accent-color', isDark ? '#3abff8' : '#0ea5e9')
  }

  useEffect(() => {
    document.title = 'API Documentation - Weaver'
    const initialTheme =
      (document.documentElement.getAttribute('data-theme') as Theme | null) || 'light'
    applyTheme(initialTheme)
  }, [])

  useEffect(() => {
    const existingScript = document.querySelector(`script[${RAPIDOC_SCRIPT_ATTR}]`)
    if (!existingScript) {
      const script = document.createElement('script')
      script.type = 'module'
      script.src = RAPIDOC_SCRIPT_SRC
      script.setAttribute(RAPIDOC_SCRIPT_ATTR, 'true')
      document.head.appendChild(script)
    }
  }, [])

  useEffect(() => {
    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        if (mutation.attributeName === 'data-theme') {
          const newTheme =
            (document.documentElement.getAttribute('data-theme') as Theme | null) || 'light'
          if (newTheme !== themeRef.current) {
            applyTheme(newTheme)
          }
        }
      }
    })

    observer.observe(document.documentElement, { attributes: true })
    return () => observer.disconnect()
  }, [])

  return (
    <div className="api-docs-container">
      {createElement('rapi-doc', {
        ref: rapidocRef,
        'spec-url': '/api/v1/openapi.json',
        theme: currentTheme === 'dark' ? 'dark' : 'light',
        'bg-color': currentTheme === 'dark' ? '#1d232a' : '#ffffff',
        'text-color': currentTheme === 'dark' ? '#a6adba' : '#000000',
        'header-color': currentTheme === 'dark' ? '#1d232a' : '#f3f4f6',
        'primary-color': currentTheme === 'dark' ? '#3abff8' : '#0ea5e9',
        'nav-bg-color': currentTheme === 'dark' ? '#1d232a' : '#f3f4f6',
        'nav-text-color': currentTheme === 'dark' ? '#a6adba' : '#1f2937',
        'nav-hover-bg-color': currentTheme === 'dark' ? '#2a323c' : '#e5e7eb',
        'nav-hover-text-color': currentTheme === 'dark' ? '#ffffff' : '#000000',
        'nav-accent-color': currentTheme === 'dark' ? '#3abff8' : '#0ea5e9',
        'render-style': 'read',
        layout: 'column',
        'schema-style': 'tree',
        'show-header': 'false',
        'allow-try': 'true',
        'allow-server-selection': 'false',
        'allow-authentication': 'false',
        style: { height: '100%', width: '100%' },
      })}
    </div>
  )
}
