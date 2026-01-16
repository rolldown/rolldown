import type { ModuleType } from '../index';
import type { MaybeArray } from '../types/utils';
import type { StringOrRegExp } from '../types/utils';

/** @category Plugin APIs */
export type GeneralHookFilter<Value = StringOrRegExp> =
  | MaybeArray<Value>
  | {
      include?: MaybeArray<Value>;
      exclude?: MaybeArray<Value>;
    };

interface FormalModuleTypeFilter {
  include?: ModuleType[];
}

/** @category Plugin APIs */
export type ModuleTypeFilter = ModuleType[] | FormalModuleTypeFilter;

/**
 * A filter to be used to do a pre-test to determine whether the hook should be called.
 * @category Plugin APIs
 */
export interface HookFilter {
  /**
   * A filter based on the module `id`.
   *
   * If the value is a string, it is treated as a glob pattern.
   * The string type is not available for {@linkcode Plugin.resolveId | resolveId} hook.
   *
   * @example
   * Include all `id`s that contain `node_modules` in the path.
   * ```js
   * { id: '**'+'/node_modules/**' }
   * ```
   * @example
   * Include all `id`s that contain `node_modules` or `src` in the path.
   * ```js
   * { id: ['**'+'/node_modules/**', '**'+'/src/**'] }
   * ```
   * @example
   * Include all `id`s that start with `http`
   * ```js
   * { id: /^http/ }
   * ```
   * @example
   * Exclude all `id`s that contain `node_modules` in the path.
   * ```js
   * { id: { exclude: '**'+'/node_modules/**' } }
   * ```
   * @example
   * Formal pattern to define includes and excludes.
   * ```js
   * { id : {
   *   include: ['**'+'/foo/**', /bar/],
   *   exclude: ['**'+'/baz/**', /qux/]
   * }}
   * ```
   */
  id?: GeneralHookFilter;
  /**
   * A filter based on the module's `moduleType`.
   */
  moduleType?: ModuleTypeFilter;
  /**
   * A filter based on the module's code.
   *
   * Only available for {@linkcode Plugin.transform | transform} hook.
   */
  code?: GeneralHookFilter;
}
