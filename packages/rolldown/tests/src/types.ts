import type { RolldownOptions, RolldownOutput } from 'rolldown'

export interface TestConfig {
  skip?: boolean
  config?: RolldownOptions
  afterTest?: (output: RolldownOutput) => void
}
