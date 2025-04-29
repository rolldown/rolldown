import * as R from 'remeda';
import type { BindingFilterToken, BindingHookFilter } from '../binding.d';
import type { FilterExpression } from '../filter-expression-index';
import * as filter from '../filter-expression-index';
import { arraify } from '../utils/misc';
import type { HookFilterExtension } from '.';
import type { GeneralHookFilter } from './hook-filter';

// Convert `exclude` and `include` to tokens of FilterExpr
// Array of `BindingFilterToken` will be converted to `FilterExpr` finally,
// use `generalHookFilterToFilterExprs` instead of `generalHookFilterToFilterArrayOfArrayBindingFilterToken` would be concise
function generalHookFilterMatcherToFilterExprs(
  matcher: GeneralHookFilter,
  stringKind: 'code' | 'id',
): filter.TopLevelFilterExpression[] | undefined {
  if (typeof matcher === 'string' || matcher instanceof RegExp) {
    return [filter.include(filter.id(matcher))];
  }
  if (Array.isArray(matcher)) {
    return matcher.map((m) => filter.include(filter.id(m)));
  }
  if (matcher.custom) {
    return matcher.custom;
  }
  let ret: filter.TopLevelFilterExpression[] = [];
  let isCode = stringKind === 'code';
  if (matcher.exclude) {
    ret.push(
      ...arraify(matcher.exclude).map((m) =>
        filter.exclude(isCode ? filter.code(m) : filter.id(m))
      ),
    );
  }
  if (matcher.include) {
    ret.push(
      ...arraify(matcher.include).map((m) =>
        filter.include(isCode ? filter.code(m) : filter.id(m))
      ),
    );
  }
  return ret;
}

// TODO: support variadic `or` and `and`
function transformFilterMatcherToFilterExprs(
  filterOption: HookFilterExtension<'transform'>['filter'],
): filter.TopLevelFilterExpression[] | undefined {
  if (!filterOption) {
    return undefined;
  }
  const { id, code, moduleType, custom } = filterOption;

  if (custom) {
    return custom;
  }
  let ret: filter.TopLevelFilterExpression[] = [];
  let idIncludes: filter.TopLevelFilterExpression[] = [];
  let idExcludes: filter.TopLevelFilterExpression[] = [];
  let codeIncludes: filter.TopLevelFilterExpression[] = [];
  let codeExcludes: filter.TopLevelFilterExpression[] = [];
  if (id) {
    [idIncludes, idExcludes] = R.partition(
      generalHookFilterMatcherToFilterExprs(id, 'id') ?? [],
      (m) => m.kind === 'include',
    );
  }
  if (code) {
    [codeIncludes, codeExcludes] = R.partition(
      generalHookFilterMatcherToFilterExprs(code, 'code') ?? [],
      (m) => m.kind === 'include',
    );
  }
  ret.push(...idExcludes);
  ret.push(...codeExcludes);

  let cursor: filter.FilterExpression | undefined;
  if (moduleType) {
    let moduleTypes = Array.isArray(moduleType)
      ? moduleType
      : moduleType.include ?? [];
    cursor = joinFilterExprsWithOr(
      moduleTypes.map((m) => filter.moduleType(m)),
    );
  }
  if (idIncludes.length) {
    let joinedOrExpr = joinFilterExprsWithOr(
      idIncludes.map((item) => item.expr),
    );
    if (!cursor) {
      cursor = joinedOrExpr;
    } else {
      cursor = filter.and(cursor, joinedOrExpr);
    }
  }

  if (codeIncludes.length) {
    let joinedOrExpr = joinFilterExprsWithOr(
      codeIncludes.map((item) => item.expr),
    );
    if (!cursor) {
      cursor = joinedOrExpr;
    } else {
      cursor = filter.and(cursor, joinedOrExpr);
    }
  }
  if (cursor) {
    ret.push(filter.include(cursor));
  }
  return ret;
}

// This is temp function, it is no more used when we support variadic `or` and `and`
// Convert List of `TopLevelFilterExpression` to a `FilterExpression`
// if the length is one then return the first element
// or recursively join the elements with `or`
function joinFilterExprsWithOr(
  filterExprs: FilterExpression[],
): FilterExpression {
  if (filterExprs.length === 1) {
    return filterExprs[0];
  }
  return filter.or(filterExprs[0], joinFilterExprsWithOr(filterExprs.slice(1)));
}

export function bindingifyGeneralHookFilter(
  matcher: GeneralHookFilter,
  stringKind: 'code' | 'id',
): BindingHookFilter | undefined {
  let filterExprs = generalHookFilterMatcherToFilterExprs(matcher, stringKind);
  let ret: BindingFilterToken[][] = [];
  if (filterExprs) {
    ret = filterExprs.map(bindingifyFilterExpr);
  }
  return ret.length > 0
    ? {
      value: ret,
    }
    : undefined;
}

function bindingifyFilterExpr(expr: FilterExpression): BindingFilterToken[] {
  let list: BindingFilterToken[] = [];
  bindingifyFilterExprImpl(expr, list);
  return list;
}
function bindingifyFilterExprImpl(
  expr: FilterExpression,
  list: BindingFilterToken[],
) {
  switch (expr.kind) {
    case 'and': {
      bindingifyFilterExprImpl(expr.right, list);
      bindingifyFilterExprImpl(expr.left, list);
      list.push({
        kind: 'And',
      });
      break;
    }
    case 'not': {
      bindingifyFilterExprImpl(expr.expr, list);
      list.push({
        kind: 'Not',
      });
      break;
    }
    case 'id': {
      list.push({ kind: 'Id', value: expr.pattern });
      break;
    }
    case 'moduleType': {
      list.push({ kind: 'ModuleType', value: expr.pattern });
      break;
    }
    case 'code': {
      list.push({ kind: 'Code', value: expr.pattern });
      break;
    }
    case 'include': {
      bindingifyFilterExprImpl(expr.expr, list);
      list.push({ kind: 'Include' });
      break;
    }
    case 'exclude': {
      bindingifyFilterExprImpl(expr.expr, list);
      list.push({ kind: 'Exclude' });
      break;
    }
    default:
      throw new Error(`Unknown filter expression kind: ${expr.kind}`);
  }
}

export function bindingifyResolveIdFilter(
  filterOption?: HookFilterExtension<'resolveId'>['filter'],
): BindingHookFilter | undefined {
  return filterOption?.id
    ? bindingifyGeneralHookFilter(filterOption.id, 'id')
    : undefined;
}

export function bindingifyLoadFilter(
  filterOption?: HookFilterExtension<'load'>['filter'],
): BindingHookFilter | undefined {
  return filterOption?.id
    ? bindingifyGeneralHookFilter(filterOption.id, 'id')
    : undefined;
}

export function bindingifyTransformFilter(
  filterOption?: HookFilterExtension<'transform'>['filter'],
): BindingHookFilter | undefined {
  if (!filterOption) {
    return undefined;
  }

  let custom = transformFilterMatcherToFilterExprs(filterOption);

  let ret: BindingFilterToken[][] = [];
  if (custom) {
    ret = custom.map(bindingifyFilterExpr);
  }
  return {
    value: ret.length > 0 ? ret : undefined,
  };
}

export function bindingifyRenderChunkFilter(
  filterOption?: HookFilterExtension<'renderChunk'>['filter'],
): BindingHookFilter | undefined {
  if (filterOption) {
    const { code } = filterOption;

    return code ? bindingifyGeneralHookFilter(code, 'code') : undefined;
  }
}
