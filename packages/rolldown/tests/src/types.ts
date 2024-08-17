import type { RolldownOptions, RolldownOutput } from 'rolldown'

export type TestKind = 'default' | 'compose-js-plugin'
export interface TestConfig {
  skip?: boolean
  skipComposingJsPlugin?: boolean
  config?: RolldownOptions
  beforeTest?: () => Promise<void> | void
  afterTest?: (output: RolldownOutput) => Promise<void> | void
  catchError?: (err: unknown) => Promise<void> | void
}
