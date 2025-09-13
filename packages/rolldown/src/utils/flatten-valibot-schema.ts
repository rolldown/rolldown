function unwrapSchema(schema: any): any {
  if (!schema) return schema;

  if (schema.type === 'optional' && schema.wrapped) {
    return unwrapSchema(schema.wrapped);
  }

  if (schema.type === 'nullable' && schema.wrapped) {
    return unwrapSchema(schema.wrapped);
  }

  if (schema.type === 'nullish' && schema.wrapped) {
    return unwrapSchema(schema.wrapped);
  }

  return schema;
}

function getValibotSchemaType(schema: any): string {
  if (!schema) return 'any';

  if (schema.type) {
    switch (schema.type) {
      case 'string':
        return 'string';
      case 'number':
        return 'number';
      case 'boolean':
        return 'boolean';
      case 'array':
        return 'array';
      case 'object':
      case 'strict_object':
      case 'loose_object':
        return 'object';
      case 'union':
        return 'union';
      case 'literal':
        return typeof schema.literal;
      case 'record':
        return 'object';
      case 'optional':
        return getValibotSchemaType(schema.wrapped);
      case 'nullable':
        return getValibotSchemaType(schema.wrapped);
      case 'nullish':
        return getValibotSchemaType(schema.wrapped);
      case 'never':
        return 'never';
      case 'any':
        return 'any';
      case 'custom':
        return 'any';
      case 'function':
        return 'never'; // Functions shouldn't be CLI options
      case 'instance':
        return 'object';
      default:
        return 'any';
    }
  }

  return 'any';
}

function getValibotDescription(schema: any): string | undefined {
  if (!schema) return undefined;

  if (schema.pipe && Array.isArray(schema.pipe)) {
    for (const action of schema.pipe) {
      if (action.type === 'description' && action.description) {
        return action.description;
      }
    }
  }

  if (schema.type === 'optional' && schema.wrapped) {
    return getValibotDescription(schema.wrapped);
  }

  return undefined;
}

export function flattenValibotSchema(
  schema: any,
  result: Record<string, { type: string; description?: string }> = {},
  prefix: string = '',
): Record<string, { type: string; description?: string }> {
  if (!schema || typeof schema !== 'object') return result;

  if (
    schema.type === 'strict_object' || schema.type === 'object' ||
    schema.type === 'loose_object'
  ) {
    if (schema.entries && typeof schema.entries === 'object') {
      for (const [key, value] of Object.entries(schema.entries)) {
        const fullKey = prefix ? `${prefix}.${key}` : key;
        const valueSchema = value as any;

        const type = getValibotSchemaType(valueSchema);
        const description = getValibotDescription(valueSchema);

        if (type === 'object') {
          const unwrappedSchema = unwrapSchema(valueSchema);
          if (unwrappedSchema && unwrappedSchema.entries) {
            flattenValibotSchema(unwrappedSchema, result, fullKey);
          } else {
            result[fullKey] = { type, description };
          }
        } else {
          result[fullKey] = { type, description };
        }
      }
    }
  }

  return result;
}
