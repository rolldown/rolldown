interface OptionConfig {
  type: 'string' | 'boolean'
  short?: string
  hint?: string
  description: string
}

export const CLI_OPTIONS: Record<string, OptionConfig> = {
  // generates an option usage:
  // -c, --config <filename>     Use this config file...
  config: {
    type: 'string',
    short: 'c',
    hint: 'filename',
    description:
      'Use this config file (if argument is used but value is unspecified, defaults to `rolldown.config.js`)',
  },
  help: {
    type: 'boolean',
    short: 'h',
    description: 'Show this help message',
  },
  // TODO: Auto-Generate CLI Args from Options Schema
}
