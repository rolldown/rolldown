import * as v from 'valibot'

export type LogLevel = 'info' | 'debug' | 'warn'
export type LogLevelOption = LogLevel | 'silent'
export type LogLevelWithError = LogLevel | 'error'

export type RollupLog = any
export type RollupLogWithString = RollupLog | string

export const LogLevelSchema: v.UnionSchema<
  [
    v.LiteralSchema<'debug', undefined>,
    v.LiteralSchema<'info', undefined>,
    v.LiteralSchema<'warn', undefined>,
  ],
  undefined
> = v.union([v.literal('debug'), v.literal('info'), v.literal('warn')])

export const LogLevelOptionSchema: v.UnionSchema<
  [typeof LogLevelSchema, v.LiteralSchema<'silent', undefined>],
  undefined
> = v.union([LogLevelSchema, v.literal('silent')])

export const LogLevelWithErrorSchema: v.UnionSchema<
  [typeof LogLevelSchema, v.LiteralSchema<'error', undefined>],
  undefined
> = v.union([LogLevelSchema, v.literal('error')])

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
export const RollupLogSchema: v.AnySchema = v.any()
export const RollupLogWithStringSchema: v.UnionSchema<
  [v.AnySchema, v.StringSchema<undefined>],
  undefined
> = v.union([RollupLogSchema, v.string()])
