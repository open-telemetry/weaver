import type { SchemaProperty } from './api'

export type TypePart = {
  text: string
  isClickable: boolean
}

function getRefType(ref: string): string {
  return ref.replace('#/$defs/', '')
}

export function formatSchemaType(prop: SchemaProperty, skipNull = false): string {
  const hasArrayType = prop.type === 'array' ||
    (Array.isArray(prop.type) && prop.type.includes('array'))
  if (hasArrayType && prop.items) {
    if (prop.items.type === 'array' && prop.items.items) {
      const innerType = prop.items.items.$ref
        ? getRefType(prop.items.items.$ref)
        : prop.items.items.type || 'any'
      return `array of array of ${innerType}`
    }
    const itemType = prop.items.$ref
      ? getRefType(prop.items.$ref)
      : prop.items.type || 'any'
    if (Array.isArray(prop.type) && prop.type.length > 1) {
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
      ? getRefType(prop.additionalProperties.$ref)
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
  if (prop.$ref) return getRefType(prop.$ref)
  if (prop.allOf) {
    if (prop.allOf.length === 1 && prop.allOf[0].$ref) {
      return getRefType(prop.allOf[0].$ref)
    }
    return prop.allOf.map(t => t.$ref ? getRefType(t.$ref) : t.type || 'object').join(' & ')
  }
  if (prop.anyOf) {
    const types = prop.anyOf.map(t => t.$ref ? getRefType(t.$ref) : t.type || 'null')
    const filtered = skipNull ? types.filter(t => t !== 'null') : types
    return filtered.join(' | ')
  }
  if (prop.oneOf) {
    const types = prop.oneOf.map(t => t.$ref ? getRefType(t.$ref) : t.type || 'null')
    const filtered = skipNull ? types.filter(t => t !== 'null') : types
    return filtered.join(' | ')
  }
  return 'unknown'
}

export function parseSchemaTypeString(typeStr: string, isDefinitionRef: (value: string) => boolean): TypePart[] {
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
      { text: innerType, isClickable: isDefinitionRef(innerType) },
    ]
  }

  if (typeStr.startsWith('array of ')) {
    const innerType = typeStr.slice(9)
    return [
      { text: 'array of ', isClickable: false },
      { text: innerType, isClickable: isDefinitionRef(innerType) },
    ]
  }

  if (typeStr.startsWith('map of ')) {
    const valueType = typeStr.slice(7)
    return [
      { text: 'map of ', isClickable: false },
      { text: valueType, isClickable: isDefinitionRef(valueType) },
    ]
  }

  return [{ text: typeStr, isClickable: isDefinitionRef(typeStr) }]
}
