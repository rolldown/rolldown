import type {
  LogLevelOption,
  RollupLog,
  NormalizedInputOptions as RollupNormalizedInputOptions,
} from '../rollup'
import type { InputOptions } from './input-options'
import type { RolldownPlugin } from '../plugin'
import type { LogLevel } from '../log/logging'
import { NormalizedTreeshakingOptions } from '../treeshake'

export interface NormalizedInputOptions
  extends Omit<InputOptions, 'treeshake'> {
  input: RollupNormalizedInputOptions['input']
  plugins: RolldownPlugin[]
  onLog: (level: LogLevel, log: RollupLog) => void
  logLevel: LogLevelOption
  // After normalized, `false` will be converted to `undefined`, otherwise, default value will be assigned
  // Because it is hard to represent Enum in napi, ref: https://github.com/napi-rs/napi-rs/issues/507
  // So we use `undefined | NormalizedTreeshakingOptions` (or Option<NormalizedTreeshakingOptions> in rust side), to represent `false | NormalizedTreeshakingOptions`
  treeshake?: NormalizedTreeshakingOptions
}
