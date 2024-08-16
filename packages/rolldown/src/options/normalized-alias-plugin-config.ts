import { isRegExp } from 'node:util/types'
import { BindingAliasPluginConfig } from '../binding'

type AliasPluginAlias = {
  find: string | RegExp
  replacement: string
}

// A temp config type for giving better user experience
export type AliasPluginConfig = {
  entries: AliasPluginAlias[]
}

export function normalizeAliasPluginConfig(
  config?: AliasPluginConfig,
): BindingAliasPluginConfig | undefined {
  if (!config) {
    return undefined
  }
  let entries = config.entries.map((entry) => {
    if (isRegExp(entry.find)) {
      return {
        find: { value: entry.find.source, flag: entry.find.flags },
        replacement: entry.replacement,
      }
    } else {
      return {
        find: { value: entry.find },
        replacement: entry.replacement,
      }
    }
  })

  return {
    entries,
  }
}
