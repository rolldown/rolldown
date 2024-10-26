type AliasPluginAlias = {
  find: string | RegExp
  replacement: string
}

// A temp config type for giving better user experience
export type AliasPluginConfig = {
  entries: AliasPluginAlias[]
}
