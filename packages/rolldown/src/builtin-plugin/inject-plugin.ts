import type { BindingInjectPluginConfig } from '../binding.cjs';
import type { StringOrRegExp } from '../types/utils';
import { bindingifyInject } from '../utils/bindingify-inject';
import { normalizedStringOrRegex } from '../utils/normalize-string-or-regex';
import { BuiltinPlugin, makeBuiltinPluginCallable } from './utils';

export interface InjectPluginOptions {
  /**
   * Only inject into modules whose id matches these patterns.
   */
  include?: StringOrRegExp | StringOrRegExp[];
  /**
   * Never inject into modules whose id matches these patterns, even if they match {@linkcode include}.
   */
  exclude?: StringOrRegExp | StringOrRegExp[];
}

/**
 * Injects imports for global variables a module uses, like `@rollup/plugin-inject`.
 *
 * `values` takes the same shape as the `transform.inject` option.
 *
 * @example
 * **Skip dependencies**
 * ```js
 * injectPlugin(
 *   { useState: ['#app', 'useState'] },
 *   { exclude: /node_modules/ },
 * )
 * ```
 *
 * @see https://rolldown.rs/builtin-plugins/inject
 * @category Builtin Plugins
 */
export function injectPlugin(
  values: Record<string, string | [string, string]>,
  options: InjectPluginOptions = {},
): BuiltinPlugin {
  const config: BindingInjectPluginConfig = {
    values: bindingifyInject(values) ?? [],
    include: normalizedStringOrRegex(options.include),
    exclude: normalizedStringOrRegex(options.exclude),
  };
  return makeBuiltinPluginCallable(new BuiltinPlugin('builtin:inject', config));
}
