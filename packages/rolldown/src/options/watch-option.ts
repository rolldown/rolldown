import { InputOptions } from './input-options'
import { OutputOptions } from './output-options'

export interface WatchOptions extends InputOptions {
  output?: OutputOptions
}
