import type { BindingViteManifestPluginConfig } from '../binding.cjs';
import type { NormalizedOutputOptions } from '../options/normalized-output-options';
import { BuiltinPlugin } from './utils';

export type ViteManifestPluginConfig =
  & Omit<BindingViteManifestPluginConfig, 'isLegacy'>
  & {
    isOutputOptionsForLegacyChunks?: (
      outputOptions: NormalizedOutputOptions,
    ) => boolean;
  };

export function viteManifestPlugin(
  config: ViteManifestPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-manifest', config);
}
