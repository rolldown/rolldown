import type { Schema } from './types'

export function getSchemaType(
  schema: Schema,
): 'string' | 'boolean' | 'object' | 'number' | 'array' {
  if ('anyOf' in schema) {
    const types = schema.anyOf.map((s) => getSchemaType(s))
    // Order: object > array > string > number > boolean
    if (types.includes('object')) return 'object'
    else if (types.includes('array')) return 'array'
    else if (types.includes('string')) return 'string'
    else if (types.includes('number')) return 'number'
    else if (types.includes('boolean')) return 'boolean'
  }

  if ('type' in schema) {
    return schema.type as 'string' | 'boolean' | 'object' | 'number' | 'array'
  }

  return 'object'
}

export function flattenSchema(
  schema: Record<string, Schema>,
  base: Record<string, Schema> = {},
  parent: string = '',
): Record<string, Schema> {
  for (const [k, value] of Object.entries(schema)) {
    const key = parent ? `${parent}.${k}` : k
    if (getSchemaType(value) === 'object') {
      if ('properties' in value) {
        flattenSchema(value.properties, base, key)
      } else {
        base[key] = value
      }
    } else {
      base[key] = value
    }
  }
  return base
}

export function setNestedProperty<T extends object, K>(
  obj: T,
  path: string,
  value: K,
) {
  const keys = path.split('.') as (keyof T)[]
  let current: any = obj

  for (let i = 0; i < keys.length - 1; i++) {
    if (!current[keys[i]]) {
      current[keys[i]] = {}
    }
    current = current[keys[i]]
  }

  const finalKey = keys[keys.length - 1]
  Object.defineProperty(current, finalKey, {
    value: value,
    writable: true,
    enumerable: true,
    configurable: true,
  })
}

export function camelCaseToKebabCase(str: string): string {
  return str.replace(/[A-Z]/g, (match) => `-${match.toLowerCase()}`)
}

export function kebabCaseToCamelCase(str: string): string {
  return str.replace(/-./g, (match) => match[1].toUpperCase())
}
