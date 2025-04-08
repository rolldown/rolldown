import { BuiltinPlugin } from './constructors';

import { BindingTransformPluginConfig } from '../binding';
import { normalizedStringOrRegex } from '../utils/normalize-string-or-regex';

type TransformPattern = string | RegExp | (RegExp | string)[];
// A temp config type for giving better user experience
export type TransformPluginConfig =
  & Omit<
    BindingTransformPluginConfig,
    'include' | 'exclude'
  >
  & {
    include?: TransformPattern;
    exclude?: TransformPattern;
  };

function normalizeEcmaTransformPluginConfig(
  config?: TransformPluginConfig,
): BindingTransformPluginConfig | undefined {
  if (!config) {
    return undefined;
  }
  let normalizedConfig: BindingTransformPluginConfig = {
    ...config,
    exclude: normalizedStringOrRegex(config.exclude),
    include: normalizedStringOrRegex(config.include),
  };

  return normalizedConfig;
}

export function transformPlugin(config?: TransformPluginConfig): BuiltinPlugin {
  return new BuiltinPlugin(
    'builtin:transform',
    normalizeEcmaTransformPluginConfig(config),
  );
}
