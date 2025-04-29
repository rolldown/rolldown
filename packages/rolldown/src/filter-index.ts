import type { ModuleType as PluginModuleType } from './plugin';
import type { StringOrRegExp } from './types/utils';

export type FilterExpressionKind = FilterExpression['kind'];

export type FilterExpression =
  | And
  | Or
  | Not
  | Id
  | ModuleType
  | Code
  | Include
  | Exclude;

export type TopLevelFilterExpression = Include | Exclude;

export class And {
  kind: 'and';
  args: FilterExpression[];
  constructor(...args: FilterExpression[]) {
    if (args.length === 0) {
      throw new Error('`And` expects at least one operand');
    }
    this.args = args;
    this.kind = 'and';
  }
}
class Or {
  kind: 'or';
  args: FilterExpression[];
  constructor(...args: FilterExpression[]) {
    if (args.length === 0) {
      throw new Error('`Or` expects at least one operand');
    }
    this.args = args;
    this.kind = 'or';
  }
}

class Not {
  kind: 'not';
  expr: FilterExpression;
  constructor(expr: FilterExpression) {
    this.expr = expr;
    this.kind = 'not';
  }
}

class Id {
  kind: 'id';
  pattern: StringOrRegExp;
  constructor(pattern: StringOrRegExp) {
    this.pattern = pattern;
    this.kind = 'id';
  }
}

class ModuleType {
  kind: 'moduleType';
  pattern: PluginModuleType;
  constructor(pattern: PluginModuleType) {
    this.pattern = pattern;
    this.kind = 'moduleType';
  }
}

class Code {
  kind: 'code';
  pattern: StringOrRegExp;
  constructor(expr: StringOrRegExp) {
    this.pattern = expr;
    this.kind = 'code';
  }
}

class Include {
  kind: 'include';
  expr: FilterExpression;
  constructor(expr: FilterExpression) {
    this.expr = expr;
    this.kind = 'include';
  }
}

class Exclude {
  kind: 'exclude';
  expr: FilterExpression;
  constructor(expr: FilterExpression) {
    this.expr = expr;
    this.kind = 'exclude';
  }
}

export function and(...args: FilterExpression[]): And {
  return new And(...args);
}

export function or(...args: FilterExpression[]): Or {
  return new Or(...args);
}

export function not(expr: FilterExpression): Not {
  return new Not(expr);
}

export function id(pattern: StringOrRegExp): Id {
  return new Id(pattern);
}

export function moduleType(pattern: PluginModuleType): ModuleType {
  return new ModuleType(pattern);
}

export function code(pattern: StringOrRegExp): Code {
  return new Code(pattern);
}

export function include(expr: FilterExpression): Include {
  return new Include(expr);
}

export function exclude(expr: FilterExpression): Exclude {
  return new Exclude(expr);
}

export { withFilter } from './plugin';
