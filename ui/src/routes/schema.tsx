import { createRoute, useNavigate, useSearch } from '@tanstack/react-router'
import { useState, useEffect, useMemo } from 'react'
import { Markdown } from '../components/Markdown'
import { InlineMarkdown } from '../components/InlineMarkdown'
import { getSchema, type SchemaProperty, type SchemaResponse } from '../lib/api'
import { Route as RootRoute } from './__root'

type SchemaSearch = {
  schema?: string
  type?: string
}

type TypePart = {
  text: string
  isClickable: boolean
}

export const Route = createRoute({
  getParentRoute: () => RootRoute,
  path: 'schema',
  component: Schema,
})

function Schema() {
  const navigate = useNavigate()
  const search = useSearch({ from: '/schema' })

  const [schema, setSchema] = useState<SchemaResponse | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [selectedDefinition, setSelectedDefinition] = useState<string | null>(null)
  const [showRoot, setShowRoot] = useState(true)

  useEffect(() => {
    async function fetchSchema() {
      setLoading(true)
      setError(null)

      const schemaParam = search.schema || 'ForgeRegistryV2'

      try {
        const data = await getSchema(schemaParam)
        setSchema(data)
      } catch (e) {
        setError(e instanceof Error ? e.message : 'Unknown error')
      } finally {
        setLoading(false)
      }
    }

    fetchSchema()
  }, [search.schema])

  useEffect(() => {
    const typeParam = search.type
    if (typeParam === 'root' || !typeParam) {
      setShowRoot(true)
      setSelectedDefinition(null)
    } else {
      setShowRoot(false)
      setSelectedDefinition(typeParam)
    }
  }, [search.type])

  const definitions = useMemo(() => {
    if (!schema?.definitions && !schema?.$defs) return []
    const defs = schema.definitions || schema.$defs || {}
    return Object.keys(defs).sort()
  }, [schema])

  function selectDefinition(name: string) {
    navigate({
      to: '/schema',
      search: (prev: SchemaSearch) => ({
        ...prev,
        type: name,
        schema: prev.schema || 'ForgeRegistryV2'
      })
    })
  }

  function selectRoot() {
    navigate({
      to: '/schema',
      search: (prev: SchemaSearch) => ({
        ...prev,
        type: 'root',
        schema: prev.schema || 'ForgeRegistryV2'
      })
    })
  }

  function formatType(prop: SchemaProperty, skipNull = false): string {
    const hasArrayType = prop.type === 'array' ||
                         (Array.isArray(prop.type) && prop.type.includes('array'))
    if (hasArrayType && prop.items) {
      if (prop.items.type === 'array' && prop.items.items) {
        const innerType = prop.items.items.$ref
          ? prop.items.items.$ref.replace('#/definitions/', '').replace('#/$defs/', '')
          : prop.items.items.type || 'any'
        return `array of array of ${innerType}`
      }
      const itemType = prop.items.$ref
        ? prop.items.$ref.replace('#/definitions/', '').replace('#/$defs/', '')
        : prop.items.type || 'any'
      if (Array.isArray(prop.type) && prop.type.length >1) {
        const otherTypes = prop.type.filter(t => t !== 'array' && (!skipNull || t !== 'null'))
        if (otherTypes.length > 0) {
          return `array of ${itemType} | ${otherTypes.join(' | ')}`
        }
      }
      return `array of ${itemType}`
    }

    const hasObjectType = prop.type === 'object' ||
                         (Array.isArray(prop.type) && prop.type.includes('object'))
    if (hasObjectType && prop.additionalProperties) {
      const valueType = prop.additionalProperties.$ref
        ? prop.additionalProperties.$ref.replace('#/definitions/', '').replace('#/$defs/', '')
        : prop.additionalProperties.type || 'any'
      if (Array.isArray(prop.type) && prop.type.length > 1) {
        const otherTypes = prop.type.filter(t => t !== 'object' && (!skipNull || t !== 'null'))
        if (otherTypes.length > 0) {
          return `map of ${valueType} | ${otherTypes.join(' | ')}`
        }
      }
      return `map of ${valueType}`
    }

    if (Array.isArray(prop.type)) {
      const types = skipNull ? prop.type.filter(t => t !== 'null') : prop.type
      return types.join(' | ')
    }
    if (prop.type) return prop.type
    if (prop.$ref) return prop.$ref.replace('#/definitions/', '').replace('#/$defs/', '')
    if (prop.allOf) {
      if (prop.allOf.length === 1 && prop.allOf[0].$ref) {
        return prop.allOf[0].$ref.replace('#/definitions/', '').replace('#/$defs/', '')
      }
      return prop.allOf.map(t => t.$ref ? t.$ref.replace('#/definitions/', '').replace('#/$defs/', '') : t.type || 'object').join(' & ')
    }
    if (prop.anyOf) {
      const types = prop.anyOf.map(t => t.$ref ? t.$ref.replace('#/definitions/', '').replace('#/$defs/', '') : t.type || 'null')
      const filtered = skipNull ? types.filter(t => t !== 'null') : types
      return filtered.join(' | ')
    }
    if (prop.oneOf) {
      const types = prop.oneOf.map(t => t.$ref ? t.$ref.replace('#/definitions/', '').replace('#/$defs/', '') : t.type || 'null')
      const filtered = skipNull ? types.filter(t => t !== 'null') : types
      return filtered.join(' | ')
    }
    return 'unknown'
  }

  function isDefinitionRef(typeStr: string): boolean {
    if (!typeStr || typeStr === 'unknown') return false
    const simpleTypes = ['string', 'number', 'boolean', 'object', 'array', 'null', 'integer', 'any']
    if (simpleTypes.includes(typeStr)) return false
    if (typeStr.startsWith('array of ')) return false
    if (typeStr.startsWith('map of ')) return false
    if (typeStr.includes(' | ')) return false
    const defs = schema?.definitions || schema?.$defs || {}
    return defs[typeStr] !== undefined
  }

  function parseTypeString(typeStr: string): TypePart[] {
    if (!typeStr) return []

    if (typeStr.includes(' | ')) {
      const parts = typeStr.split(' | ')
      const result: TypePart[] = []

      parts.forEach((part, index) => {
        const trimmedPart = part.trim()

        if (trimmedPart.startsWith('map of ')) {
          const valueType = trimmedPart.slice(7)
          result.push({ text: 'map of ', isClickable: false })
          result.push({ text: valueType, isClickable: isDefinitionRef(valueType) })
        } else if (trimmedPart.startsWith('array of array of ')) {
          const innerType = trimmedPart.slice(18)
          result.push({ text: 'array of array of ', isClickable: false })
          result.push({ text: innerType, isClickable: isDefinitionRef(innerType) })
        } else if (trimmedPart.startsWith('array of ')) {
          const innerType = trimmedPart.slice(9)
          result.push({ text: 'array of ', isClickable: false })
          result.push({ text: innerType, isClickable: isDefinitionRef(innerType) })
        } else {
          result.push({ text: trimmedPart, isClickable: isDefinitionRef(trimmedPart) })
        }

        if (index < parts.length - 1) {
          result.push({ text: ' | ', isClickable: false })
        }
      })

      return result
    }

    if (typeStr.startsWith('array of array of ')) {
      const innerType = typeStr.slice(18)
      return [
        { text: 'array of array of ', isClickable: false },
        { text: innerType, isClickable: isDefinitionRef(innerType) }
      ]
    }

    if (typeStr.startsWith('array of ')) {
      const innerType = typeStr.slice(9)
      return [
        { text: 'array of ', isClickable: false },
        { text: innerType, isClickable: isDefinitionRef(innerType) }
      ]
    }

    if (typeStr.startsWith('map of ')) {
      const valueType = typeStr.slice(7)
      return [
        { text: 'map of ', isClickable: false },
        { text: valueType, isClickable: isDefinitionRef(valueType) }
      ]
    }

    return [{ text: typeStr, isClickable: isDefinitionRef(typeStr) }]
  }

  return (
    <div className="flex gap-4 h-[calc(100vh-4rem)]">
      <div className="w-80 border-r border-base-300 overflow-y-auto pr-4">
        {loading ? (
          <div className="flex justify-center py-8">
            <span className="loading loading-spinner loading-lg"></span>
          </div>
        ) : error ? (
          <div className="alert alert-error">
            <span>Error: {error}</span>
          </div>
        ) : schema ? (
          <div className="space-y-4">
            <button
              onClick={() => selectRoot()}
              className={`card sticky top-0 z-10 w-full text-left transition-colors cursor-pointer ${
                showRoot ? 'bg-primary text-primary-content' : 'bg-base-200 hover:bg-base-300'
              }`}
            >
              <div className="card-body p-4">
                <h1 className="font-bold text-xl">{schema.title || 'Schema'}</h1>
                <p className="text-sm opacity-70">{schema.description || ''}</p>
              </div>
            </button>

            <div>
              <h3 className="font-bold text-sm mb-2 text-base-content/70">
                TYPE DEFINITIONS ({definitions.length})
              </h3>
              <div className="space-y-1">
                {definitions.map(def => (
                  <button
                    key={def}
                    onClick={() => selectDefinition(def)}
                    className={`w-full text-left px-3 py-2 rounded transition-colors text-sm ${
                      selectedDefinition === def
                        ? 'bg-primary text-primary-content'
                        : 'hover:bg-base-200'
                    }`}
                  >
                    <code className="font-mono">{def}</code>
                  </button>
                ))}
              </div>
            </div>
          </div>
        ) : null}
      </div>

      <div className="flex-1 overflow-y-auto">
        {showRoot && schema && (schema.properties || schema.oneOf || schema.anyOf) ? (
          <div className="space-y-6">
            <div>
              <h2 className="text-2xl font-mono font-bold mb-2">Root</h2>
              {schema.description ? (
                <div className="text-base-content/70">
                  <Markdown content={schema.description} />
                </div>
              ) : (
                <div className="text-base-content/70">
                  <p>Top-level properties of schema</p>
                </div>
              )}
              <div className="mt-2">
                {schema.type ? (
                  <span className="badge badge-outline">{schema.type}</span>
                ) : schema.oneOf ? (
                  <span className="badge badge-outline">oneOf</span>
                ) : schema.anyOf ? (
                  <span className="badge badge-outline">anyOf</span>
                ) : null}
              </div>
            </div>

            {(schema.oneOf || schema.anyOf) && (
              <div>
                <h3 className="text-lg font-bold mb-3">{schema.oneOf ? 'One Of' : 'Any Of'}</h3>
                <div className="space-y-2">
                  {(schema.oneOf || schema.anyOf)!.map((variant, idx) => {
                    const variantType = formatType(variant)
                    const variantParts = parseTypeString(variantType)
                    return (
                      <div key={idx} className="card bg-base-200">
                        <div className="card-body p-4">
                          {variant.enum && variant.enum.length === 1 ? (
                            <code className="badge badge-outline font-mono">{String(variant.enum[0])}</code>
                          ) : variant.type === 'object' && variant.properties ? (
                            <span className="badge badge-outline font-mono">object</span>
                          ) : (
                            <span className="badge badge-outline font-mono inline-flex items-center gap-1">
                              {variantParts.map((part, i) => (
                                <span key={i}>
                                  {part.isClickable ? (
                                    <button
                                      className="hover:text-primary cursor-pointer underline"
                                      onClick={() => selectDefinition(part.text)}
                                    >
                                      {part.text}
                                    </button>
                                  ) : (
                                    <span>{part.text}</span>
                                  )}
                                </span>
                              ))}
                            </span>
                          )}
                          {variant.description && (
                            <div className="text-sm mt-2">
                              <Markdown content={variant.description} />
                            </div>
                          )}

                          {variant.type === 'object' && variant.properties && (
                            <div className="mt-3">
                              <table className="table table-xs">
                                <thead>
                                  <tr>
                                    <th>Field</th>
                                    <th>Type</th>
                                    <th>Required</th>
                                    <th>Description</th>
                                  </tr>
                                </thead>
                                <tbody>
                                  {Object.entries(variant.properties).map(([propName, propDef]) => {
                                    const isRequired = variant.required?.includes(propName)
                                    const typeStr = formatType(propDef, !isRequired)
                                    const typeParts = parseTypeString(typeStr)
                                    return (
                                      <tr key={propName}>
                                        <td>
                                          <code className="font-mono text-xs">{propName}</code>
                                        </td>
                                        <td>
                                          <span className="badge badge-xs badge-outline font-mono inline-flex items-center gap-1">
                                            {typeParts.map((part, i) => (
                                              <span key={i}>
                                                {part.isClickable ? (
                                                  <button
                                                    className="hover:text-primary cursor-pointer underline"
                                                    onClick={() => selectDefinition(part.text)}
                                                  >
                                                    {part.text}
                                                  </button>
                                                ) : (
                                                  <span>{part.text}</span>
                                                )}
                                              </span>
                                            ))}
                                          </span>
                                          {propDef.enum && (
                                            <div className="flex flex-wrap gap-1 mt-1">
                                              {propDef.enum.map(enumVal => (
                                                <code key={String(enumVal)} className="badge badge-xs">
                                                  {String(enumVal)}
                                                </code>
                                              ))}
                                            </div>
                                          )}
                                        </td>
                                        <td>
                                          {isRequired ? (
                                            <span className="badge badge-xs badge-error">required</span>
                                          ) : (
                                            <span className="badge badge-xs badge-ghost">optional</span>
                                          )}
                                        </td>
                                        <td className="text-xs">
                                          {propDef.description ? (
                                            <InlineMarkdown content={propDef.description} />
                                          ) : (
                                            '-'
                                          )}
                                        </td>
                                      </tr>
                                    )
                                  })}
                                </tbody>
                              </table>
                            </div>
                          )}
                        </div>
                      </div>
                    )
                  })}
                </div>
              </div>
            )}

            {schema.properties && schema.required && schema.required.length > 0 && (
              <div>
                <h3 className="text-lg font-bold mb-3">Required Fields</h3>
                <div className="flex flex-wrap gap-2">
                  {schema.required.map(field => (
                    <span key={field} className="badge badge-error">{field}</span>
                  ))}
                </div>
              </div>
            )}

            {schema.properties && (
              <div>
                <h3 className="text-lg font-bold mb-3">Properties</h3>
                <div className="overflow-x-auto">
                  <table className="table table-zebra">
                    <thead>
                      <tr>
                        <th>Field</th>
                        <th>Type</th>
                        <th>Required</th>
                        <th>Description</th>
                      </tr>
                    </thead>
                    <tbody>
                      {Object.entries(schema.properties).map(([propName, propDef]) => {
                        const isRequired = schema.required?.includes(propName)
                        const typeStr = formatType(propDef, !isRequired)
                        const typeParts = parseTypeString(typeStr)
                        return (
                          <tr key={propName}>
                            <td>
                              <code className="font-mono text-sm">{propName}</code>
                            </td>
                            <td>
                              <span className="badge badge-sm badge-outline font-mono inline-flex items-center gap-1">
                                {typeParts.map((part, i) => (
                                  <span key={i}>
                                    {part.isClickable ? (
                                      <button
                                        className="hover:text-primary cursor-pointer underline"
                                        onClick={() => selectDefinition(part.text)}
                                      >
                                        {part.text}
                                      </button>
                                    ) : (
                                      <span>{part.text}</span>
                                    )}
                                  </span>
                                ))}
                              </span>
                            </td>
                            <td>
                              {isRequired ? (
                                <span className="badge badge-sm badge-error">required</span>
                              ) : (
                                <span className="badge badge-sm badge-ghost">optional</span>
                              )}
                            </td>
                            <td className="text-sm max-w-md">
                              {propDef.description ? (
                                <InlineMarkdown content={propDef.description} />
                              ) : (
                                '-'
                              )}
                            </td>
                          </tr>
                        )
                      })}
                    </tbody>
                  </table>
                </div>
              </div>
            )}

            <div>
              <h3 className="text-lg font-bold mb-3">Raw JSON</h3>
              <div className="mockup-code bg-base-300">
                <pre className="text-xs overflow-x-auto text-base-content">
                  <code>
                    {JSON.stringify(
                      schema.properties
                        ? {
                            type: 'object',
                            properties: schema.properties,
                            required: schema.required,
                          }
                        : schema,
                      null,
                      2
                    )}
                  </code>
                </pre>
              </div>
            </div>
          </div>
        ) : selectedDefinition && ((schema?.definitions || schema?.$defs || {})[selectedDefinition]) ? (
          (() => {
            const def = (schema.definitions || schema.$defs || {})[selectedDefinition]
            return (
              <div className="space-y-6">
                <div>
                  <h2 className="text-2xl font-mono font-bold mb-2">{selectedDefinition}</h2>
                  {def.description && (
                    <div className="text-base-content/70">
                      <Markdown content={def.description} />
                    </div>
                  )}
                  {def.type && (
                    <div className="mt-2">
                      <span className="badge badge-outline">{def.type}</span>
                    </div>
                  )}
                </div>

                {def.required && def.required.length > 0 && (
                  <div>
                    <h3 className="text-lg font-bold mb-3">Required Fields</h3>
                    <div className="flex flex-wrap gap-2">
                      {def.required.map(field => (
                        <span key={field} className="badge badge-error">{field}</span>
                      ))}
                    </div>
                  </div>
                )}

                {def.properties && (
                  <div>
                    <h3 className="text-lg font-bold mb-3">Properties</h3>
                    <div className="overflow-x-auto">
                      <table className="table table-zebra">
                        <thead>
                          <tr>
                            <th>Field</th>
                            <th>Type</th>
                            <th>Required</th>
                            <th>Description</th>
                          </tr>
                        </thead>
                        <tbody>
                          {Object.entries(def.properties).map(([propName, propDef]) => {
                            const isRequired = def.required?.includes(propName)
                            const typeStr = formatType(propDef, !isRequired)
                            const typeParts = parseTypeString(typeStr)
                            return (
                              <tr key={propName}>
                                <td>
                                  <code className="font-mono text-sm">{propName}</code>
                                </td>
                                <td>
                                  <span className="badge badge-sm badge-outline font-mono inline-flex items-center gap-1">
                                    {typeParts.map((part, i) => (
                                      <span key={i}>
                                        {part.isClickable ? (
                                          <button
                                            className="hover:text-primary cursor-pointer underline"
                                            onClick={() => selectDefinition(part.text)}
                                          >
                                            {part.text}
                                          </button>
                                        ) : (
                                          <span>{part.text}</span>
                                        )}
                                      </span>
                                    ))}
                                  </span>
                                </td>
                                <td>
                                  {isRequired ? (
                                    <span className="badge badge-sm badge-error">required</span>
                                  ) : (
                                    <span className="badge badge-sm badge-ghost">optional</span>
                                  )}
                                </td>
                                <td className="text-sm max-w-md">
                                  {propDef.description ? (
                                    <InlineMarkdown content={propDef.description} />
                                  ) : (
                                    '-'
                                  )}
                                </td>
                              </tr>
                            )
                          })}
                        </tbody>
                      </table>
                    </div>
                  </div>
                )}

                {def.enum && (
                  <div>
                    <h3 className="text-lg font-bold mb-3">Allowed Values</h3>
                    <div className="flex flex-wrap gap-2">
                      {def.enum.map(value => (
                        <code key={String(value)} className="badge badge-lg font-mono">
                          {String(value)}
                        </code>
                      ))}
                    </div>
                  </div>
                )}

                {(def.oneOf || def.anyOf) && (
                  <div>
                    <h3 className="text-lg font-bold mb-3">{def.oneOf ? 'One Of' : 'Any Of'}</h3>
                    <div className="space-y-2">
                      {(def.oneOf || def.anyOf)!.map((variant, idx) => {
                        const variantType = formatType(variant)
                        const variantParts = parseTypeString(variantType)
                        return (
                          <div key={idx} className="card bg-base-200">
                            <div className="card-body p-4">
                              {variant.enum && variant.enum.length === 1 ? (
                                <code className="badge badge-outline font-mono">{String(variant.enum[0])}</code>
                              ) : variant.type === 'object' && variant.properties ? (
                                <span className="badge badge-outline font-mono">object</span>
                              ) : (
                                <span className="badge badge-outline font-mono inline-flex items-center gap-1">
                                  {variantParts.map((part, i) => (
                                    <span key={i}>
                                      {part.isClickable ? (
                                        <button
                                          className="hover:text-primary cursor-pointer underline"
                                          onClick={() => selectDefinition(part.text)}
                                        >
                                          {part.text}
                                        </button>
                                      ) : (
                                        <span>{part.text}</span>
                                      )}
                                    </span>
                                  ))}
                                </span>
                              )}
                              {variant.description && (
                                <div className="text-sm mt-2">
                                  <Markdown content={variant.description} />
                                </div>
                              )}

                              {variant.type === 'object' && variant.properties && (
                                <div className="mt-3">
                                  <table className="table table-xs">
                                    <thead>
                                      <tr>
                                        <th>Field</th>
                                        <th>Type</th>
                                        <th>Required</th>
                                        <th>Description</th>
                                      </tr>
                                    </thead>
                                    <tbody>
                                      {Object.entries(variant.properties).map(([propName, propDef]) => {
                                        const isRequired = variant.required?.includes(propName)
                                        const typeStr = formatType(propDef, !isRequired)
                                        const typeParts = parseTypeString(typeStr)
                                        return (
                                          <tr key={propName}>
                                            <td>
                                              <code className="font-mono text-xs">{propName}</code>
                                            </td>
                                            <td>
                                              <span className="badge badge-xs badge-outline font-mono inline-flex items-center gap-1">
                                                {typeParts.map((part, i) => (
                                                  <span key={i}>
                                                    {part.isClickable ? (
                                                      <button
                                                        className="hover:text-primary cursor-pointer underline"
                                                        onClick={() => selectDefinition(part.text)}
                                                      >
                                                        {part.text}
                                                      </button>
                                                    ) : (
                                                      <span>{part.text}</span>
                                                    )}
                                                  </span>
                                                ))}
                                              </span>
                                              {propDef.enum && (
                                                <div className="flex flex-wrap gap-1 mt-1">
                                                  {propDef.enum.map(enumVal => (
                                                    <code key={String(enumVal)} className="badge badge-xs">
                                                      {String(enumVal)}
                                                    </code>
                                                  ))}
                                                </div>
                                              )}
                                            </td>
                                            <td>
                                              {isRequired ? (
                                                <span className="badge badge-xs badge-error">required</span>
                                              ) : (
                                                <span className="badge badge-xs badge-ghost">optional</span>
                                              )}
                                            </td>
                                            <td className="text-xs">
                                              {propDef.description ? (
                                                <InlineMarkdown content={propDef.description} />
                                              ) : (
                                                '-'
                                              )}
                                            </td>
                                          </tr>
                                        )
                                      })}
                                    </tbody>
                                  </table>
                                </div>
                              )}
                            </div>
                          </div>
                        )
                      })}
                    </div>
                  </div>
                )}

                <div>
                  <h3 className="text-lg font-bold mb-3">Raw JSON</h3>
                  <div className="mockup-code bg-base-300">
                    <pre className="text-xs overflow-x-auto text-base-content">
                      <code>{JSON.stringify(def, null, 2)}</code>
                    </pre>
                  </div>
                </div>
              </div>
            )
          })()
        ) : (
          <div className="flex items-center justify-center h-full text-base-content/50">
            <div className="text-center">
              <svg xmlns="http://www.w3.org/2000/svg" className="h-16 w-16 mx-auto mb-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
              </svg>
              <p className="text-lg">Select a type definition to view details</p>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
