import { BuiltinPlugin } from './utils';

type ViteAliasPluginConfig = {
  entries: {
    find: string | RegExp;
    replacement: string;
  }[];
};

export function viteAliasPlugin(config: ViteAliasPluginConfig): BuiltinPlugin {
  return new BuiltinPlugin('builtin:vite-alias', config);
}
