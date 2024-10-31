import type { StringOrRegExp } from '../constants/types'
import type { ModuleType } from '../index'

interface StringFilter {
  include?: StringOrRegExp[]
  exclude?: StringOrRegExp[]
}

interface FormalModuleTypeFilter {
  include?: ModuleType[]
}

type ModuleTypeFilter = ModuleType[] | FormalModuleTypeFilter

export interface HookFilter {
  id?: StringFilter
  moduleType?: ModuleTypeFilter
  code?: StringFilter
}
