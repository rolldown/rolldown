import { ModuleType as PluginModuleType } from './plugin';
import { StringOrRegExp } from './types/utils';

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
  left: FilterExpression;
  right: FilterExpression;
  constructor(left: FilterExpression, right: FilterExpression) {
    this.left = left;
    this.right = right;
    this.kind = 'and';
  }
}
class Or {
  kind: 'or';
  left: FilterExpression;
  right: FilterExpression;
  constructor(left: FilterExpression, right: FilterExpression) {
    this.left = left;
    this.right = right;
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
  return {
    kind: 'id',
    pattern,
  };
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
