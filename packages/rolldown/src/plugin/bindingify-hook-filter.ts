import {
  BindingGeneralHookFilter,
  BindingTransformHookFilter,
} from '../binding.d'
import { hookFilterExtension, ModuleType } from '.'
import { normalizedStringOrRegex } from '../options/utils'

export function bindingifyResolveIdFilter(
  filterOption?: hookFilterExtension<'resolveId'>['filter'],
): BindingGeneralHookFilter | undefined {
  if (!filterOption) {
    return undefined
  }
  const { id } = filterOption
  if (!id) {
    return undefined
  }
  let include
  let exclude
  if (id.include) {
    include = normalizedStringOrRegex(id.include)
  }
  if (id.exclude) {
    exclude = normalizedStringOrRegex(id.exclude)
  }
  return {
    include,
    exclude,
  }
}

export function bindingifyLoadFilter(
  filterOption?: hookFilterExtension<'load'>['filter'],
): BindingGeneralHookFilter | undefined {
  if (!filterOption) {
    return undefined
  }
  const { id } = filterOption
  if (!id) {
    return undefined
  }
  let include
  let exclude
  if (id.include) {
    include = normalizedStringOrRegex(id.include)
  }
  if (id.exclude) {
    exclude = normalizedStringOrRegex(id.exclude)
  }
  let ret = {
    include,
    exclude,
  }
  return ret
}

export function bindingifyTransformFilter(
  filterOption?: hookFilterExtension<'transform'>['filter'],
): BindingTransformHookFilter | undefined {
  if (!filterOption) {
    return undefined
  }
  const { id, moduleType, code } = filterOption
  let idRet
  let moduleTypeRet: ModuleType[] | undefined
  let codeRet
  if (id) {
    let include
    let exclude
    if (id.include) {
      include = normalizedStringOrRegex(id.include)
    }
    if (id.exclude) {
      exclude = normalizedStringOrRegex(id.exclude)
    }
    idRet = {
      include,
      exclude,
    }
  }
  if (code) {
    let include
    let exclude
    if (code.include) {
      include = normalizedStringOrRegex(code.include)
    }
    if (code.exclude) {
      exclude = normalizedStringOrRegex(code.exclude)
    }
    codeRet = {
      include,
      exclude,
    }
  }
  if (moduleType) {
    if (Array.isArray(moduleType)) {
      moduleTypeRet = moduleType
    } else {
      moduleTypeRet = moduleType.include
    }
  }

  return {
    id: idRet,
    moduleType: moduleTypeRet,
    code: codeRet,
  }
}
