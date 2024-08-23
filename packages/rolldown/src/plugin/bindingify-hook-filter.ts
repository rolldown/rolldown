import { BindingGeneralHookFilter, BindingTransformHookFilter } from "../binding.d";
import { hookFilterExtension } from ".";
import { normalizedStringOrRegex } from "../options/utils";

export function bindingifyResolveIdFilter(
  filterOption?: hookFilterExtension<'resolveId'>['filter'],
): BindingGeneralHookFilter | undefined {
  if (!filterOption) {
    return undefined;
  }
  const {id}= filterOption;
  if (!id) {
    return undefined;
  }
  let include;
  let exclude;
  if (id.include) {
    include = normalizedStringOrRegex(id.include)
  }
  if (id.exclude) {
    exclude = normalizedStringOrRegex(id.exclude)
  }
  return {
    include,exclude
  }
}



export function bindingifyLoadFilter(
  filterOption?: hookFilterExtension<'load'>['filter'],
):BindingGeneralHookFilter | undefined {
  if (!filterOption) {
    return undefined;
  }
  const {id}= filterOption;
  if (!id) {
    return undefined;
  }
  let include;
  let exclude;
  if (id.include) {
    include = normalizedStringOrRegex(id.include)
  }
  if (id.exclude) {
    exclude = normalizedStringOrRegex(id.exclude)
  }
  let ret = {
    include,exclude
  }
  console.log(`ret: `, ret)
  return ret;
}

