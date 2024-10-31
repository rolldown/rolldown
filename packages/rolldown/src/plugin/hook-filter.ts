import type { StringOrRegExp } from '../constants/types'
import type { ModuleType } from '../index'

export type BaseHookFilter = {
  id?: {
    include?: StringOrRegExp[]
    exclude?: StringOrRegExp[]
  }
  moduleType?:
    | ModuleType[]
    | {
        include?: ModuleType[]
      }
  code?: {
    include?: StringOrRegExp[]
    exclude?: StringOrRegExp[]
  }
}
