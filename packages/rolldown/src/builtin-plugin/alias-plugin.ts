import { BindingBuiltinPluginName } from '../binding'
import { BuiltinPlugin } from './constructors'

type AliasPluginAlias = {
  find: string | RegExp
  replacement: string
}

// A temp config type for giving better user experience
type AliasPluginConfig = {
  entries: AliasPluginAlias[]
}

class AliasPlugin extends BuiltinPlugin {
  constructor(config?: AliasPluginConfig) {
    super(BindingBuiltinPluginName.Alias, config)
  }
}

export function aliasPlugin(config: AliasPluginConfig) {
  return new AliasPlugin(config)
}
