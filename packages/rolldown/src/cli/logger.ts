import { type ConsolaInstance, createConsola } from 'consola';

const SILENT_ALLOW_TYPES = new Set(['fatal', 'error']);
let cliSilent = false;

/**
 * Console logger
 */
export const logger: Record<string, any> | ConsolaInstance = wrapLogger(
  process.env.ROLLDOWN_TEST
    ? createTestingLogger()
    : createConsola({
      formatOptions: {
        date: false,
      },
    }),
);

export function setCliSilent(silent: boolean): void {
  cliSilent = silent;
}

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
    // oxlint-disable-next-line no-console
    ret[type] = console.log;
  }
  return ret;
}

function wrapLogger(logger: any) {
  const types = [
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
  for (const type of types) {
    if (typeof logger[type] !== 'function') continue;
    const original = logger[type].bind(logger);
    logger[type] = (...args: any[]) => {
      if (cliSilent && !SILENT_ALLOW_TYPES.has(type)) {
        return;
      }
      return original(...args);
    };
  }
  return logger;
}
