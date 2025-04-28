import type {
  BindingFilterToken,
  BindingGeneralHookFilter,
  BindingRenderChunkHookFilter,
  BindingTransformHookFilter,
} from '../binding.d';
import type { FilterExpression } from '../filter-expression-index';
import { arraify } from '../utils/misc';
import type { HookFilterExtension, ModuleType } from '.';
import type { GeneralHookFilter } from './hook-filter';

export function bindingifyGeneralHookFilter(
  matcher: GeneralHookFilter,
): BindingGeneralHookFilter {
  if (typeof matcher === 'string' || matcher instanceof RegExp) {
    return { include: [matcher] };
  }
  if (Array.isArray(matcher)) {
    return { include: matcher };
  }
  let custom: BindingFilterToken[][] = [];
  if (matcher.custom) {
    custom = matcher.custom.map(bindingifyFilterExpr);
  }
  return {
    include: matcher.include ? arraify(matcher.include) : undefined,
    exclude: matcher.exclude ? arraify(matcher.exclude) : undefined,
    custom: custom.length > 0 ? custom : undefined,
  };
}

function bindingifyFilterExpr(
  expr: FilterExpression,
): BindingFilterToken[] {
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
): BindingGeneralHookFilter | undefined {
  return filterOption?.id
    ? bindingifyGeneralHookFilter(filterOption.id)
    : undefined;
}

export function bindingifyLoadFilter(
  filterOption?: HookFilterExtension<'load'>['filter'],
): BindingGeneralHookFilter | undefined {
  return filterOption?.id
    ? bindingifyGeneralHookFilter(filterOption.id)
    : undefined;
}

export function bindingifyTransformFilter(
  filterOption?: HookFilterExtension<'transform'>['filter'],
): BindingTransformHookFilter | undefined {
  if (!filterOption) {
    return undefined;
  }
  const { id, code, moduleType, custom } = filterOption;

  let moduleTypeRet: ModuleType[] | undefined;
  if (moduleType) {
    if (Array.isArray(moduleType)) {
      moduleTypeRet = moduleType;
    } else {
      moduleTypeRet = moduleType.include;
    }
  }
  let ret: BindingFilterToken[][] = [];
  if (custom) {
    ret = custom.map(bindingifyFilterExpr);
  }
  return {
    id: id ? bindingifyGeneralHookFilter(id) : undefined,
    code: code ? bindingifyGeneralHookFilter(code) : undefined,
    moduleType: moduleTypeRet,
    custom: ret.length > 0 ? ret : undefined,
  };
}

export function bindingifyRenderChunkFilter(
  filterOption?: HookFilterExtension<'renderChunk'>['filter'],
): BindingRenderChunkHookFilter | undefined {
  if (filterOption) {
    const { code } = filterOption;

    return {
      code: code ? bindingifyGeneralHookFilter(code) : undefined,
    };
  }
}
