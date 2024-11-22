import type { LogHandler } from '../rollup'
import type { InputOptions } from '../types/input-options'

export interface NormalizedInputOptions extends InputOptions {
  onLog: LogHandler
}
