import type { InputOptions } from '../options/input-options'
import type { OutputOptions } from '../options/output-options'

export interface RolldownOptions extends InputOptions {
  // This is included for compatibility with config files but ignored by `rolldown.rolldown`
  output?: OutputOptions
}
