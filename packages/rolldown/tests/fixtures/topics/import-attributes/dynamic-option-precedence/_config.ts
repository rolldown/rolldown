import { execFile } from 'node:child_process';
import path from 'node:path';
import { promisify } from 'node:util';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

let deprecatedAssertionWarnings = 0;

export default defineTest({
  config: {
    experimental: { importAttributes: true },
    external: ['data:application/json,42'],
    onLog(level, log) {
      if (level === 'warn' && log.code === 'DEPRECATED_IMPORT_ASSERTION') {
        deprecatedAssertionWarnings++;
      }
    },
  },
  async afterTest() {
    // Run with Node's native import() so the data URL receives the emitted import options.
    await promisify(execFile)(process.execPath, [path.join(import.meta.dirname, 'assert.mjs')]);

    expect(deprecatedAssertionWarnings).toBe(3);
  },
});
