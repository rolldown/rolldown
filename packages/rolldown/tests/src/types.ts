import type { RolldownOptions, RolldownOutput } from 'rolldown'

export interface TestConfig {
  skip?: boolean
  skipComposingJsPlugin?: boolean
  config?: RolldownOptions
  beforeTest?: () => Promise<void> | void
  afterTest?: (output: RolldownOutput) => Promise<void> | void
}
