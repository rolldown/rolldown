import type { BindingAssetPluginConfig } from '../binding';
import type { StringOrRegExp } from '../types/utils';
import { normalizedStringOrRegex } from '../utils/normalize-string-or-regex';
import { BuiltinPlugin } from './utils';

type AssetPluginConfig = Omit<BindingAssetPluginConfig, 'assetsInclude'> & {
  assetsInclude: StringOrRegExp;
};

export function assetPlugin(config: AssetPluginConfig): BuiltinPlugin {
  return new BuiltinPlugin('builtin:asset', {
    ...config,
    assetsInclude: normalizedStringOrRegex(config.assetsInclude),
  });
}
