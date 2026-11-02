import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const code = fs.readFileSync(path.join(import.meta.dirname, 'dist/main.js'), 'utf8');
assert.ok(code.includes('features/a.js'));
assert.ok(code.includes('other/b.js'));
