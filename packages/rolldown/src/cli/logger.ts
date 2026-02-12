import { type ConsolaInstance, createConsola } from 'consola';

/**
 * Console logger
 */
export const logger: Record<string, any> | ConsolaInstance = process.env.ROLLDOWN_TEST
  ? createTestingLogger()
  : createConsola({
      formatOptions: {
        date: false,
      },
    });

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
  ];
  const ret: Record<string, any> = Object.create(null);
  for (const type of types) {
    // Use a wrapper function to allow spying in tests
    // oxlint-disable-next-line no-console
    ret[type] = (...args: unknown[]) => console.log(...args);
  }
  return ret;
}
