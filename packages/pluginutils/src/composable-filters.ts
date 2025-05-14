import { cleanUrl, extractQueryWithoutFragment } from './utils';

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

export type FilterExpression = And | Or | Not | Id | ModuleType | Code | Query;

export type TopLevelFilterExpression = Include | Exclude;

class And {
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

export interface QueryFilterObject {
  [key: string]: StringOrRegExp | boolean;
}

interface IdParams {
  cleanUrl?: boolean;
}

class Id {
  kind: 'id';
  pattern: StringOrRegExp;
  params: IdParams;
  constructor(pattern: StringOrRegExp, params?: IdParams) {
    this.pattern = pattern;
    this.kind = 'id';
    this.params = params ?? {
      cleanUrl: false,
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

class Query {
  kind: 'query';
  key: string;
  pattern: StringOrRegExp | boolean;
  constructor(key: string, pattern: StringOrRegExp | boolean) {
    this.pattern = pattern;
    this.key = key;
    this.kind = 'query';
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

export function id(pattern: StringOrRegExp, params?: IdParams): Id {
  return new Id(pattern, params);
}

export function moduleType(pattern: PluginModuleType): ModuleType {
  return new ModuleType(pattern);
}

export function code(pattern: StringOrRegExp): Code {
  return new Code(pattern);
}

/*
 * There are three kinds of conditions are supported:
 * 1. `boolean`: if the value is `true`, the key must exist and be truthy. if the value is `false`, the key must not exist or be falsy.
 * 2. `string`: the key must exist and be equal to the value.
 * 3. `RegExp`: the key must exist and match the value.
 */
export function query(key: string, pattern: StringOrRegExp | boolean): Query {
  return new Query(key, pattern);
}

export function include(expr: FilterExpression): Include {
  return new Include(expr);
}

export function exclude(expr: FilterExpression): Exclude {
  return new Exclude(expr);
}

/**
 * convert a queryObject to FilterExpression like
 * ```js
 *   and(query(k1, v1), query(k2, v2))
 * ```
 * @param queryFilterObject The query filter object needs to be matched.
 * @returns a `And` FilterExpression
 */
export function queries(queryFilter: QueryFilterObject): And {
  let arr = Object.entries(queryFilter).map(([key, value]) => {
    return new Query(key, value);
  });
  return and(...arr);
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

interface InterpreterCtx {
  urlSearchParamsCache?: URLSearchParams;
}

export function interpreterImpl(
  expr: TopLevelFilterExpression[],
  code?: string,
  id?: string,
  moduleType?: PluginModuleType,
  ctx: InterpreterCtx = {},
): boolean {
  let hasInclude = false;
  for (const e of expr) {
    switch (e.kind) {
      case 'include': {
        hasInclude = true;
        if (exprInterpreter(e.expr, code, id, moduleType, ctx)) {
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
  ctx: InterpreterCtx = {},
): boolean {
  switch (expr.kind) {
    case 'and': {
      return expr.args.every((e) =>
        exprInterpreter(e, code, id, moduleType, ctx)
      );
    }
    case 'or': {
      return expr.args.some((e) =>
        exprInterpreter(e, code, id, moduleType, ctx)
      );
    }
    case 'not': {
      return !exprInterpreter(expr.expr, code, id, moduleType, ctx);
    }
    case 'id': {
      if (id === undefined) {
        throw new Error('`id` is required for `id` expression');
      }
      if (expr.params.cleanUrl) {
        id = cleanUrl(id);
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
    case 'query': {
      if (id === undefined) {
        throw new Error('`id` is required for `Query` expression');
      }
      if (!ctx.urlSearchParamsCache) {
        let queryString = extractQueryWithoutFragment(id);
        ctx.urlSearchParamsCache = new URLSearchParams(queryString);
      }
      let urlParams = ctx.urlSearchParamsCache;
      if (typeof expr.pattern === 'boolean') {
        if (expr.pattern) {
          return urlParams.has(expr.key);
        } else {
          return !urlParams.has(expr.key);
        }
      } else if (typeof expr.pattern === 'string') {
        return urlParams.get(expr.key) === expr.pattern;
      } else {
        return expr.pattern.test(urlParams.get(expr.key) ?? '');
      }
    }
    default: {
      throw new Error(`Expression ${JSON.stringify(expr)} is not expected.`);
    }
  }
}
