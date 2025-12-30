import type { BindingViteCssPostPluginConfig } from '../../dist/binding.cjs';
import type { NormalizedOutputOptions } from '../options/normalized-output-options';
import { BuiltinPlugin } from './utils';

export type ViteCssPostPluginConfig = Omit<
  BindingViteCssPostPluginConfig,
  'cssScopeTo' | 'isLegacy'
> & {
  isOutputOptionsForLegacyChunks?: (outputOptions: NormalizedOutputOptions) => boolean;
};

export function viteCSSPostPlugin(config: ViteCssPostPluginConfig): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-css-post', config);
}
