/**
 * Minimal injectable logger (the test-dev-server analog of Vite's
 * `customLogger`). The server and environment log through this instead of
 * bare `console.*` so the in-process browser test harness can capture server
 * output into its `serverLogs` array. Defaults to `console` everywhere, which
 * preserves the CLI (`serve`) behavior.
 */
export interface Logger {
  info(...args: unknown[]): void;
  warn(...args: unknown[]): void;
  error(...args: unknown[]): void;
  debug(...args: unknown[]): void;
}
