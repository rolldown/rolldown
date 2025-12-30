import type { BindingViteAssetPluginConfig } from '../../dist/binding.cjs';
import { BuiltinPlugin } from './utils';

export function viteAssetPlugin(config: BindingViteAssetPluginConfig): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-asset', config);
}
