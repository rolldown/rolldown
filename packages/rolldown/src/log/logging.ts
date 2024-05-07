import { z } from 'zod'

const LogLevelSchema = z
  .literal('info')
  .or(z.literal('debug'))
  .or(z.literal('warn'))

export type LogLevel = z.infer<typeof LogLevelSchema>

export const LogLevelOptionSchema = LogLevelSchema.or(z.literal('silent'))

export type LogLevelOption = z.infer<typeof LogLevelOptionSchema>

export const LOG_LEVEL_SILENT: LogLevelOption = 'silent'
// export const LOG_LEVEL_ERROR = 'error'
export const LOG_LEVEL_WARN: LogLevel = 'warn'
export const LOG_LEVEL_INFO: LogLevel = 'info'
export const LOG_LEVEL_DEBUG: LogLevel = 'debug'

export const logLevelPriority: Record<LogLevelOption, number> = {
  [LOG_LEVEL_DEBUG]: 0,
  [LOG_LEVEL_INFO]: 1,
  [LOG_LEVEL_WARN]: 2,
  [LOG_LEVEL_SILENT]: 3,
}
