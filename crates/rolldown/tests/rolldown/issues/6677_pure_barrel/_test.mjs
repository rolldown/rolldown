import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const distDir = path.join(import.meta.dirname, 'dist');
const assets = Object.fromEntries(
  fs.readdirSync(distDir).map((file) => [file, fs.readFileSync(path.join(distDir, file), 'utf8')]),
);

assert(!('components.js' in assets), 'the pure empty barrel should be eliminated');
for (const [file, code] of Object.entries(assets)) {
  assert(!code.includes('./components.js'), `${file} should not import the eliminated pure barrel`);
}

await import('./dist/index.js');
await import('./dist/index-1.js');
