import fs from 'node:fs';
import assert from 'node:assert';
import path from 'node:path';

assert.strictEqual(
  fs.readdirSync(path.resolve(import.meta.dirname, './dist')).filter((item) => item.endsWith('.js'))
    .length,
  1,
);
