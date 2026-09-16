import assert from 'node:assert/strict';
import { readdir } from 'node:fs/promises';

const files = await readdir(new URL('./dist/', import.meta.url));
assert.ok(files.includes('app~main.js'));
assert.ok(files.includes('app~main~lazy.js'));

let timer;
try {
  await Promise.race([
    import('./dist/main.js'),
    new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error('entry evaluation did not settle')), 5000);
    }),
  ]);
} finally {
  clearTimeout(timer);
}

assert.equal(typeof globalThis.__tla_emitted_entry_ran, 'number');
