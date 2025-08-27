import { BuiltinPlugin, createBuiltinPlugin } from './utils';

type AliasPluginAlias = {
  find: string | RegExp;
  replacement: string;
};

// A temp config type for giving better user experience
type AliasPluginConfig = {
  entries: AliasPluginAlias[];
};

export function aliasPlugin(config: AliasPluginConfig): BuiltinPlugin {
  return createBuiltinPlugin('builtin:alias', config);
}
