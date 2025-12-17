import { BuiltinPlugin } from './utils';

import type { BindingViteTransformPluginConfig } from '../binding.cjs';
import { normalizedStringOrRegex } from '../utils/normalize-string-or-regex';

type TransformPattern = string | RegExp | readonly (RegExp | string)[];

// A temp config type for giving better user experience
type TransformPluginConfig =
  & Omit<
    BindingViteTransformPluginConfig,
    | 'include'
    | 'exclude'
    | 'jsxRefreshInclude'
    | 'jsxRefreshExclude'
    | 'yarnPnp'
  >
  & {
    include?: TransformPattern;
    exclude?: TransformPattern;
    jsxRefreshInclude?: TransformPattern;
    jsxRefreshExclude?: TransformPattern;
  };

export function viteTransformPlugin(
  config: TransformPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-transform', {
    ...config,
    include: normalizedStringOrRegex(config.include),
    exclude: normalizedStringOrRegex(config.exclude),
    jsxRefreshInclude: normalizedStringOrRegex(config.jsxRefreshInclude),
    jsxRefreshExclude: normalizedStringOrRegex(config.jsxRefreshExclude),
    // process is undefined for browser build
    yarnPnp: typeof process === 'object' && !!process.versions?.pnp,
  });
}
