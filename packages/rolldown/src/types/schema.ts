export interface JsonSchema {
  type: string;
  description?: string;
}

export interface ObjectSchema extends JsonSchema {
  type: 'object';
  properties: Record<string, Schema>;
  required?: string[];
  additionalProperties?: boolean | { type: 'string' };
}

export interface ArraySchema extends JsonSchema {
  type: 'array';
  items: Schema;
}

export interface StringConstantSchema extends JsonSchema {
  type: 'string';
  const: string;
}

export interface StringEnumSchema extends JsonSchema {
  type: 'string';
  enum: string[];
}

export interface BooleanSchema extends JsonSchema {
  type: 'boolean';
}

export type StringSchema = StringConstantSchema | StringEnumSchema;

export interface AnyOfSchema {
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
