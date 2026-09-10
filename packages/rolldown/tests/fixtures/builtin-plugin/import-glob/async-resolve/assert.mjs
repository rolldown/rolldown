import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const code = fs.readFileSync(path.join(import.meta.dirname, 'dist/main.js'), 'utf8');
assert.match(code, /features\/a\.js/);
assert.match(code, /other\/b\.js/);
