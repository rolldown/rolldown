import type {
  BindingGeneralHookFilter,
  BindingTransformHookFilter,
} from '../binding.d';
import { arraify } from '../utils/misc';
import type { HookFilterExtension, ModuleType } from '.';
import type { StringFilter } from './hook-filter';

export function bindingifyStringFilter(
  matcher: StringFilter,
): BindingGeneralHookFilter {
  if (typeof matcher === 'string' || matcher instanceof RegExp) {
    return { include: [matcher] };
  }
  if (Array.isArray(matcher)) {
    return { include: matcher };
  }

  return {
    include: matcher.include ? arraify(matcher.include) : undefined,
    exclude: matcher.exclude ? arraify(matcher.exclude) : undefined,
  };
}

export function bindingifyResolveIdFilter(
  filterOption?: HookFilterExtension<'resolveId'>['filter'],
): BindingGeneralHookFilter | undefined {
  return filterOption?.id ? bindingifyStringFilter(filterOption.id) : undefined;
}

export function bindingifyLoadFilter(
  filterOption?: HookFilterExtension<'load'>['filter'],
): BindingGeneralHookFilter | undefined {
  return filterOption?.id ? bindingifyStringFilter(filterOption.id) : undefined;
}

export function bindingifyTransformFilter(
  filterOption?: HookFilterExtension<'transform'>['filter'],
): BindingTransformHookFilter | undefined {
  if (!filterOption) {
    return undefined;
  }
  const { id, code, moduleType } = filterOption;

  let moduleTypeRet: ModuleType[] | undefined;
  if (moduleType) {
    if (Array.isArray(moduleType)) {
      moduleTypeRet = moduleType;
    } else {
      moduleTypeRet = moduleType.include;
    }
  }

  return {
    id: id ? bindingifyStringFilter(id) : undefined,
    code: code ? bindingifyStringFilter(code) : undefined,
    moduleType: moduleTypeRet,
  };
}
