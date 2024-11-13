import type { InputOptions } from '../types/input-options'
import type { OutputOptions } from '../types/output-options'

export interface RolldownOptions extends InputOptions {
  // This is included for compatibility with config files but ignored by `rolldown.rolldown`
  output?: OutputOptions
}
