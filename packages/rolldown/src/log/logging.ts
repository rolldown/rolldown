import { z } from 'zod'

export type LogLevel = 'info' | 'debug' | 'warn'
export type LogLevelOption = LogLevel | 'silent'
export type LogLevelWithError = LogLevel | 'error'

export type RollupLog = any
export type RollupLogWithString = RollupLog | string

export const LogLevelSchema = z
  .literal('info')
  .or(z.literal('debug'))
  .or(z.literal('warn')) satisfies z.ZodType<LogLevel>

export const LogLevelOptionSchema = LogLevelSchema.or(z.literal('silent'))
export const LogLevelWithErrorSchema = LogLevelSchema.or(z.literal('error'))

export const LOG_LEVEL_SILENT: LogLevelOption = 'silent'
export const LOG_LEVEL_ERROR = 'error'
export const LOG_LEVEL_WARN: LogLevel = 'warn'
export const LOG_LEVEL_INFO: LogLevel = 'info'
export const LOG_LEVEL_DEBUG: LogLevel = 'debug'

export const logLevelPriority: Record<LogLevelOption, number> = {
  [LOG_LEVEL_DEBUG]: 0,
  [LOG_LEVEL_INFO]: 1,
  [LOG_LEVEL_WARN]: 2,
  [LOG_LEVEL_SILENT]: 3,
}

// TODO RollupLog Fields
export const RollupLogSchema = z.any() satisfies z.ZodType<RollupLog>
export const RollupLogWithStringSchema = RollupLogSchema.or(
  z.string(),
) satisfies z.ZodType<RollupLogWithString>
