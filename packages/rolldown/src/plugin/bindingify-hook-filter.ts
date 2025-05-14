import type {
  FilterExpression,
  TopLevelFilterExpression,
} from '@rolldown/pluginutils';
import * as filter from '@rolldown/pluginutils';
import * as R from 'remeda';
import type { BindingFilterToken, BindingHookFilter } from '../binding.d';
import type { StringOrRegExp } from '../types/utils';
import { arraify } from '../utils/misc';
import type { HookFilterExtension } from '.';
import type { GeneralHookFilter } from './hook-filter';

// Convert `exclude` and `include` to tokens of FilterExpr
// Array of `BindingFilterToken` will be converted to `FilterExpr` finally,
// use `generalHookFilterToFilterExprs` instead of `generalHookFilterToFilterArrayOfArrayBindingFilterToken` would be concise
function generalHookFilterMatcherToFilterExprs<T extends StringOrRegExp>(
  matcher: GeneralHookFilter<T>,
  stringKind: 'code' | 'id',
): filter.TopLevelFilterExpression[] | undefined {
  if (typeof matcher === 'string' || matcher instanceof RegExp) {
    return [filter.include(generateAtomMatcher(stringKind, matcher))];
  }
  if (Array.isArray(matcher)) {
    return matcher.map((m) =>
      filter.include(generateAtomMatcher(stringKind, m))
    );
  }
  let ret: filter.TopLevelFilterExpression[] = [];
  if (matcher.exclude) {
    ret.push(
      ...arraify(matcher.exclude).map((m) =>
        filter.exclude(generateAtomMatcher(stringKind, m))
      ),
    );
  }
  if (matcher.include) {
    ret.push(
      ...arraify(matcher.include).map((m) =>
        filter.include(generateAtomMatcher(stringKind, m))
      ),
    );
  }
  return ret;
}

function generateAtomMatcher(kind: 'code' | 'id', matcher: StringOrRegExp) {
  return kind === 'code' ? filter.code(matcher) : filter.id(matcher);
}

function transformFilterMatcherToFilterExprs(
  filterOption: HookFilterExtension<'transform'>['filter'],
): filter.TopLevelFilterExpression[] | undefined {
  if (!filterOption) {
    return undefined;
  }
  if (Array.isArray(filterOption)) {
    return filterOption;
  }
  const { id, code, moduleType } = filterOption;

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

  let andExprList: FilterExpression[] = [];
  if (moduleType) {
    let moduleTypes = Array.isArray(moduleType)
      ? moduleType
      : moduleType.include ?? [];
    andExprList.push(
      filter.or(...moduleTypes.map((m) => filter.moduleType(m))),
    );
  }
  if (idIncludes.length) {
    andExprList.push(filter.or(...idIncludes.map((item) => item.expr)));
  }

  if (codeIncludes.length) {
    andExprList.push(filter.or(...codeIncludes.map((item) => item.expr)));
  }

  if (andExprList.length) {
    ret.push(filter.include(filter.and(...andExprList)));
  }
  return ret;
}

export function bindingifyGeneralHookFilter<
  T extends StringOrRegExp,
  F extends GeneralHookFilter<T>,
>(stringKind: 'code' | 'id', pattern: F): BindingHookFilter | undefined {
  let filterExprs = generalHookFilterMatcherToFilterExprs(pattern, stringKind);
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

function bindingifyFilterExpr(
  expr: FilterExpression | TopLevelFilterExpression,
): BindingFilterToken[] {
  let list: BindingFilterToken[] = [];
  bindingifyFilterExprImpl(expr, list);
  return list;
}
function bindingifyFilterExprImpl(
  expr: FilterExpression | TopLevelFilterExpression,
  list: BindingFilterToken[],
) {
  switch (expr.kind) {
    case 'and': {
      let args = expr.args;
      for (let i = args.length - 1; i >= 0; i--) {
        bindingifyFilterExprImpl(args[i], list);
      }
      list.push({
        kind: 'And',
        payload: args.length,
      });
      break;
    }
    case 'or': {
      let args = expr.args;
      for (let i = args.length - 1; i >= 0; i--) {
        bindingifyFilterExprImpl(args[i], list);
      }
      list.push({
        kind: 'Or',
        payload: args.length,
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
      list.push({ kind: 'Id', payload: expr.pattern });
      if (expr.params.cleanUrl) {
        list.push({ kind: 'CleanUrl' });
      }
      break;
    }
    case 'moduleType': {
      list.push({ kind: 'ModuleType', payload: expr.pattern });
      break;
    }
    case 'code': {
      list.push({ kind: 'Code', payload: expr.pattern });
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
    case 'query': {
      list.push({ kind: 'QueryKey', payload: expr.key });
      list.push({ kind: 'QueryValue', payload: expr.pattern });
      break;
    }
    default:
      throw new Error(`Unknown filter expression: ${expr}`);
  }
}

export function bindingifyResolveIdFilter(
  filterOption?: HookFilterExtension<'resolveId'>['filter'],
): BindingHookFilter | undefined {
  if (!filterOption) {
    return undefined;
  }
  if (Array.isArray(filterOption)) {
    return {
      value: filterOption.map(bindingifyFilterExpr),
    };
  }
  return filterOption.id
    ? bindingifyGeneralHookFilter('id', filterOption.id)
    : undefined;
}

export function bindingifyLoadFilter(
  filterOption?: HookFilterExtension<'load'>['filter'],
): BindingHookFilter | undefined {
  if (!filterOption) {
    return undefined;
  }
  if (Array.isArray(filterOption)) {
    return {
      value: filterOption.map(bindingifyFilterExpr),
    };
  }
  return filterOption.id
    ? bindingifyGeneralHookFilter('id', filterOption.id)
    : undefined;
}

export function bindingifyTransformFilter(
  filterOption?: HookFilterExtension<'transform'>['filter'],
): BindingHookFilter | undefined {
  if (!filterOption) {
    return undefined;
  }

  let filterExprs = transformFilterMatcherToFilterExprs(filterOption);

  let ret: BindingFilterToken[][] = [];
  if (filterExprs) {
    ret = filterExprs.map(bindingifyFilterExpr);
  }
  return {
    value: ret.length > 0 ? ret : undefined,
  };
}

export function bindingifyRenderChunkFilter(
  filterOption?: HookFilterExtension<'renderChunk'>['filter'],
): BindingHookFilter | undefined {
  if (!filterOption) {
    return undefined;
  }
  if (Array.isArray(filterOption)) {
    return {
      value: filterOption.map(bindingifyFilterExpr),
    };
  }
  return filterOption.code
    ? bindingifyGeneralHookFilter('code', filterOption.code)
    : undefined;
}
