import type { RolldownOptions, RolldownOutput } from 'rolldown'

export interface TestConfig {
  skip?: boolean
  skipComposingJsPlugin?: boolean
  config?: RolldownOptions
  onerror?: (err: unknown) => void
  afterTest?: (output: RolldownOutput) => Promise<void> | void
}
