interface JsonSchema {
  type: string;
  description?: string;
}

export interface ObjectSchema extends JsonSchema {
  type: 'object';
  properties: Record<string, Schema>;
  required?: string[];
  additionalProperties?: boolean | { type: 'string' };
}

interface ArraySchema extends JsonSchema {
  type: 'array';
  items: Schema;
}

interface StringConstantSchema extends JsonSchema {
  type: 'string';
  const: string;
}

interface StringEnumSchema extends JsonSchema {
  type: 'string';
  enum: string[];
}

interface BooleanSchema extends JsonSchema {
  type: 'boolean';
}

type StringSchema = StringConstantSchema | StringEnumSchema;

interface AnyOfSchema {
  anyOf: (StringSchema | ObjectSchema | BooleanSchema | ArraySchema)[];
  description?: string;
}

export type Schema =
  | StringSchema
  | ObjectSchema
  | ArraySchema
  | BooleanSchema
  | AnyOfSchema
  | JsonSchema;
