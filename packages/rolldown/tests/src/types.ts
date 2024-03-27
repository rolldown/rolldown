import type { RolldownOptions, RolldownOutput } from 'rolldown'

export interface TestConfig {
  config?: RolldownOptions
  afterTest?: (output: RolldownOutput) => void
}
