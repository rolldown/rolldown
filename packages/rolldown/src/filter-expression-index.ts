import type { ModuleType as PluginModuleType } from './plugin';
import type { StringOrRegExp } from './types/utils';

interface FilterExpression {
  kind: string;
}

interface And extends FilterExpression {
  kind: 'and';
  left: FilterExpression;
  right: FilterExpression;
}

interface Or extends FilterExpression {
  kind: 'or';
  left: FilterExpression;
  right: FilterExpression;
}

interface Not extends FilterExpression {
  kind: 'not';
  expr: FilterExpression;
}

interface Id extends FilterExpression {
  kind: 'id';
  pattern: StringOrRegExp;
}

interface ModuleType extends FilterExpression {
  kind: 'moduleType';
  pattern: string;
}

interface Code extends FilterExpression {
  kind: 'code';
  pattern: StringOrRegExp;
}

interface Include extends FilterExpression {
  kind: 'include';
  expr: FilterExpression;
}

interface Exclude extends FilterExpression {
  kind: 'exclude';
  expr: FilterExpression;
}

export function and(left: FilterExpression, right: FilterExpression): And {
  return { kind: 'and', left, right };
}

export function or(left: FilterExpression, right: FilterExpression): Or {
  return { kind: 'or', left, right };
}

export function not(expr: FilterExpression): Not {
  return { kind: 'not', expr };
}

export function id(pattern: StringOrRegExp): Id {
  return { kind: 'id', pattern };
}

export function moduleType(pattern: PluginModuleType): ModuleType {
  return { kind: 'moduleType', pattern };
}

export function code(pattern: StringOrRegExp): Code {
  return { kind: 'code', pattern };
}

export function include(expr: FilterExpression): Include {
  return { kind: 'include', expr };
}

export function exclude(expr: FilterExpression): Exclude {
  return { kind: 'exclude', expr };
}
