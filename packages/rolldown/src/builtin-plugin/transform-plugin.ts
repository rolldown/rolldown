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

function normalizeEcmaTransformPluginConfig(
  config?: TransformPluginConfig,
): BindingTransformPluginConfig | undefined {
  if (config) {
    return {
      ...config,
      include: normalizedStringOrRegex(config.include),
      exclude: normalizedStringOrRegex(config.exclude),
      jsxRefreshInclude: normalizedStringOrRegex(config.jsxRefreshInclude),
      jsxRefreshExclude: normalizedStringOrRegex(config.jsxRefreshExclude),
    };
  }
}

export function transformPlugin(config?: TransformPluginConfig): BuiltinPlugin {
  return new BuiltinPlugin(
    'builtin:transform',
    normalizeEcmaTransformPluginConfig(config),
  );
}
