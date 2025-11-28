import type { BindingViteCssPostPluginConfig } from '../binding.cjs';
import { BuiltinPlugin } from './utils';

export type ViteCssPostPluginConfig = Omit<
  BindingViteCssPostPluginConfig,
  'cssScopeTo'
>;

export function viteCSSPostPlugin(
  config?: ViteCssPostPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-css-post', config);
}
