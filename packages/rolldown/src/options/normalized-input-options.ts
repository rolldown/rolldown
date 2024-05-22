import type {
  RollupLog,
  NormalizedInputOptions as RollupNormalizedInputOptions,
} from '../rollup'
import type { InputOptions } from './input-options'
import { Plugin, ParallelPlugin } from '../plugin'
import { LogLevel } from '../log/logging'

export interface NormalizedInputOptions extends InputOptions {
  input: RollupNormalizedInputOptions['input']
  plugins: (Plugin | ParallelPlugin)[]
  onLog: (level: LogLevel, log: RollupLog) => void
  logLevel: LogLevel
}
