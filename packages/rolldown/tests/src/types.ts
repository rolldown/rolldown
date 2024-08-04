import type { RolldownOptions, RolldownOutput } from 'rolldown'

export interface TestConfig {
  skip?: boolean
  skipComposingJsPlugin?: boolean
  config?: RolldownOptions
  afterTest?: (output: RolldownOutput) => Promise<void> | void
}
