import { cleanUrl } from "./utils";

type StringOrRegExp = string | RegExp;

// Inline this type to avoid import it from `rolldown`.
// The only downside is we need to keep it in sync with `rolldown` manually,
// it is alright since it is pretty stable now.
type PluginModuleType =
  | 'js'
  | 'jsx'
  | 'ts'
  | 'tsx'
  | 'json'
  | 'text'
  | 'base64'
  | 'dataurl'
  | 'binary'
  | 'empty'
  | (string & {});

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

interface IdParams {
  cleanUrl?: boolean
}
class Id {
  kind: 'id';
  pattern: StringOrRegExp;
  params: IdParams;
  constructor(pattern: StringOrRegExp, params?: IdParams) {
    this.pattern = pattern;
    this.kind = 'id';
    this.params = params ?? {
      cleanUrl: false
    };
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

export function interpreter(
  exprs: TopLevelFilterExpression | TopLevelFilterExpression[],
  code?: string,
  id?: string,
  moduleType?: PluginModuleType,
): boolean {
  let arr: TopLevelFilterExpression[] = [];
  if (Array.isArray(exprs)) {
    arr = exprs;
  } else {
    arr = [exprs];
  }
  return interpreterImpl(arr, code, id, moduleType);
}

export function interpreterImpl(
  expr: TopLevelFilterExpression[],
  code?: string,
  id?: string,
  moduleType?: PluginModuleType,
): boolean {
  let hasInclude = false;
  for (const e of expr) {
    switch (e.kind) {
      case 'include': {
        hasInclude = true;
        if (exprInterpreter(e.expr, code, id, moduleType)) {
          return true;
        }
        break;
      }
      case 'exclude': {
        if (exprInterpreter(e.expr, code, id, moduleType)) {
          return false;
        }
        break;
      }
    }
  }
  return !hasInclude;
}

export function exprInterpreter(
  expr: FilterExpression,
  code?: string,
  id?: string,
  moduleType?: PluginModuleType,
): boolean {
  switch (expr.kind) {
    case 'and': {
      return expr.args.every((e) => exprInterpreter(e, code, id, moduleType));
    }
    case 'or': {
      return expr.args.some((e) => exprInterpreter(e, code, id, moduleType));
    }
    case 'not': {
      return !exprInterpreter(expr.expr, code, id, moduleType);
    }
    case 'id': {
      if (id === undefined) {
        throw new Error('`id` is required for `id` expression');
      }
      if (expr.params.cleanUrl) {
        id = cleanUrl(id)
      }
      return typeof expr.pattern === 'string'
        ? id === expr.pattern
        : expr.pattern.test(id);
    }
    case 'moduleType': {
      if (moduleType === undefined) {
        throw new Error('`moduleType` is required for `moduleType` expression');
      }
      return moduleType === expr.pattern;
    }
    case 'code': {
      if (code === undefined) {
        throw new Error('`code` is required for `code` expression');
      }
      return typeof expr.pattern === 'string'
        ? code.includes(expr.pattern)
        : expr.pattern.test(code);
    }
    default: {
      throw new Error(`Expression kind ${expr.kind} is not expected.`);
    }
  }
}
