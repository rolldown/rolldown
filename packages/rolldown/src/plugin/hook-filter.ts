import type { MaybeArray } from '../types/utils'
import type { StringOrRegExp } from '../constants/types'
import type { ModuleType } from '../index'

export type StringFilter =
  | MaybeArray<StringOrRegExp>
  | {
      include?: MaybeArray<StringOrRegExp>
      exclude?: MaybeArray<StringOrRegExp>
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
