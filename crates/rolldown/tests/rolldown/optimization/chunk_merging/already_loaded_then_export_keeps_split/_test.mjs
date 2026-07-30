import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const logs = [];
const originalLog = console.log;
console.log = (...args) => {
  logs.push(args.map(String).join(' '));
};

let resolved;
try {
  const { result } = await import('./dist/main.js');
  // `import('./app.js')` resolves through the exported callable `then`, which
  // forwards the lazy chain: f0(3) = 7.
  resolved = await result;
} finally {
  console.log = originalLog;
}

assert.deepEqual(logs, ['3']);
assert.strictEqual(resolved, 7);

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((file) =>
  file.endsWith('.js'),
);

// Namespace extraction is refused for a `then`-exporting entry, so `vendor.js`
// must stay in its own chunk: main + app + lazy + vendor.
assert.strictEqual(jsFiles.length, 4, `Expected 4 chunks but got: ${jsFiles.join(', ')}`);
