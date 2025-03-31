import type { MaybeArray } from '../types/utils'
import type { StringOrRegExp } from '../types/utils'
import type { ModuleType } from '../index'

export type StringFilter<Value = StringOrRegExp> =
  | MaybeArray<Value>
  | {
      include?: MaybeArray<Value>
      exclude?: MaybeArray<Value>
    }

interface FormalModuleTypeFilter {
  include?: ModuleType[]
}

export type ModuleTypeFilter = ModuleType[] | FormalModuleTypeFilter

export interface HookFilter {
  /**
   * This filter is used to do a pre-test to determine whether the hook should be called.
   * 
   * @example
   * Include all `id`s that contain `node_modules` in the path.
   * ```js
   * { id: 'node_modules' }
   * ```
   * @example
   * Include all `id`s that contain `node_modules` or `src` in the path.
   * ```js
   * { id: ['node_modules', 'src'] }
   * ```
   * @example
   * Include all `id`s that start with `http`
   * ```js
   * { id: /^http/ }
   * ```
   * @example
   * Exclude all `id`s that contain `node_modules` in the path.
   * ```js
   * { id: { exclude: 'node_modules' } }
   * ```
   * @example
   * Formal pattern to define includes and excludes.
   * ```
   * { id : {
   *   include: ["foo", /bar/],
   *   exclude: ["baz", /qux/]
   * }}
   * ```
   */
  id?: StringFilter
  moduleType?: ModuleTypeFilter
  code?: StringFilter
}
