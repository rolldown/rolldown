import {
  BindingGeneralHookFilter,
  BindingTransformHookFilter,
} from '../binding.d'
import { hookFilterExtension, ModuleType } from '.'

export function bindingifyResolveIdFilter(
  filterOption?: hookFilterExtension<'resolveId'>['filter'],
): BindingGeneralHookFilter | undefined {
  return filterOption?.id
}

export function bindingifyLoadFilter(
  filterOption?: hookFilterExtension<'load'>['filter'],
): BindingGeneralHookFilter | undefined {
  return filterOption?.id
}

export function bindingifyTransformFilter(
  filterOption?: hookFilterExtension<'transform'>['filter'],
): BindingTransformHookFilter | undefined {
  if (!filterOption) {
    return undefined
  }
  const { id, code, moduleType } = filterOption

  let moduleTypeRet: ModuleType[] | undefined
  if (moduleType) {
    if (Array.isArray(moduleType)) {
      moduleTypeRet = moduleType
    } else {
      moduleTypeRet = moduleType.include
    }
  }

  return {
    id,
    code,
    moduleType: moduleTypeRet,
  }
}
