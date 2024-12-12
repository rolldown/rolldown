import { type ConsolaInstance, createConsola } from 'consola'

/**
 * Console logger
 */
export const logger: Record<string, any> | ConsolaInstance = process.env
  .ROLLDOWN_TEST
  ? createTestingLogger()
  : createConsola({
      formatOptions: {
        date: false,
      },
    })

function createTestingLogger() {
  const types = [
    'silent',
    'fatal',
    'error',
    'warn',
    'log',
    'info',
    'success',
    'fail',
    'ready',
    'start',
    'box',
    'debug',
    'trace',
    'verbose',
  ]
  const ret: Record<string, any> = Object.create(null)
  for (const type of types) {
    ret[type] = console.log
  }
  return ret
}
