import type { BindingAssetPluginConfig } from '../binding';
import { BuiltinPlugin } from './utils';

export function assetPlugin(config: BindingAssetPluginConfig): BuiltinPlugin {
  return new BuiltinPlugin('builtin:asset', {
    config,
  });
}
