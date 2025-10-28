import type { BindingAssetPluginConfig } from '../binding.cjs';
import { BuiltinPlugin } from './utils';

export function assetPlugin(config: BindingAssetPluginConfig): BuiltinPlugin {
  return new BuiltinPlugin('builtin:asset', config);
}
