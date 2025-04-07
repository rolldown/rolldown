import type { Schema } from '../../types/schema';

type SchemaType = 'string' | 'boolean' | 'object' | 'number' | 'array';

const priority: SchemaType[] = [
  'object',
  'array',
  'string',
  'number',
  'boolean',
];

export function getSchemaType(schema: Schema): SchemaType {
  if ('anyOf' in schema) {
    const types: SchemaType[] = schema.anyOf.map(getSchemaType);

    // Order: object > array > string > number > boolean
    let result: SchemaType | undefined = priority.find((type) =>
      types.includes(type)
    );

    if (result) {
      return result;
    }
  }

  if ('type' in schema) {
    return schema.type as SchemaType;
  }

  if ('const' in schema) {
    return typeof schema.const as SchemaType;
  }

  return 'object';
}

export function flattenSchema(
  schema: Record<string, Schema>,
  base: Record<string, Schema> = {},
  parent: string = '',
): Record<string, Schema> {
  if (schema === undefined) {
    return base;
  }

  for (const [k, value] of Object.entries(schema)) {
    const key = parent ? `${parent}.${k}` : k;
    if (getSchemaType(value) === 'object') {
      if ('properties' in value) {
        flattenSchema(value.properties, base, key);
      } else {
        base[key] = value;
      }
    } else {
      base[key] = value;
    }
  }

  return base;
}

export function setNestedProperty<T extends object, K>(
  obj: T,
  path: string,
  value: K,
): void {
  const keys = path.split('.') as (keyof T)[];
  let current: any = obj;

  for (let i = 0; i < keys.length - 1; i++) {
    if (!current[keys[i]]) {
      current[keys[i]] = {};
    }
    current = current[keys[i]];
  }

  const finalKey = keys[keys.length - 1];
  Object.defineProperty(current, finalKey, {
    value: value,
    writable: true,
    enumerable: true,
    configurable: true,
  });
}

export function camelCaseToKebabCase(str: string): string {
  return str.replace(/[A-Z]/g, (match) => `-${match.toLowerCase()}`);
}

export function kebabCaseToCamelCase(str: string): string {
  return str.replace(/-./g, (match) => match[1].toUpperCase());
}
