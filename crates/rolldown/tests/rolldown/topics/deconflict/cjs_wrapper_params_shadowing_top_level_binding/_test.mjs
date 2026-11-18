import { strict as assert } from 'node:assert';
import fs from 'node:fs';
const lib =
  globalThis.__configName === 'iife'
    ? new Function(
        fs.readFileSync(new URL('./dist/main.js', import.meta.url), 'utf8') + '; return lib;',
      )()
    : (await import('./dist/main.js')).default;
assert.deepEqual(lib, { value: ['esm-module', 'esm-exports'], self: true });
