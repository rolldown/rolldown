import { InputOptions } from '../types/input-options'
import { OutputOptions } from '../types/output-options'

export interface WatchOptions extends InputOptions {
  output?: OutputOptions
}
