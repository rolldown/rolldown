import { RolldownOptions } from 'rolldown'
import { RolldownOutput } from 'src'

export interface TestConfig {
  config?: RolldownOptions
  afterTest?: (output: RolldownOutput) => void
}
