import { createRoute } from '@tanstack/react-router'
import { useState, useRef, useEffect } from 'react'
import { filterRegistry, ApiError, type FilterErrorDetail } from '../lib/api'
import { Editor, useMonaco } from '@monaco-editor/react'
import { Route as RootRoute } from './__root'
import YAML from 'yaml'

export const Route = createRoute({
  getParentRoute: () => RootRoute,
  path: 'filter_playground',
  component: FilterPlayground,
})

function FilterPlayground() {
  const [filter, setFilter] = useState('.')
  const [result, setResult] = useState<unknown | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [errorDetails, setErrorDetails] = useState<FilterErrorDetail[] | null>(null)
  const [loading, setLoading] = useState(false)
  const [format, setFormat] = useState<'json' | 'yaml'>('json')

  const monaco = useMonaco()
  const editorRef = useRef<any>(null)

  const handleEditorDidMount = (editor: any) => {
    editorRef.current = editor
  }

  useEffect(() => {
    if (monaco && editorRef.current) {
      const model = editorRef.current.getModel()
      if (model) {
        if (errorDetails && errorDetails.length > 0) {
          const markers = errorDetails.map(d => {
            const startLine = d.source ? d.source.start.line : 1
            const startCol = d.source ? d.source.start.col : 1
            const endLine = d.source && d.source.end ? d.source.end.line : startLine
            const endCol = d.source && d.source.end ? d.source.end.col : startCol + 1

            return {
              startLineNumber: startLine,
              startColumn: startCol,
              endLineNumber: endLine,
              endColumn: endCol,
              message: d.error,
              severity: monaco.MarkerSeverity.Error,
            }
          })
          monaco.editor.setModelMarkers(model, 'jq', markers)
        } else {
          monaco.editor.setModelMarkers(model, 'jq', [])
        }
      }
    }
  }, [errorDetails, monaco])

  const handleExecute = async () => {
    setLoading(true)
    setErrorDetails(null)
    setError(null)
    setResult(null)
    try {
      const data = await filterRegistry(filter)
      setResult(data)
    } catch (err: unknown) {
      if (err instanceof ApiError) {
        setError(err.message)
        setErrorDetails(err.details || null)
      } else {
        setError(err instanceof Error ? err.message : 'Unknown error')
      }
    } finally {
      setLoading(false)
    }
  }

  const formattedResult = (() => {
    if (!result) return ''
    if (format === 'json') {
      return JSON.stringify(result, null, 2)
    }
    return YAML.stringify(result)
  })()

  return (
    <div className="space-y-6">
      <h1 className="text-3xl font-bold">JQ Filter Playground</h1>
      
      <div className="form-control w-full">
        <label className="label">
          <span className="label-text font-medium">Filter Expression</span>
        </label>
        <div className="h-32 rounded-lg overflow-hidden border border-base-content/20 shadow-inner">
          <Editor
            height="100%"
            defaultLanguage="plaintext"
            value={filter}
            onChange={(val) => setFilter(val || '')}
            onMount={handleEditorDidMount}
            options={{
              minimap: { enabled: false },
              lineNumbers: 'off',
              padding: { top: 12, bottom: 12 },
              scrollBeyondLastLine: false,
              overviewRulerLanes: 0,
            }}
          />
        </div>
      </div>

      <div className="flex gap-4 items-center">
        <button 
          className="btn btn-primary shadow-sm" 
          onClick={handleExecute}
          disabled={loading}
        >
          {loading ? <span className="loading loading-spinner"></span> : 'Execute'}
        </button>

        <div className="join shadow-sm">
          <input 
            className="join-item btn btn-outline btn-sm" 
            type="radio" 
            name="format" 
            aria-label="JSON" 
            checked={format === 'json'}
            onChange={() => setFormat('json')}
          />
          <input 
            className="join-item btn btn-outline btn-sm" 
            type="radio" 
            name="format" 
            aria-label="YAML" 
            checked={format === 'yaml'}
            onChange={() => setFormat('yaml')}
          />
        </div>
      </div>

      {error && (
        <div className="alert alert-error shadow-sm">
          <svg xmlns="http://www.w3.org/2000/svg" className="stroke-current shrink-0 h-6 w-6" fill="none" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z" /></svg>
          <span className="font-mono text-sm break-all">{error}</span>
        </div>
      )}

      {result !== null && (
        <div className="card bg-base-200 shadow-sm">
          <div className="card-body">
            <h2 className="card-title">Result</h2>
            <pre className="p-4 bg-base-300 rounded-lg overflow-x-auto text-sm shadow-inner font-mono">
              <code>{formattedResult}</code>
            </pre>
          </div>
        </div>
      )}
    </div>
  )
}
