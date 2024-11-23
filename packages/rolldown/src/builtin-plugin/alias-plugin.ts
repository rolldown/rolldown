import { BuiltinPlugin } from './constructors'

type AliasPluginAlias = {
  find: string | RegExp
  replacement: string
}

// A temp config type for giving better user experience
type AliasPluginConfig = {
  entries: AliasPluginAlias[]
}

export function aliasPlugin(config: AliasPluginConfig) {
  return new BuiltinPlugin('builtin:alias', config)
}
