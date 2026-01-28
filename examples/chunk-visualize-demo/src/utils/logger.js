// Simple logger utility with different log levels

export const logger = {
  info(...args) {
    console.log('[INFO]', ...args);
  },

  warn(...args) {
    console.warn('[WARN]', ...args);
  },

  error(...args) {
    console.error('[ERROR]', ...args);
  },

  debug(...args) {
    if (typeof process !== 'undefined' && process.env?.DEBUG) {
      console.log('[DEBUG]', ...args);
    }
  },
};
