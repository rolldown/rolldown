import type { RolldownLog } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const warnings: RolldownLog[] = [];
const consoleWarnings: string[] = [];
let restoreConsoleWarn = () => {};

export default defineTest({
  sequential: true,
  config: {
    experimental: {
      importDefer: true,
    },
    onLog(level, log) {
      if (level === 'warn') {
        warnings.push(log);
      }
    },
  },
  beforeTest() {
    const original = console.warn;
    console.warn = (...args: unknown[]) => {
      consoleWarnings.push(args.map(String).join(' '));
      original(...args);
    };
    restoreConsoleWarn = () => {
      console.warn = original;
    };
  },
  afterTest: async () => {
    restoreConsoleWarn();
    // The validator accepts the option.
    expect(
      consoleWarnings.filter((message) => message.includes('Invalid input options')),
    ).toStrictEqual([]);
    // The build keeps the deferred evaluation, so it gives no warning.
    expect(warnings.map((log) => log.code)).toStrictEqual([]);
    const m = await import('./dist/main.js' as string);
    // `dep.js` evaluates at the first access to `dep.value`, after `before` reads the log.
    expect(m.before).toStrictEqual([]);
    expect(m.value).toBe(1);
    expect(m.after).toStrictEqual(['dep']);
  },
});
