import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

// The entry parses and runs neither module: each body stays in its wrapper.
globalThis.__events = [];
await import('./dist/main.js');
assert.deepStrictEqual(globalThis.__events, []);

// No access reads a namespace, so no code calls `__toESM`.
const dist = path.join(import.meta.dirname, 'dist');
for (const file of fs.readdirSync(dist).filter((name) => name.endsWith('.js'))) {
  const code = fs.readFileSync(path.join(dist, file), 'utf8');
  assert.ok(!code.includes('__toESM'), `${file} contains __toESM`);
}
