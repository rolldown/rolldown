import assert from 'node:assert';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const { result } = await import('./dist/main.js');

// `import('./app.js')` resolves through the exported callable `then`, which
// forwards the lazy chain: 7 * 2.
assert.strictEqual(await result, 14);

const jsFiles = readdirSync(join(import.meta.dirname, 'dist')).filter((file) =>
  file.endsWith('.js'),
);

// Namespace extraction is refused for a `then`-exporting entry, so `shared.js`
// must stay in its own chunk: main + app + lazy + shared.
assert.strictEqual(jsFiles.length, 4, `Expected 4 chunks but got: ${jsFiles.join(', ')}`);
