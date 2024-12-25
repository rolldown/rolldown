import { InputOptions } from '../options/input-options'
import { OutputOptions } from '../options/output-options'

export interface WatchOptions extends InputOptions {
  output?: OutputOptions | OutputOptions[]
}
