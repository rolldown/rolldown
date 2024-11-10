import type { MaybeArray } from '../types/utils'
import type { StringOrRegExp } from '../types/utils'
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
  /**
   * This filter is used to do a pre-test to determine whether the hook should be called.
   * @example
   * // Filter out all `id`s that contain `node_modules` in the path.
   * ```js
   * { id: 'node_modules' }
   * ```
   * @example
   * // Filter out all `id`s that contain `node_modules` or `src` in the path.
   * ```js
   * { id: ['node_modules', 'src'] }
   * ```
   * @example
   * // Filter out all `id`s that start with `http`
   * ```js
   * { id: /^http/ }
   * ```
   * @example
   * // Exclude all `id`s that contain `node_modules` in the path.
   * ```js
   * { id: { exclude: 'node_modules' } }
   * ```
   * @example
   * // Formal pattern
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
