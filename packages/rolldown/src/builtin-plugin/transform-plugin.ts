import { BuiltinPlugin } from './constructors';

import { BindingTransformPluginConfig } from '../binding';
import { normalizedStringOrRegex } from '../utils/normalize-string-or-regex';

type TransformPattern = string | RegExp | (RegExp | string)[];

// A temp config type for giving better user experience
export type TransformPluginConfig =
  & Omit<
    BindingTransformPluginConfig,
    'include' | 'exclude' | 'jsxRefreshInclude' | 'jsxRefreshExclude'
  >
  & {
    include?: TransformPattern;
    exclude?: TransformPattern;
    jsxRefreshInclude?: TransformPattern;
    jsxRefreshExclude?: TransformPattern;
  };

export function transformPlugin(config?: TransformPluginConfig): BuiltinPlugin {
  if (config) {
    config = {
      ...config,
      include: normalizedStringOrRegex(config.include),
      exclude: normalizedStringOrRegex(config.exclude),
      jsxRefreshInclude: normalizedStringOrRegex(config.jsxRefreshInclude),
      jsxRefreshExclude: normalizedStringOrRegex(config.jsxRefreshExclude),
    };
  }
  return new BuiltinPlugin('builtin:transform', config);
}
